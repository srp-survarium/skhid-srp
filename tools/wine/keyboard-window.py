"""Windows half of check-keyboard.py: record actual key and character events."""
import ctypes as c
from ctypes import wintypes as w
import json

user32 = c.WinDLL("user32")
kernel32 = c.WinDLL("kernel32")
window_proc = c.WINFUNCTYPE(c.c_ssize_t, w.HWND, w.UINT, w.WPARAM, w.LPARAM)


class WindowClass(c.Structure):
    _fields_ = [("style", w.UINT), ("proc", window_proc),
                ("class_extra", c.c_int), ("window_extra", c.c_int),
                ("instance", w.HINSTANCE), ("icon", w.HICON), ("cursor", w.HANDLE),
                ("brush", w.HBRUSH), ("menu", w.LPCWSTR), ("name", w.LPCWSTR)]


user32.DefWindowProcW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user32.DefWindowProcW.restype = c.c_ssize_t
user32.CreateWindowExW.argtypes = [w.DWORD, w.LPCWSTR, w.LPCWSTR, w.DWORD,
                                  c.c_int, c.c_int, c.c_int, c.c_int,
                                  w.HWND, w.HMENU, w.HINSTANCE, w.LPVOID]
user32.CreateWindowExW.restype = w.HWND
user32.SetTimer.argtypes = [w.HWND, c.c_size_t, w.UINT, c.c_void_p]
user32.SetForegroundWindow.argtypes = [w.HWND]
user32.SetFocus.argtypes = [w.HWND]
user32.RegisterClassW.argtypes = [c.POINTER(WindowClass)]
user32.GetKeyboardLayout.restype = c.c_void_p
user32.MapVirtualKeyExW.argtypes = [w.UINT, w.UINT, c.c_void_p]
user32.GetMessageW.argtypes = [c.POINTER(w.MSG), w.HWND, w.UINT, w.UINT]
user32.TranslateMessage.argtypes = [c.POINTER(w.MSG)]
user32.DispatchMessageW.argtypes = [c.POINTER(w.MSG)]
user32.DispatchMessageW.restype = c.c_ssize_t
kernel32.GetModuleHandleW.restype = w.HMODULE


@window_proc
def handler(hwnd, message, key, parameter):
    if message in (0x100, 0x101, 0x102):  # WM_KEYDOWN, WM_KEYUP, WM_CHAR
        print(json.dumps({"event": hex(message), "key": key,
                          "scan": (parameter >> 16) & 0x1ff,
                          "char": chr(key) if message == 0x102 else None}), flush=True)
    if message == 0x113:  # WM_TIMER: bounded probe lifetime
        user32.PostQuitMessage(0)
        return 0
    return user32.DefWindowProcW(hwnd, message, key, parameter)


window_class = WindowClass()
window_class.proc = handler
window_class.instance = kernel32.GetModuleHandleW(None)
window_class.name = "SrpKeyboardProbe"
assert user32.RegisterClassW(c.byref(window_class))
window = user32.CreateWindowExW(0, window_class.name, "SRP keyboard probe", 0x10cf0000,
                                0, 0, 600, 400, None, None, window_class.instance, None)
assert window
user32.SetForegroundWindow(window)
user32.SetFocus(window)
assert user32.SetTimer(window, 1, 6000, None)
layout = user32.GetKeyboardLayout(0)
print(json.dumps({"ready": True, "maps": {
    key: user32.MapVirtualKeyExW(ord(key), 0, layout) for key in "WASD"}}), flush=True)
message = w.MSG()
while user32.GetMessageW(c.byref(message), None, 0, 0) > 0:
    user32.TranslateMessage(c.byref(message))
    user32.DispatchMessageW(c.byref(message))
