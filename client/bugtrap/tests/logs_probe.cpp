// Standalone: MinGW or MSVC, no BugTrap import library needed.
#include <windows.h>
#include <stdio.h>
#include <wchar.h>
#include <string.h>
static BY_HANDLE_FILE_INFORMATION target;
static unsigned reads;
static unsigned failAfter = 1;
static bool slowRead;
static unsigned long long logStart = ~0ULL, highWater;
static bool logWrites;
static BOOL WINAPI WatchWrite(HANDLE file, LPCVOID data, DWORD size, LPDWORD written, LPOVERLAPPED overlap) {
    const BYTE* p = (const BYTE*)data;
    LARGE_INTEGER zero = {}, position;
    if (!SetFilePointerEx(file, zero, &position, FILE_CURRENT)) return FALSE;
    if (size >= 35 && !memcmp(p, "PK\3\4", 4) && !memcmp(p + 30, "logs/", 5)) {
        if (size >= 49 && !memcmp(p + 30, "logs/collection.txt", 19)) logWrites = false;
        else {
            logWrites = true;
            if (logStart == ~0ULL) logStart = position.QuadPart;
        }
    }
    if (logWrites && (unsigned long long)position.QuadPart + size > highWater)
        highWater = position.QuadPart + size;
    return WriteFile(file, data, size, written, overlap);
}
static BOOL WINAPI FailLogRead(HANDLE file, LPVOID data, DWORD size, LPDWORD got, LPOVERLAPPED overlap) {
    BY_HANDLE_FILE_INFORMATION info;
    if (GetFileInformationByHandle(file, &info) && info.dwVolumeSerialNumber == target.dwVolumeSerialNumber
        && info.nFileIndexLow == target.nFileIndexLow && info.nFileIndexHigh == target.nFileIndexHigh
        && reads++ >= failAfter) {
        if (slowRead) { Sleep(5200); return ReadFile(file, data, size, got, overlap); }
        *got = 0;
        SetLastError(ERROR_READ_FAULT);
        return FALSE;
    }
    return ReadFile(file, data, size, got, overlap);
}
static bool Hook(HMODULE module, const char* function, ULONG_PTR replacement) {
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
            if (strcmp((char*)name->Name, function)) continue;
            DWORD old;
            if (!VirtualProtect(&slots->u1.Function, sizeof(ULONG_PTR), PAGE_READWRITE, &old)) return false;
            slots->u1.Function = replacement;
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
    bool watch = !wcscmp(argv[2], L"watch");
    if (watch && !Hook(dll, "WriteFile", (ULONG_PTR)WatchWrite)) return 10;
    HANDLE log = INVALID_HANDLE_VALUE;
    if (wcscmp(argv[2], L"none") && !watch) {
        DWORD share = wcscmp(argv[2], L"locked") ? FILE_SHARE_READ | FILE_SHARE_WRITE : 0;
        log = CreateFileW(argv[3], GENERIC_READ | GENERIC_WRITE, share, NULL, OPEN_EXISTING, 0, NULL);
        if (log == INVALID_HANDLE_VALUE) return 6;
    }
    bool inject = !wcscmp(argv[2], L"readfail") || !wcscmp(argv[2], L"tailfail") || !wcscmp(argv[2], L"headfail") || !wcscmp(argv[2], L"slow");
    if (!wcscmp(argv[2], L"tailfail")) failAfter = 2;
    if (!wcscmp(argv[2], L"headfail")) failAfter = 0;
    slowRead = !wcscmp(argv[2], L"slow");
    if (slowRead) failAfter = 0;
    if (inject) {
        if (!GetFileInformationByHandle(log, &target) || !Hook(dll, "ReadFile", (ULONG_PTR)FailLogRead)) return 8;
    }
    BOOL ok = snapshot(NULL);
    if (inject && reads <= failAfter) return 9;
    if (watch) {
        if (logStart == ~0ULL || highWater < logStart || highWater - logStart > 16 * 1024 * 1024) return 11;
        printf("log_stream_high_water=%llu\n", highWater - logStart);
    }
    if (log != INVALID_HANDLE_VALUE) CloseHandle(log);
    return ok ? 0 : 7;
}
