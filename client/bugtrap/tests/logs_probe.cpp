// Standalone: MinGW or MSVC, no BugTrap import library needed.
#include <windows.h>
#include <stdio.h>
#include <wchar.h>
#include <string.h>
static BY_HANDLE_FILE_INFORMATION target;
static unsigned reads;
static unsigned failAfter = 1;
static BOOL WINAPI FailLogRead(HANDLE file, LPVOID data, DWORD size, LPDWORD got, LPOVERLAPPED overlap) {
    BY_HANDLE_FILE_INFORMATION info;
    if (GetFileInformationByHandle(file, &info) && info.dwVolumeSerialNumber == target.dwVolumeSerialNumber
        && info.nFileIndexLow == target.nFileIndexLow && info.nFileIndexHigh == target.nFileIndexHigh
        && reads++ >= failAfter) {
        *got = 0;
        SetLastError(ERROR_READ_FAULT);
        return FALSE;
    }
    return ReadFile(file, data, size, got, overlap);
}
static bool InjectReadFailure(HMODULE module) {
    BYTE* base = (BYTE*)module;
    IMAGE_NT_HEADERS* nt = (IMAGE_NT_HEADERS*)(base + ((IMAGE_DOS_HEADER*)base)->e_lfanew);
    IMAGE_IMPORT_DESCRIPTOR* imports = (IMAGE_IMPORT_DESCRIPTOR*)(base +
        nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT].VirtualAddress);
    for (; imports->Name; ++imports) {
        IMAGE_THUNK_DATA* names = (IMAGE_THUNK_DATA*)(base + imports->OriginalFirstThunk);
        IMAGE_THUNK_DATA* slots = (IMAGE_THUNK_DATA*)(base + imports->FirstThunk);
        for (; names->u1.AddressOfData; ++names, ++slots) {
            if (IMAGE_SNAP_BY_ORDINAL(names->u1.Ordinal)) continue;
            IMAGE_IMPORT_BY_NAME* name = (IMAGE_IMPORT_BY_NAME*)(base + names->u1.AddressOfData);
            if (strcmp((char*)name->Name, "ReadFile")) continue;
            DWORD old;
            if (!VirtualProtect(&slots->u1.Function, sizeof(ULONG_PTR), PAGE_READWRITE, &old)) return false;
            slots->u1.Function = (ULONG_PTR)FailLogRead;
            VirtualProtect(&slots->u1.Function, sizeof(ULONG_PTR), old, &old);
            return true;
        }
    }
    return false;
}
int wmain(int argc, wchar_t** argv) {
    if (argc != 4) return 2;
    // Only run in a disposable test profile: redirect its Documents folder.
    const wchar_t* keys[] = {
        L"Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\User Shell Folders",
        L"Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Shell Folders"
    };
    for (unsigned i = 0; i < 2; ++i) {
        HKEY key;
        if (RegCreateKeyExW(HKEY_CURRENT_USER, keys[i], 0, NULL, 0, KEY_SET_VALUE,
                            NULL, &key, NULL)) return 3;
        LONG error = RegSetValueExW(key, L"Personal", 0, REG_SZ,
            (const BYTE*)argv[1], (DWORD)(wcslen(argv[1]) + 1) * sizeof(wchar_t));
        RegCloseKey(key);
        if (error) return 3;
    }
    HMODULE dll = LoadLibraryW(L"BugTrap.dll");
    if (!dll) return 4;
    typedef BOOL (WINAPI *Snapshot)(LPCSTR);
    Snapshot snapshot = (Snapshot)GetProcAddress(dll, "BT_SaveSnapshot");
    if (!snapshot) return 5;
    HANDLE log = INVALID_HANDLE_VALUE;
    if (wcscmp(argv[2], L"none")) {
        DWORD share = wcscmp(argv[2], L"locked") ? FILE_SHARE_READ | FILE_SHARE_WRITE : 0;
        log = CreateFileW(argv[3], GENERIC_READ | GENERIC_WRITE, share, NULL, OPEN_EXISTING, 0, NULL);
        if (log == INVALID_HANDLE_VALUE) return 6;
    }
    bool inject = !wcscmp(argv[2], L"readfail") || !wcscmp(argv[2], L"tailfail") || !wcscmp(argv[2], L"headfail");
    if (!wcscmp(argv[2], L"tailfail")) failAfter = 2;
    if (!wcscmp(argv[2], L"headfail")) failAfter = 0;
    if (inject) {
        if (!GetFileInformationByHandle(log, &target) || !InjectReadFailure(dll)) return 8;
    }
    BOOL ok = snapshot(NULL);
    if (inject && reads <= failAfter) return 9;
    if (log != INVALID_HANDLE_VALUE) CloseHandle(log);
    return ok ? 0 : 7;
}
