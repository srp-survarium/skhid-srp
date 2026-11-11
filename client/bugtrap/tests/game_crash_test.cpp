#include <windows.h>
#include <tlhelp32.h>
#include <stdio.h>
#include <string.h>
#include "bugtrap_abi.h"

static DWORD gamePid;
static HWND gameWindow, crashDialog;

static BOOL CALLBACK FindWindows(HWND window, LPARAM)
{
    DWORD pid;
    GetWindowThreadProcessId(window, &pid);
    if (pid != gamePid || !IsWindowVisible(window)) return TRUE;
    if (GetDlgItem(window, IDC_INTRO2)) crashDialog = window;
    RECT rect;
    GetClientRect(window, &rect);
    if (rect.right > 400 && rect.bottom > 250 && !GetDlgItem(window, IDC_INTRO2))
        gameWindow = window;
    return TRUE;
}

static BOOL FindBugTrap(void)
{
    HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, gamePid);
    if (snapshot == INVALID_HANDLE_VALUE) return FALSE;
    MODULEENTRY32 module;
    module.dwSize = sizeof(module);
    BOOL found = FALSE;
    if (Module32First(snapshot, &module)) do {
        if (!_stricmp(module.szModule, "bugtrap.dll")) {
            printf("BUGTRAP_LOADED pid=%lu base=%p path=%s\n", gamePid, module.modBaseAddr, module.szExePath);
            found = TRUE;
            break;
        }
    } while (Module32Next(snapshot, &module));
    CloseHandle(snapshot);
    return found;
}

static void Screenshot(HWND window, const char* name)
{
    RECT rect;
    GetWindowRect(window, &rect);
    int width = rect.right-rect.left, height = rect.bottom-rect.top;
    HDC dc = GetWindowDC(window), memory = CreateCompatibleDC(dc);
    BITMAPINFO info = {0};
    info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    void* pixels = NULL;
    HBITMAP bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &pixels, NULL, 0);
    if (bitmap) {
        HGDIOBJ old = SelectObject(memory, bitmap);
        if (BitBlt(memory, 0, 0, width, height, dc, 0, 0, SRCCOPY)) {
            BITMAPFILEHEADER header = {0};
            header.bfType = 0x4D42;
            header.bfOffBits = sizeof(header) + sizeof(info.bmiHeader);
            header.bfSize = header.bfOffBits + width*height*4;
            FILE* file = fopen(name, "wb");
            if (file) {
                fwrite(&header, sizeof(header), 1, file);
                fwrite(&info.bmiHeader, sizeof(info.bmiHeader), 1, file);
                fwrite(pixels, width*height*4, 1, file);
                fclose(file);
            }
        }
        SelectObject(memory, old);
        DeleteObject(bitmap);
    }
    DeleteDC(memory);
    ReleaseDC(window, dc);
}

int main(int argc, char** argv)
{
    if (argc != 2) return 2;
    char exe[MAX_PATH];
    sprintf_s(exe, "%s\\survarium_skhid.exe", argv[1]);
    char command[4096];
    sprintf_s(command, "\"%s\" -localization=english -server=127.0.0.1:9 -minimum_resources_memory_size=768", exe);
    STARTUPINFO start = {sizeof(start)};
    PROCESS_INFORMATION process = {0};
    if (!CreateProcess(exe, command, NULL, NULL, FALSE, 0, NULL, argv[1], &start, &process)) {
        printf("CREATE_PROCESS_FAILED error=%lu\n", GetLastError()); return 3;
    }
    gamePid = process.dwProcessId;
    printf("GAME_STARTED pid=%lu exe=%s\n", gamePid, exe); fflush(stdout);
    BOOL loaded = FALSE;
    for (int i=0; i<900; ++i) {
        if (WaitForSingleObject(process.hProcess, 100) == WAIT_OBJECT_0) break;
        if (!loaded) loaded = FindBugTrap();
        EnumWindows(FindWindows, 0);
        if (gameWindow) break;
        if (crashDialog) break;
    }
    if (!gameWindow || crashDialog) {
        printf("STARTUP_FAILED loaded=%d window=%p crash_dialog=%p\n", loaded, gameWindow, crashDialog);
        TerminateProcess(process.hProcess, 3); return 4;
    }
    printf("GAME_WINDOW_READY hwnd=%p; allowing 15 seconds for startup\n", gameWindow); fflush(stdout);
    if (WaitForSingleObject(process.hProcess, 15000) != WAIT_TIMEOUT) {
        puts("GAME_EXITED_BEFORE_INJECTION"); return 5;
    }
    Screenshot(gameWindow, "game-before-crash.bmp");
    char eventName[80];
    sprintf_s(eventName, "Local\\SRP_026_CrashTest_%lu", gamePid);
    HANDLE event = OpenEventA(EVENT_MODIFY_STATE, FALSE, eventName);
    if (!event) {
        printf("CRASH_EVENT_FAILED error=%lu\n", GetLastError());
        TerminateProcess(process.hProcess, 5); return 7;
    }
    SetEvent(event);
    CloseHandle(event);
    printf("CRASH_TRIGGERED pid=%lu access=null-write value=0x26\n", gamePid); fflush(stdout);
    for (int i=0; i<600; ++i) {
        EnumWindows(FindWindows, 0);
        if (crashDialog || WaitForSingleObject(process.hProcess, 100) == WAIT_OBJECT_0) break;
    }
    int result = 8;
    if (crashDialog) {
        WCHAR text[33000]; char utf8[99000];
        GetDlgItemTextW(crashDialog, IDC_INTRO2, text, 33000);
        WideCharToMultiByte(CP_UTF8, 0, text, -1, utf8, sizeof(utf8), NULL, NULL);
        printf("CRASH_DIALOG %s\n", utf8);
        BOOL discord = IsWindowEnabled(GetDlgItem(crashDialog, IDC_SUBMIT_BUG));
        printf("DISCORD_BUTTON=%d\n", discord);
        Sleep(500);
        Screenshot(crashDialog, "crash-dialog.bmp");
        PostMessage(crashDialog, WM_COMMAND, IDCANCEL, 0);
        puts("CRASH_DIALOG_CLOSED"); fflush(stdout);
        result = discord ? 0 : 9;
    } else puts("NO_CRASH_DIALOG");
    if (WaitForSingleObject(process.hProcess, 15000) != WAIT_OBJECT_0) {
        puts("GAME_DID_NOT_EXIT; terminating only the test process");
        TerminateProcess(process.hProcess, 6);
        result = 10;
    }
    DWORD exitCode;
    GetExitCodeProcess(process.hProcess, &exitCode);
    printf("GAME_EXIT_CODE=0x%08lx\n", exitCode);
    CloseHandle(process.hThread); CloseHandle(process.hProcess);
    return result;
}
