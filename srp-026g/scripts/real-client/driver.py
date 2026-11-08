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

The game reads raw mouse deltas and scan codes (DirectInput): a cursor position cannot be set, and a virtual
key never arrives. So the cursor is found by its picture in the window, moved by relative steps and corrected
until it is on the target.

Needs numpy.
"""
from __future__ import annotations

import ctypes
import json
import os
import re
import subprocess
import time
from ctypes import wintypes
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ULONG_PTR = ctypes.c_size_t
user32 = ctypes.WinDLL("user32", use_last_error=True)
gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)
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

MOVE, LEFT_DOWN, LEFT_UP = 0x0001, 0x0002, 0x0004
KEY_UP, SCANCODE, EXTENDED = 0x0002, 0x0008, 0x0001
VK = {"enter": 0x0D, "esc": 0x1B, "tab": 0x09, "space": 0x20, "left": 0x25, "right": 0x27, "up": 0x26, "down": 0x28,
      "ctrl": 0x11, "shift": 0x10, "m": 0x4D, "h": 0x48, "c": 0x43, "1": 0x31, "2": 0x32, "3": 0x33}
ARROWS = ("left", "right", "up", "down")
# The top left corner of the client area: the cursor parked there is looked for only when it is nowhere else.
CORNER = (27, 34)
GAIN = 0.9  # game pixels moved per mouse count, near enough for the correction loop to converge


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


class Game:
    def __init__(self, run: "Run", account: str, process: subprocess.Popen) -> None:
        self.run, self.account, self.process = run, account, process
        self.held: set[str] = set()

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
            send(scan(0x38), scan(0x38, KEY_UP))  # Alt, which lets a process take the foreground
            user32.ShowWindow(window, 9)
            user32.SetForegroundWindow(window)
            end = time.time() + 2
            while user32.GetForegroundWindow() != window:
                if time.time() > end:
                    raise self.fail("could not take the foreground")
                time.sleep(0.02)

    def _steps(self, dx: int, dy: int, size: int = 3) -> None:
        count = max(abs(dx), abs(dy)) // size + 1
        done_x = done_y = 0
        for i in range(1, count + 1):
            to_x, to_y = dx * i // count, dy * i // count
            send(mouse(to_x - done_x, to_y - done_y, MOVE))
            done_x, done_y = to_x, to_y
            time.sleep(0.002)

    def picture(self) -> np.ndarray:
        image, (left, top) = capture(self.window)
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

    def move(self, x: int, y: int, tolerance: int = 14) -> None:
        self.focus()
        last = window = None
        for _ in range(14):
            position = self.steady_cursor(window)
            if position is None and window is not None:
                position = self.cursor()
                # Not where the correction aimed, and nowhere else: over a control the cursor changes shape.
                if position is None and max(abs(x - last[0]), abs(y - last[1])) <= 500:
                    print(f"    {self.account}: over a control at ({x}, {y})", flush=True)
                    return
            print(f"    {self.account}: cursor at {position}, want ({x}, {y})", flush=True)
            if position is None or position == last:  # lost, or it did not move: park it where it is known
                self._steps(-5000, -5000, 200)
                last = window = None
                continue
            dx, dy = x - position[0], y - position[1]
            if abs(dx) <= tolerance and abs(dy) <= tolerance:
                return
            self._steps(int(dx / GAIN), int(dy / GAIN))
            last, window = position, (x, y, max(60, int(0.4 * max(abs(dx), abs(dy)))))
        raise self.fail(f"could not bring the cursor to ({x}, {y}), last at {last}")

    def click(self, x: int, y: int, tolerance: int = 14) -> None:
        self.move(x, y, tolerance)
        self.focus()
        send(mouse(flags=LEFT_DOWN))
        time.sleep(0.06)
        send(mouse(flags=LEFT_UP))
        time.sleep(0.05)

    def key(self, name: str) -> None:
        self.focus()
        code, flag = key_scan(name), EXTENDED if name in ARROWS else 0
        send(scan(code, flag))
        time.sleep(0.05)
        send(scan(code, flag | KEY_UP))
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
        """Ctrl toggles a replay viewer's cursor: pressed only when the cursor is off."""
        if self.cursor() is None:
            self.key("ctrl")
            end = time.time() + 2
            while self.cursor() is None:
                if time.time() > end:
                    raise self.fail("Ctrl did not bring up the cursor")
                time.sleep(0.05)

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
        return Stop(f"{self.account}: {why}; screen in {shot}")

    def wait_screen(self, test, what: str, seconds: float = 15) -> None:
        """Until `test(picture)` holds; the game's windows open only when the server has answered."""
        end = time.time() + seconds
        while not test(self.picture()):
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

    def __enter__(self) -> "Run":
        return self

    def __exit__(self, *_) -> None:
        for name in ("ctrl", "shift"):
            send(scan(key_scan(name), KEY_UP))
        for game in self.games:
            game.close()

    def game_log(self) -> Log:
        logs = list(self.documents.glob("Survarium*.log"))
        return Log(max(logs, key=lambda p: p.stat().st_mtime))

    def launch(self, account: str, password: str) -> Game:
        arguments = [str(self.game_dir / "survarium_skhid.exe"), f"-server={self.host}:1234", f"-account_name={account}",
                     f"-account_password={password}", "-localization=english", "-mt_allocator=768",
                     "-minimum_resources_memory_size=768"]
        game = Game(self, account, subprocess.Popen(arguments, cwd=self.game_dir, creationflags=subprocess.DETACHED_PROCESS))
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
        while True:
            found = re.search(pattern, log.since(mark))
            if found:
                return found.group(0)
            if time.time() > end:
                why = f"not seen within {seconds} s: {pattern}"
                raise game.fail(why) if game else Stop(why)
            time.sleep(0.05)
