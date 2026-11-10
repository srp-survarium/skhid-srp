#!/usr/bin/env python3
"""Drives real game windows on Windows, reliably, for tests that need the real client.

    from driver import Run
    with Run(game_dir, server_log, host) as run:
        a = run.launch("ReplayTester", "password")
        a.sign_in()
        a.click(1099, 52)
        a.expect_server(r"ReplayTester .* signed in")

Every game belongs to one `Game`, by its own process id. Before any input the game's window must be
the foreground one, or the step stops. A wait reads only what was logged after it began, and stops on a
timeout with a screenshot. On leaving `Run`, held keys are let go and only this run's games are closed.

The game draws its arrow where the Windows cursor is, and takes keys as scan codes (DirectInput; a virtual key
never arrives). So the Windows cursor is placed on the target and the arrow checked by its picture.

Needs numpy.
"""
from __future__ import annotations

import atexit
import ctypes
import json
import os
import re
import shutil
import subprocess
import threading
import time
from ctypes import wintypes
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ULONG_PTR = ctypes.c_size_t
user32 = ctypes.WinDLL("user32", use_last_error=True)
gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
# The game is not DPI aware: Windows scales its window. Unaware too, this process sees the window in the game's own
# pixels; aware, the picture is the scaled size with the game in one corner of it.
user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-1))
for function in (user32.GetDC, user32.GetWindowDC):
    function.restype = wintypes.HDC
    function.argtypes = [wintypes.HWND]
user32.ReleaseDC.argtypes = [wintypes.HWND, wintypes.HDC]
user32.PrintWindow.argtypes = [wintypes.HWND, wintypes.HDC, wintypes.UINT]
gdi32.CreateCompatibleDC.restype = wintypes.HDC
gdi32.CreateCompatibleDC.argtypes = [wintypes.HDC]
gdi32.CreateCompatibleBitmap.restype = wintypes.HBITMAP
gdi32.CreateCompatibleBitmap.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int]
gdi32.SelectObject.restype = wintypes.HGDIOBJ
gdi32.SelectObject.argtypes = [wintypes.HDC, wintypes.HGDIOBJ]
gdi32.DeleteObject.argtypes = [wintypes.HGDIOBJ]
gdi32.DeleteDC.argtypes = [wintypes.HDC]
gdi32.GetDIBits.argtypes = [wintypes.HDC, wintypes.HBITMAP, wintypes.UINT, wintypes.UINT, ctypes.c_void_p, ctypes.c_void_p, wintypes.UINT]
user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
user32.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
user32.GetForegroundWindow.restype = wintypes.HWND
user32.SetForegroundWindow.argtypes = [wintypes.HWND]
user32.ShowWindow.argtypes = [wintypes.HWND, ctypes.c_int]
user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
user32.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
user32.IsWindowVisible.argtypes = [wintypes.HWND]
user32.IsHungAppWindow.argtypes = [wintypes.HWND]
user32.AttachThreadInput.argtypes = [wintypes.DWORD, wintypes.DWORD, wintypes.BOOL]
user32.SendMessageTimeoutW.argtypes = [wintypes.HWND, wintypes.UINT, ctypes.c_size_t, ctypes.c_size_t, wintypes.UINT,
                                       wintypes.UINT, ctypes.POINTER(ctypes.c_size_t)]
user32.SendMessageTimeoutW.restype = ctypes.c_size_t
user32.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
user32.ClipCursor.argtypes = [ctypes.POINTER(wintypes.RECT)]
user32.GetCursorPos.argtypes = [ctypes.POINTER(wintypes.POINT)]
user32.SetCursorPos.argtypes = [ctypes.c_int, ctypes.c_int]
user32.WindowFromPoint.argtypes = [wintypes.POINT]
user32.WindowFromPoint.restype = wintypes.HWND
user32.GetAncestor.argtypes = [wintypes.HWND, wintypes.UINT]
user32.GetAncestor.restype = wintypes.HWND
atexit.register(user32.ClipCursor, None)  # the owner's cursor is theirs again, however a run ends

MOVE, LEFT_DOWN, LEFT_UP, RIGHT_DOWN, RIGHT_UP = 0x0001, 0x0002, 0x0004, 0x0008, 0x0010
KEY_UP, SCANCODE, EXTENDED = 0x0002, 0x0008, 0x0001
VK = {"backspace": 0x08, "enter": 0x0D, "esc": 0x1B, "tab": 0x09, "space": 0x20, "left": 0x25, "right": 0x27, "up": 0x26, "down": 0x28,
      "ctrl": 0x11, "shift": 0x10, "m": 0x4D, "h": 0x48, "c": 0x43, "k": 0x4B, "1": 0x31, "2": 0x32, "3": 0x33}
ARROWS = ("left", "right", "up", "down")
# The top left corner of the client area: the cursor parked there is looked for only when it is nowhere else.
CORNER = (27, 34)


class Again(Exception):
    """The run drew something the flow cannot test, or lost a step to a box of the game's own: it starts over."""


class Stop(Exception):
    """A step did not do what it had to."""


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG), ("mouseData", wintypes.DWORD), ("dwFlags", wintypes.DWORD),
                ("time", wintypes.DWORD), ("dwExtraInfo", ULONG_PTR)]


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [("wVk", wintypes.WORD), ("wScan", wintypes.WORD), ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD),
                ("dwExtraInfo", ULONG_PTR)]


class _U(ctypes.Union):
    _fields_ = [("mi", MOUSEINPUT), ("ki", KEYBDINPUT), ("pad", ctypes.c_byte * 32)]


class INPUT(ctypes.Structure):
    _fields_ = [("type", wintypes.DWORD), ("u", _U)]


def send(*events: INPUT) -> None:
    array = (INPUT * len(events))(*events)
    if user32.SendInput(len(events), array, ctypes.sizeof(INPUT)) != len(events):
        raise OSError(ctypes.get_last_error(), "SendInput")


def mouse(dx: int = 0, dy: int = 0, flags: int = 0) -> INPUT:
    event = INPUT(type=0)
    event.u.mi = MOUSEINPUT(dx, dy, 0, flags, 0, 0)
    return event


def scan(code: int, flags: int = 0) -> INPUT:
    event = INPUT(type=1)
    event.u.ki = KEYBDINPUT(0, code, flags | SCANCODE, 0, 0)
    return event


def key_scan(name: str) -> int:
    return user32.MapVirtualKeyW(VK[name], 0)


class Sprite:
    """A cursor's picture: its opaque pixels, relative to its tip."""

    def __init__(self, points: list) -> None:
        data = np.array(points, dtype=np.int32)
        order = np.argsort(-(data[:, 2:].sum(axis=1)))
        self.body = data[order[: max(40, len(data) // 2)]]
        self.edge = data[order[-len(data) // 4:]]
        self.contrast = float(self.body[:, 2:].mean() - self.edge[:, 2:].mean())

    def _stands_out(self, image: np.ndarray, x: int, y: int) -> bool:
        """The cursor's fill is lighter than its outline, wherever it is; the same greys in a flat patch of wall are not."""
        height, width = image.shape[:2]

        def level(points: np.ndarray) -> float | None:
            px, py = x + points[:, 0], y + points[:, 1]
            inside = (0 <= px) & (px < width) & (0 <= py) & (py < height)
            return float(image[py[inside], px[inside]].mean()) if inside.any() else None

        fill, edge = level(self.body), level(self.edge)
        return fill is not None and edge is not None and fill - edge >= 0.4 * self.contrast

    @classmethod
    def load(cls, name: str) -> "Sprite":
        return cls(json.loads((HERE / name).read_text()))

    def find(self, image: np.ndarray, tolerance: int = 30, needed: float = 0.8, skip_corner: bool = False,
             near: tuple[int, int, int] | None = None) -> tuple[int, int] | None:
        """The tip's position in `image` (height, width, 3 RGB), every body pixel weighed: a bright patch of a
        picture cannot pass for the sprite."""
        anchor = self.body[0]
        close = np.abs(image.astype(np.int32) - anchor[2:]).max(axis=2) <= tolerance
        ys, xs = np.nonzero(close)
        if len(ys) == 0:
            return None
        ys, xs = ys - anchor[1], xs - anchor[0]
        if skip_corner:
            away = (xs >= CORNER[0]) | (ys >= CORNER[1])
            ys, xs = ys[away], xs[away]
        if near is not None:  # (x, y, radius): where a correction put it, so a look-alike elsewhere cannot pass
            inside_window = (np.abs(xs - near[0]) <= near[2]) & (np.abs(ys - near[1]) <= near[2])
            ys, xs = ys[inside_window], xs[inside_window]
        height, width = image.shape[:2]
        # A few of the body's pixels first, over every candidate at once: a 3D scene has thousands of them.
        sample = self.body[:: max(1, len(self.body) // 10)]
        votes = np.zeros(len(xs), dtype=np.int32)
        for px, py, *colour in sample.tolist():
            at_x, at_y = xs + px, ys + py
            inside = (0 <= at_x) & (at_x < width) & (0 <= at_y) & (at_y < height)
            good = np.zeros(len(xs), dtype=bool)
            good[inside] = np.abs(image[at_y[inside], at_x[inside]].astype(np.int32) - colour).max(axis=1) <= tolerance
            votes += good
        keep = votes >= int(0.6 * len(sample))
        ys, xs = ys[keep], xs[keep]
        best, best_score = None, 0
        for y, x in zip(ys.tolist(), xs.tolist()):
            px, py = x + self.body[:, 0], y + self.body[:, 1]
            inside = (0 <= px) & (px < width) & (0 <= py) & (py < height)
            if not inside.any():
                continue
            colours = image[py[inside], px[inside]].astype(np.int32)
            score = int((np.abs(colours - self.body[inside, 2:]).max(axis=1) <= tolerance).sum())
            if score > best_score and self._stands_out(image, x, y):
                best, best_score = (x, y), score
        return best if best_score >= needed * len(self.body) else None


# Only the arrow: the text and hover cursors are dim and thin, and a template of them matches dark interface.
ARROW = Sprite.load("arrow.json")


def capture(window: int) -> tuple[np.ndarray, tuple[int, int]]:
    """The window's picture (RGB) and where its client area begins in it: rendered by the window itself, so
    nothing covering it on the desktop can appear."""
    rect, origin = wintypes.RECT(), wintypes.POINT(0, 0)
    user32.GetWindowRect(window, ctypes.byref(rect))
    user32.ClientToScreen(window, ctypes.byref(origin))
    width, height = rect.right - rect.left, rect.bottom - rect.top
    screen = user32.GetWindowDC(window)
    memory = gdi32.CreateCompatibleDC(screen)
    bitmap = gdi32.CreateCompatibleBitmap(screen, width, height)
    previous = gdi32.SelectObject(memory, bitmap)
    try:
        if not user32.PrintWindow(window, memory, 2):
            raise Stop("PrintWindow failed")
        header = (ctypes.c_int32 * 10)(40, width, -height, 0x00200001, 0, 0, 0, 0, 0, 0)
        buffer = ctypes.create_string_buffer(width * height * 4)
        if not gdi32.GetDIBits(memory, bitmap, 0, height, buffer, ctypes.byref(header), 0):
            raise Stop("GetDIBits failed")
    finally:
        gdi32.SelectObject(memory, previous)
        gdi32.DeleteObject(bitmap)
        gdi32.DeleteDC(memory)
        user32.ReleaseDC(window, screen)
    image = np.frombuffer(buffer, dtype=np.uint8).reshape(height, width, 4)[:, :, [2, 1, 0]].copy()
    return image, (origin.x - rect.left, origin.y - rect.top)


def save_png(image: np.ndarray, path: Path) -> None:
    import struct
    import zlib

    height, width = image.shape[:2]
    raw = b"".join(b"\0" + image[y].tobytes() for y in range(height))
    chunk = lambda kind, data: struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(raw, 1)) + chunk(b"IEND", b""))


class Log:
    """A growing log read only from where a wait began."""

    def __init__(self, path: Path) -> None:
        self.path = path

    def text(self) -> str:
        return self.path.read_text(encoding="utf-8", errors="replace").replace("\0", "") if self.path.exists() else ""

    def mark(self) -> int:
        return len(self.text())

    def since(self, mark: int) -> str:
        return self.text()[mark:]


def room_released(log: str, room: str) -> bool:
    """Whether the server's log says no one is left in a room."""
    if re.search(rf"left room {room} with its session", log):
        return True
    left_with_match = re.search(rf"and with it room {room}\b", log) is not None
    sent_back = re.search(rf"Room {room}: its match \S+ is over; back in the room", log)
    if not sent_back:
        return left_with_match
    # One went with the match and the other, sent back to the room, left it: the flows' rooms have two.
    return left_with_match and re.search(rf": left room {room}\s*$", log, re.MULTILINE) is not None


def front_window() -> str:
    """What has the foreground, for a failure's message."""
    handle = user32.GetForegroundWindow()
    title, owner = ctypes.create_unicode_buffer(120), wintypes.DWORD()
    user32.GetWindowTextW(handle, title, 120)
    user32.GetWindowThreadProcessId(handle, ctypes.byref(owner))
    return f"window {handle}, '{title.value}', process {owner.value}"


class Game:
    def __init__(self, run: "Run", account: str, process: subprocess.Popen) -> None:
        self.run, self.account, self.process = run, account, process
        self.held: set[str] = set()
        # (test(picture), close(game)): boxes the game puts up by itself at a time of its own (the daily reward's, some
        # seconds after a sign-in). Modal: a press elsewhere does nothing while one is up.
        self.dialogs: list = []
        self._closing = False

    def close_dialogs(self, picture: np.ndarray | None = None) -> bool:
        """Closes a box of `dialogs` that is up. Before every input, so the input is not lost to it."""
        if self._closing or not self.dialogs:
            return False
        picture = self.picture() if picture is None else picture
        for test, close in self.dialogs:
            if test(picture):
                self._closing = True
                try:
                    close(self)
                finally:
                    self._closing = False
                return True
        return False

    def _dialog_in_wait(self, picture: np.ndarray | None = None) -> None:
        """A box that came up during a wait is closed, and the flow starts over: the input before the wait may have
        gone to the box."""
        if self.close_dialogs(picture):
            raise Again(f"{self.account}: a box of the game's own came up during a wait, and was closed")

    @property
    def window(self) -> int:
        found: list[int] = []

        def visit(handle, _):
            owner = wintypes.DWORD()
            user32.GetWindowThreadProcessId(handle, ctypes.byref(owner))
            title = ctypes.create_unicode_buffer(32)
            user32.GetWindowTextW(handle, title, 32)
            if owner.value == self.process.pid and user32.IsWindowVisible(handle) and title.value.startswith("Survarium"):
                found.append(handle)
            return True

        user32.EnumWindows(ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)(visit), 0)
        if not found:
            raise Stop(f"{self.account}: no game window")
        return found[0]

    # Input: only ever to this game's window, with the window verified first.

    def focus(self) -> None:
        window = self.window
        if user32.GetForegroundWindow() != window:
            # Allowed to change the foreground by sharing the input queue of the thread that has it. Not by a tap of Alt,
            # the usual trick: that lands on the game in front and puts its window in menu mode.
            def bring_front() -> None:
                # With only the desktop in front there is no input queue worth sharing; Windows lets the process that made
                # the last input event take the foreground, and a move of nothing reaches no game.
                send(mouse(0, 0, MOVE))
                own, front = kernel32.GetCurrentThreadId(), user32.GetWindowThreadProcessId(user32.GetForegroundWindow(), None)
                user32.AttachThreadInput(own, front, True)
                try:
                    user32.ShowWindow(window, 9)
                    user32.SetForegroundWindow(window)
                finally:
                    user32.AttachThreadInput(own, front, False)

            # A window whose thread is still starting the renderer cannot take the activation: first it has to answer.
            answer = ctypes.c_size_t()
            if not user32.SendMessageTimeoutW(window, 0, 0, 0, 0, 60000, ctypes.byref(answer)):  # WM_NULL, SMTO_NORMAL
                raise self.fail("the game's window did not answer for 60 s")
            self.run.note(f"{self.account} asks for the foreground, held by {front_window()}")
            end = time.time() + 5
            bring_front()
            asked = time.time()
            while user32.GetForegroundWindow() != window:
                if time.time() > end:
                    raise self.fail(f"could not take the foreground for window {window}: it is with {front_window()}")
                window = self.window  # a game on a cold first start makes its window again while it starts its renderer
                if time.time() - asked > 0.25:  # a window still being made can refuse, and asks are not queued
                    bring_front()
                    asked = time.time()
                time.sleep(0.02)
            # Input sent now would be lost: the game takes its mouse and keyboard back when it has handled the activation.
            # A message to its window returns once its thread has got that far, and a frame or two later it reads input.
            answer = ctypes.c_size_t()
            if not user32.SendMessageTimeoutW(window, 0, 0, 0, 2, 5000, ctypes.byref(answer)):  # WM_NULL, SMTO_ABORTIFHUNG
                raise self.fail("the game did not handle being brought to the front")
            time.sleep(0.25)
        self._confine(window)

    def _confine(self, window: int) -> None:
        """Keeps the Windows cursor inside this game. Input moves it as well as the game's own cursor, and a press lands
        on whatever window is under it: on another game, that takes the focus and its mouse is lost for good."""
        area, origin = wintypes.RECT(), wintypes.POINT(0, 0)
        user32.GetClientRect(window, ctypes.byref(area))
        user32.ClientToScreen(window, ctypes.byref(origin))
        clip = wintypes.RECT(origin.x, origin.y, origin.x + area.right, origin.y + area.bottom)
        if not user32.ClipCursor(ctypes.byref(clip)):
            raise OSError(ctypes.get_last_error(), "ClipCursor")

    def _aim_windows_cursor(self, x: int, y: int) -> None:
        """The game draws its cursor from relative input, and the Windows cursor is elsewhere: a press must find both on
        the target. Then checks that it lands on this game and no other window."""
        window = self.window
        at = wintypes.POINT(x, y)
        user32.ClientToScreen(window, ctypes.byref(at))
        if not user32.SetCursorPos(at.x, at.y):
            raise OSError(ctypes.get_last_error(), "SetCursorPos")
        for attempt in range(6):  # with several games open, another can rise over this one for a moment
            time.sleep(0.05 if attempt == 0 else 0.3)
            now = wintypes.POINT()
            user32.GetCursorPos(ctypes.byref(now))
            under = user32.GetAncestor(user32.WindowFromPoint(now), 2)  # GA_ROOT
            if under == window and (now.x, now.y) == (at.x, at.y):
                return
            self.focus()
            user32.SetCursorPos(at.x, at.y)
        title = ctypes.create_unicode_buffer(120)
        user32.GetWindowTextW(under, title, 120)
        raise self.fail(f"a press at ({now.x}, {now.y}), wanted ({at.x}, {at.y}), would not land on this game: {title.value!r} is over it")

    def picture(self) -> np.ndarray:
        # PrintWindow now and then hands back an all-black frame of a game that is drawing normally: taken again.
        for _ in range(30):
            image, (left, top) = capture(self.window)
            if image.max() > 0:
                break
            time.sleep(0.03)
        return image[top:, left:]

    def cursor(self, near: tuple[int, int, int] | None = None) -> tuple[int, int] | None:
        image = self.picture()
        return ARROW.find(image, skip_corner=True, near=near) or ARROW.find(image, near=near)

    def steady_cursor(self, near: tuple[int, int, int] | None = None) -> tuple[int, int] | None:
        """The cursor once two reads agree: a picture is read in milliseconds, before the game has drawn the last move."""
        time.sleep(0.05)
        seen = self.cursor(near)
        for _ in range(8):
            time.sleep(0.04)
            again = self.cursor(near)
            if again == seen:
                break
            seen = again
        return seen

    def _place(self, x: int, y: int) -> None:
        """The Windows cursor on (x, y), and a one-count wiggle: the game redraws its arrow when the mouse reports motion."""
        self._aim_windows_cursor(x, y)
        send(mouse(1, 0, MOVE))
        time.sleep(0.03)
        send(mouse(-1, 0, MOVE))

    def move(self, x: int, y: int, tolerance: int = 14) -> None:
        """The game's arrow is the Windows cursor, seen from the window: placed there, then checked by its picture."""
        self.close_dialogs()
        self.focus()
        self._place(x, y)
        end = time.time() + 2
        while time.time() < end:
            seen = self.steady_cursor((x, y, tolerance))
            if seen is not None:
                print(f"    {self.account}: cursor at {seen}, want ({x}, {y})", flush=True)
                return
        if self._arrow_returns_near(x, y):
            print(f"    {self.account}: over a control at ({x}, {y})", flush=True)
            return
        raise self.fail(f"no arrow at ({x}, {y}) or beside it")

    def click(self, x: int, y: int, tolerance: int = 14) -> None:
        self.move(x, y, tolerance)
        self.focus()
        self._aim_windows_cursor(x, y)
        time.sleep(0.15)  # the movie must have drawn the hover, or the press does not reach the button
        self.run.note(f"{self.account} press at ({x},{y}) down")
        send(mouse(flags=LEFT_DOWN))
        time.sleep(0.06)
        send(mouse(flags=LEFT_UP))
        self.run.note(f"{self.account} press up")
        time.sleep(0.05)

    def _arrow_returns_near(self, x: int, y: int) -> bool:
        """Beside (x, y) and back: true when the arrow shows there, so a control had changed its shape on (x, y)."""
        area = wintypes.RECT()
        user32.GetClientRect(self.window, ctypes.byref(area))
        try:
            for dx, dy in ((0, -60), (60, 0), (-60, 0), (0, 60)):  # up first: beside a button in a bar is another button
                if 0 <= x + dx < area.right and 0 <= y + dy < area.bottom:
                    self._place(x + dx, y + dy)
                    if self.steady_cursor((x + dx, y + dy, 20)) is not None:
                        return True
            return False
        finally:
            self._place(x, y)

    def right_click(self, x: int, y: int, tolerance: int = 14) -> None:
        self.move(x, y, tolerance)
        self.focus()
        self._aim_windows_cursor(x, y)
        time.sleep(0.15)
        send(mouse(flags=RIGHT_DOWN))
        time.sleep(0.06)
        send(mouse(flags=RIGHT_UP))
        time.sleep(0.05)

    def key(self, name: str) -> None:
        self.focus()
        code, flag = key_scan(name), EXTENDED if name in ARROWS else 0
        send(scan(code, flag))
        time.sleep(0.05)
        send(scan(code, flag | KEY_UP))
        time.sleep(0.05)

    def hold_key(self, name: str, seconds: float) -> None:
        """A key held down as a player's finger would: a game action is read from the frames it is down in."""
        self.focus()
        send(scan(key_scan(name)))
        time.sleep(seconds)
        send(scan(key_scan(name), KEY_UP))
        time.sleep(0.05)

    def key_chord(self, held: str, name: str) -> None:
        """`name` pressed while `held` is down."""
        self.focus()
        send(scan(key_scan(held)))
        time.sleep(0.05)
        send(scan(key_scan(name)))
        time.sleep(0.05)
        send(scan(key_scan(name), KEY_UP))
        time.sleep(0.05)
        send(scan(key_scan(held), KEY_UP))
        time.sleep(0.05)

    def type(self, text: str) -> None:
        self.focus()
        shift = key_scan("shift")
        for character in text:
            found = user32.VkKeyScanW(ord(character))
            code, shifted = user32.MapVirtualKeyW(found & 0xFF, 0), (found >> 8) & 1
            if shifted:
                send(scan(shift))
            send(scan(code))
            time.sleep(0.015)
            send(scan(code, KEY_UP))
            if shifted:
                send(scan(shift, KEY_UP))
            time.sleep(0.015)

    def cursor_on(self) -> None:
        """Ctrl toggles a replay viewer's cursor. It comes up at the corner and takes the Windows cursor's place when the
        mouse next moves, so it is moved after Ctrl and then looked for where it must be: a look-alike in the scene cannot pass."""
        self.focus()
        here = (800, 700, 25)
        if self.cursor() is None:
            self.key("ctrl")
            time.sleep(0.3)
        self._place(*here[:2])
        if self.steady_cursor(here) is None:  # it was a look-alike: the cursor is still off
            self.key("ctrl")
            time.sleep(0.3)
            self._place(*here[:2])
            if self.steady_cursor(here) is None:
                raise self.fail("Ctrl did not bring up the cursor")

    # What the run needs to see.

    def shot(self, name: str) -> Path:
        path = self.run.shots / f"{name}.png"
        save_png(self.picture(), path)
        return path

    def fail(self, why: str) -> Stop:
        try:
            shot = self.shot(f"FAILED-{self.account}-{int(time.time())}")
        except Exception as error:
            shot = f"(no shot: {error})"
        try:  # the game writes this file over at its next start, and its piped log loses what is still buffered at a kill
            shutil.copy(self.run.game_log().path, Path(shot).with_suffix(".log"))
        except Exception as error:
            print(f"    no copy of the game's log: {error}", flush=True)
        crashes = self.run.new_crashes()
        crashed = f"; THE GAME CRASHED: {', '.join(crashes)}" if crashes else ""
        return Stop(f"{self.account}: {why}; screen in {shot}{crashed}")

    def wait_screen(self, test, what: str, seconds: float = 15, steady: float = 0) -> None:
        """Until `test(picture)` holds, for `steady` seconds if given (a window can show for a frame while the game
        changes screens); the game's windows open only when the server has answered."""
        end = time.time() + seconds
        held_since = None
        while True:
            picture = self.picture()
            self._dialog_in_wait(picture)
            if test(picture):
                held_since = held_since or time.time()
                if time.time() - held_since >= steady:
                    return
            else:
                held_since = None
            if time.time() > end:
                raise self.fail(f"not on screen within {seconds} s: {what}")
            time.sleep(0.1)

    def wait_game(self, pattern: str, seconds: float = 30) -> None:
        self.run.wait(self.run.game_log(), pattern, seconds, self)

    def expect_server(self, pattern: str, seconds: float = 15, since: int | None = None) -> str:
        return self.run.wait(self.run.server, pattern, seconds, self, since)

    def sign_in(self, seconds: float = 30) -> None:
        mark = self.run.server.mark()
        end = time.time() + seconds
        while time.time() < end:  # Enter does nothing until the login screen is up
            self.key("enter")
            if re.search(rf"{re.escape(self.account)} \(account \d+\) signed in", self.run.server.since(mark)):
                return
            time.sleep(0.5)
        raise self.fail("did not sign in")

    def close(self) -> None:
        if self.process.poll() is None:
            self.process.kill()
            self.process.wait(15)


class Run:
    """One test run: its games, and everything needed to leave the machine as it was."""

    def __init__(self, game_dir: str, server_log: str, host: str, shots: str) -> None:
        self.game_dir, self.host = Path(game_dir), host
        self.server, self.shots = Log(Path(server_log)), Path(shots)
        self.shots.mkdir(parents=True, exist_ok=True)
        self.documents = Path(os.path.expanduser("~")) / "Documents" / "Survarium"
        self.games: list[Game] = []
        self.known_crashes = set(self.game_dir.glob("SRP-crash-*"))
        self.began = self.server.mark()

    def new_crashes(self) -> list[str]:
        """The crash files written since this run began."""
        return sorted(path.name for path in set(self.game_dir.glob("SRP-crash-*")) - self.known_crashes)

    def note(self, text: str) -> None:
        """One line on the run's timeline, on the same clock as the samples."""
        with self.timeline_lock, (self.shots.parent / "timeline.log").open("a", encoding="utf-8") as out:
            out.write(f"{time.time():.3f} {text}\n")

    def _sample(self) -> None:
        """Ten times a second, per game: its arrow, the Windows cursor and who has the foreground."""
        while not self.sampling_over.wait(0.1):
            for game in list(self.games):
                try:
                    window = game.window
                    at, front = wintypes.POINT(), user32.GetForegroundWindow() == window
                    user32.GetCursorPos(ctypes.byref(at))
                    self.note(f"{game.account} arrow={game.cursor()} windows=({at.x},{at.y}) front={front}")
                except (Stop, OSError, ValueError) as error:
                    self.note(f"{game.account} sample failed: {error}")

    def __enter__(self) -> "Run":
        self.timeline_lock, self.sampling_over = threading.Lock(), threading.Event()
        self.note(f"===== run begins")
        threading.Thread(target=self._sample, daemon=True).start()
        return self

    def __exit__(self, *_) -> None:
        self.sampling_over.set()
        user32.ClipCursor(None)
        for name in ("ctrl", "shift"):
            send(scan(key_scan(name), KEY_UP))
        mark = self.server.mark()
        for game in self.games:
            game.close()
        # A killed game's account keeps its lobby session until the server times it out, and a room with someone sent back
        # to it stays a minute longer; PLAY in the next run would join that room. The run is over when both are gone.
        accounts = {game.account for game in self.games}
        end = time.time() + 150
        told = time.time()
        while True:
            since_kill, whole = self.server.since(mark), self.server.since(self.began)
            signed_in = {a for a in accounts if re.search(rf"{re.escape(a)} \(account \d+\)", whole)}  # a game killed before that holds nothing
            held = {a for a in signed_in if not re.search(rf"{re.escape(a)} \(account \d+\) left the lobby", since_kill)}
            rooms = {room for room in re.findall(r"made room (\d+)", whole) if not room_released(whole, room)}
            if not held and not rooms:
                return
            if time.time() > end:
                print(f"=== teardown: FAILED: after 150 s the server still holds {sorted(held)}, rooms {sorted(rooms)}", flush=True)
                return
            if time.time() - told >= 10:  # a watcher of this log sees the wait, and what it is for
                told = time.time()
                print(f"    teardown: the server still holds {sorted(held)}, rooms {sorted(rooms)}", flush=True)
            time.sleep(0.25)

    def game_log(self) -> Log:
        logs = list(self.documents.glob("Survarium*.log"))
        return Log(max(logs, key=lambda p: p.stat().st_mtime))

    def launch(self, account: str, password: str) -> Game:
        arguments = [str(self.game_dir / "survarium_skhid.exe"), f"-server={self.host}:1234", f"-account_name={account}",
                     f"-account_password={password}", f"-localization={os.environ.get('SRP_TEST_LANGUAGE', 'english')}", "-mt_allocator=768",
                     "-minimum_resources_memory_size=768", "-log_to_stdout"]
        # Every game of a Windows user writes the same log file, the last one started over the others: each has its own here.
        output = (self.shots.parent / f"client-{account}.log").open("ab", buffering=0)
        output.write(f"\n===== launched at {time.time():.3f}\n".encode())
        game = Game(self, account, subprocess.Popen(arguments, cwd=self.game_dir, stdout=output, stderr=subprocess.STDOUT,
                                                    creationflags=subprocess.DETACHED_PROCESS))
        self.games.append(game)
        end = time.time() + 30
        while True:
            try:
                game.window
                return game
            except Stop:
                if time.time() > end or game.process.poll() is not None:
                    raise Stop(f"{account}: no window after 30 s")
                time.sleep(0.2)

    def wait(self, log: Log, pattern: str, seconds: float, game: Game | None = None, since: int | None = None) -> str:
        mark = log.mark() if since is None else since
        end = time.time() + seconds
        looked = 0.0
        while True:
            found = re.search(pattern, log.since(mark))
            if found:
                return found.group(0)
            if game and game.dialogs and time.time() - looked > 0.5:
                looked = time.time()
                game._dialog_in_wait()
            if time.time() > end:
                why = f"not seen within {seconds} s: {pattern}"
                raise game.fail(why) if game else Stop(why)
            time.sleep(0.05)
