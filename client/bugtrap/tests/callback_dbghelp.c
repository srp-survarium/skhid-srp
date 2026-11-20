// Test-only dbghelp.dll for check_memory.py: writes the dump type BugTrap
// asks for and the extra memory ranges its callback gives to
// dbghelp-request.txt, then forwards to the system's MiniDumpWriteDump
// (Wine's ignores memory callbacks, Windows' honours them).
// i686-w64-mingw32-gcc -O2 -D__USE_MINGW_ANSI_STDIO=0 -shared -nostdlib -Wl,--kill-at
//   -Wl,-e,_DllMainCRTStartup@12
//   tests/callback_dbghelp.c -lkernel32 -lmsvcrt -o callback_dbghelp.dll
#include <windows.h>
#include <dbghelp.h>
#include <stdio.h>

__declspec(dllexport) BOOL WINAPI MiniDumpWriteDump(HANDLE process, DWORD pid, HANDLE file,
    MINIDUMP_TYPE type, PMINIDUMP_EXCEPTION_INFORMATION exception,
    PMINIDUMP_USER_STREAM_INFORMATION streams, PMINIDUMP_CALLBACK_INFORMATION callback)
{
    FILE *out = fopen("dbghelp-request.txt", "w");
    if (!out) return FALSE;
    fprintf(out, "type=%lu\n", (unsigned long)type);
    if (callback && callback->CallbackRoutine) {
        for (int i = 0; i < 64; i++) {
            MINIDUMP_CALLBACK_INPUT input = {0};
            MINIDUMP_CALLBACK_OUTPUT output = {0};
            input.ProcessId = pid;
            input.ProcessHandle = process;
            input.CallbackType = MemoryCallback;
            if (!callback->CallbackRoutine(callback->CallbackParam, &input, &output)) break;
            fprintf(out, "range=%llu+%lu\n", (unsigned long long)output.MemoryBase,
                    (unsigned long)output.MemorySize);
        }
    }
    fclose(out);
    WCHAR path[MAX_PATH];
    if (!GetSystemDirectoryW(path, MAX_PATH)) return FALSE;
    lstrcatW(path, L"\\dbghelp.dll");
    HMODULE real = LoadLibraryW(path);
    if (!real) return FALSE;
    typedef BOOL(WINAPI * Writer)(HANDLE, DWORD, HANDLE, MINIDUMP_TYPE, PMINIDUMP_EXCEPTION_INFORMATION,
                                  PMINIDUMP_USER_STREAM_INFORMATION, PMINIDUMP_CALLBACK_INFORMATION);
    Writer writer = (Writer)GetProcAddress(real, "MiniDumpWriteDump");
    // The ranges are given: the real writer runs without the callback.
    return writer && (void *)writer != (void *)MiniDumpWriteDump &&
           writer(process, pid, file, type, exception, streams, NULL);
}

BOOL WINAPI DllMainCRTStartup(HINSTANCE instance, DWORD reason, LPVOID reserved)
{
    return TRUE;
}
