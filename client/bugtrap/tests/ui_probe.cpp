// Standalone dialog probe: no real crash, browser launch or network access.
// Uses only exported BugTrap entry points and Win32 controls/imports.
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static HMODULE bugtrap;
static HWND report;
static LONG browserCalls;
static bool browserFailure;
static bool clicked;
static bool sawBrowserError;
static bool dialogFailure;
static LANGID windowsLanguage;

static void PrintText(int id, HWND control)
{
    WCHAR text[2048];
    GetWindowTextW(control, text, 2048);
    for (WCHAR* p = text; *p; ++p) if (*p == '\r' || *p == '\n') *p = ' ';
    char utf8[8192];
    WideCharToMultiByte(CP_UTF8, 0, text, -1, utf8, sizeof(utf8), NULL, NULL);
    printf("TEXT %d %s\n", id, utf8);
}

static void Fits(HWND control, int id, bool wrap)
{
    WCHAR text[2048];
    int length = GetWindowTextW(control, text, 2048);
    RECT bounds;
    GetClientRect(control, &bounds);
    RECT measured = bounds;
    HDC dc = GetDC(control);
    HGDIOBJ old = SelectObject(dc, (HFONT)SendMessage(control, WM_GETFONT, 0, 0));
    DrawTextW(dc, text, length, &measured,
              DT_CALCRECT | DT_NOPREFIX | (wrap ? DT_WORDBREAK : DT_SINGLELINE));
    SelectObject(dc, old);
    ReleaseDC(control, dc);
    int padding = id == 1013 || id == IDCANCEL ? 8 : 0;
    if (measured.right > bounds.right - padding || measured.bottom > bounds.bottom) {
        fprintf(stderr, "clipped control %d: needs %ldx%ld, has %ldx%ld\n", id,
                measured.right, measured.bottom, bounds.right, bounds.bottom);
        ExitProcess(71);
    }
}

static HINSTANCE WINAPI Browser(HWND, LPCWSTR verb, LPCWSTR url, LPCWSTR args, LPCWSTR dir, INT)
{
    if (wcscmp(verb, L"open") || wcscmp(url, L"https://discord.gg/QUmJg3gBhg") || args || dir)
        ExitProcess(72);
    InterlockedIncrement(&browserCalls);
    return (HINSTANCE)(browserFailure ? 31 : 33);
}
static BOOL WINAPI RefuseMove(LPCWSTR, LPCWSTR, DWORD)
{ SetLastError(ERROR_ACCESS_DENIED); return FALSE; }
static BOOL WINAPI RefuseFlush(HANDLE)
{ SetLastError(ERROR_WRITE_FAULT); return FALSE; }
static HANDLE WINAPI RefuseCreate(LPCWSTR, DWORD, DWORD, LPSECURITY_ATTRIBUTES, DWORD, DWORD, HANDLE)
{ SetLastError(ERROR_ACCESS_DENIED); return INVALID_HANDLE_VALUE; }
static LANGID WINAPI WindowsLanguage() { return windowsLanguage; }
static INT_PTR WINAPI RefuseDialog(HINSTANCE, LPCDLGTEMPLATEW, HWND, DLGPROC, LPARAM)
{ SetLastError(ERROR_NOT_ENOUGH_MEMORY); return -1; }
// An antivirus refusing the capture: MiniDumpWriteDump fails as it was seen to,
// HRESULT_FROM_WIN32(ERROR_GEN_FAILURE), after BugTrap made the file.
static BOOL WINAPI RefuseCapture(HANDLE, DWORD, HANDLE, int, void*, void*, void*)
{ SetLastError(0x8007001F); return FALSE; }
static FARPROC WINAPI CaptureRefused(HMODULE module, LPCSTR name)
{
    if ((ULONG_PTR)name > 0xffff && !strcmp(name, "MiniDumpWriteDump")) return (FARPROC)RefuseCapture;
    return GetProcAddress(module, name);
}

static void PrintClipboard()
{
    if (!OpenClipboard(NULL)) ExitProcess(79);
    HANDLE data = GetClipboardData(CF_UNICODETEXT);
    const WCHAR* text = data ? (const WCHAR*)GlobalLock(data) : NULL;
    if (!text) ExitProcess(80);
    static char utf8[65536];
    int n = WideCharToMultiByte(CP_UTF8, 0, text, -1, utf8, sizeof(utf8), NULL, NULL);
    GlobalUnlock(data);
    CloseClipboard();
    int out = 0; // one line: CR dropped, LF shown as |
    for (int i = 0; i < n; ++i) if (utf8[i] != '\r') utf8[out++] = utf8[i] == '\n' ? '|' : utf8[i];
    printf("CLIP %s\n", utf8);
}

static void ReplaceImport(const char* name, void* replacement)
{
    BYTE* module = (BYTE*)bugtrap;
    IMAGE_DOS_HEADER* dos = (IMAGE_DOS_HEADER*)module;
    IMAGE_NT_HEADERS* pe = (IMAGE_NT_HEADERS*)(module + dos->e_lfanew);
    IMAGE_IMPORT_DESCRIPTOR* imports = (IMAGE_IMPORT_DESCRIPTOR*)(module +
        pe->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT].VirtualAddress);
    for (; imports->Name; ++imports) {
        IMAGE_THUNK_DATA* symbol = (IMAGE_THUNK_DATA*)(module + imports->OriginalFirstThunk);
        IMAGE_THUNK_DATA* slot = (IMAGE_THUNK_DATA*)(module + imports->FirstThunk);
        for (; symbol->u1.AddressOfData; ++symbol, ++slot) {
            if (IMAGE_SNAP_BY_ORDINAL(symbol->u1.Ordinal)) continue;
            IMAGE_IMPORT_BY_NAME* entry = (IMAGE_IMPORT_BY_NAME*)(module + symbol->u1.AddressOfData);
            if (strcmp((char*)entry->Name, name)) continue;
            DWORD old;
            if (!VirtualProtect(&slot->u1.Function, sizeof(DWORD), PAGE_READWRITE, &old)) ExitProcess(73);
            slot->u1.Function = (DWORD)replacement;
            VirtualProtect(&slot->u1.Function, sizeof(DWORD), old, &old);
            return;
        }
    }
    fprintf(stderr, "missing import: %s\n", name);
    ExitProcess(74);
}

static void Screenshot(HWND window)
{
    const char* path = getenv("SRP_UI_SCREENSHOT");
    if (!path) return;
    RECT rect;
    GetWindowRect(window, &rect);
    int width = rect.right - rect.left, height = rect.bottom - rect.top;
    HDC dc = GetWindowDC(window), copy = CreateCompatibleDC(dc);
    BITMAPINFO info = {0};
    info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    void* pixels;
    HBITMAP bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &pixels, NULL, 0);
    HGDIOBJ old = SelectObject(copy, bitmap);
    RedrawWindow(window, NULL, NULL, RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN);
    BitBlt(copy, 0, 0, width, height, dc, 0, 0, SRCCOPY);
    GdiFlush();
    BITMAPFILEHEADER header = {0};
    header.bfType = 0x4d42;
    header.bfOffBits = sizeof(header) + sizeof(info.bmiHeader);
    header.bfSize = header.bfOffBits + width * height * 4;
    FILE* file = fopen(path, "wb");
    if (!file) ExitProcess(75);
    fwrite(&header, sizeof(header), 1, file);
    fwrite(&info.bmiHeader, sizeof(info.bmiHeader), 1, file);
    fwrite(pixels, width * height * 4, 1, file);
    fclose(file);
    SelectObject(copy, old);
    DeleteObject(bitmap);
    DeleteDC(copy);
    ReleaseDC(window, dc);
}

static BOOL CALLBACK Inspect(HWND window, LPARAM)
{
    DWORD process;
    GetWindowThreadProcessId(window, &process);
    if (process != GetCurrentProcessId() || !IsWindowVisible(window)) return TRUE;
    if (dialogFailure && GetDlgItem(window, IDOK)) {
        PrintText(0, window);
        PrintText(-1, GetDlgItem(window, 0xffff));
        if (browserCalls) ExitProcess(78);
        puts("PASS localized fallback dialog, no browser launch");
        fflush(stdout);
        PostMessage(window, WM_COMMAND, IDOK, 0);
        return FALSE;
    }
    if (report && GetWindow(window, GW_OWNER) == report && GetDlgItem(window, IDOK)) {
        PrintText(-1, GetDlgItem(window, 0xffff));
        sawBrowserError = true;
        PostMessage(window, WM_COMMAND, IDOK, 0);
        return TRUE;
    }
    if (!GetDlgItem(window, 1036)) return TRUE;
    report = window;
    if (!clicked) {
        // Wait until WM_INITDIALOG has populated the file/status fields.
        if (!GetWindowTextLengthW(GetDlgItem(window, 1031))) return TRUE;
        PrintText(0, window);
        int ids[] = {1030, 1031, 1034, 1035, 1036, 1037, 1038, 1040, 1042, 1013, 1041, IDCANCEL};
        for (int i = 0; i < 12; ++i) {
            HWND control = GetDlgItem(window, ids[i]);
            if (!IsWindowVisible(control)) continue;
            PrintText(ids[i], control);
            if (ids[i] != 1034 && ids[i] != 1035)
                Fits(control, ids[i], ids[i] == 1030 || ids[i] == 1031 || ids[i] == 1036 || ids[i] == 1040 || ids[i] == 1042);
        }
        Screenshot(window);
        // Copy: the details reach the clipboard and the button says so.
        SendMessage(window, WM_COMMAND, 1041, 0);
        PrintClipboard();
        PrintText(-2, GetDlgItem(window, 1041));
        if (browserCalls) ExitProcess(76);
        clicked = true;
        PostMessage(window, WM_COMMAND, 1013, 0);
    } else if (browserCalls == 1 && (!browserFailure || (sawBrowserError && IsWindowEnabled(window)))) {
        puts("PASS localized dialog, text fits, browser opened only on click, dialog kept");
        fflush(stdout);
        PostMessage(window, WM_COMMAND, IDCANCEL, 0);
        return FALSE;
    }
    return TRUE;
}
static DWORD WINAPI InspectUI(PVOID)
{
    for (int i = 0; i < 300; ++i) {
        if (!EnumWindows(Inspect, 0)) return 0;
        Sleep(50);
    }
    ExitProcess(77);
    return 0;
}

int main(int argc, char** argv)
{
    if (argc < 3) return 2; // state and simulated Windows LANGID, then game arguments
    bugtrap = LoadLibraryW(L"BugTrap.dll");
    if (!bugtrap) return 3;
    windowsLanguage = (LANGID)strtoul(argv[2], NULL, 16);
    ReplaceImport("GetUserDefaultUILanguage", (void*)WindowsLanguage);
    ReplaceImport("ShellExecuteW", (void*)Browser);
    browserFailure = !strcmp(argv[1], "browser-failure");
    dialogFailure = !strcmp(argv[1], "dialog-failure");
    if (dialogFailure) ReplaceImport("DialogBoxIndirectParamW", (void*)RefuseDialog);
    if (!strcmp(argv[1], "raw")) ReplaceImport("MoveFileExW", (void*)RefuseMove);
    if (!strcmp(argv[1], "partial")) ReplaceImport("FlushFileBuffers", (void*)RefuseFlush);
    if (!strcmp(argv[1], "failure")) ReplaceImport("CreateFileW", (void*)RefuseCreate);
    if (!strcmp(argv[1], "blocked")) ReplaceImport("GetProcAddress", (void*)CaptureRefused);
    typedef void (WINAPI *SetValue)(DWORD);
    ((SetValue)GetProcAddress(bugtrap, "BT_SetActivityType"))(1);
    ((SetValue)GetProcAddress(bugtrap, "BT_SetExitMode"))(2);
    HANDLE thread = CreateThread(NULL, 0, InspectUI, NULL, 0, NULL);
    ((void (__cdecl*)())GetProcAddress(bugtrap, "BT_CallCppFilter"))();
    WaitForSingleObject(thread, INFINITE);
    CloseHandle(thread);
    return dialogFailure || (clicked && browserCalls == 1) ? 0 : 4;
}
