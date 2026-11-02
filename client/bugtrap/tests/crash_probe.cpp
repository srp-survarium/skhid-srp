#include <windows.h>
#include <dbghelp.h>
#include <stdio.h>
#include <string.h>
#include "bugtrap_abi.h"

static BOOL expectFailure;
static BOOL testDiscord;
static LONG browserCalls;
static HANDLE callbackRelease;
static HANDLE secondEntered;

static void CALLBACK HoldAfterDump(INT_PTR)
{
    // Another crash must wait for the file, but never for callbacks or the UI.
    if (WaitForSingleObject(callbackRelease, 10000) != WAIT_OBJECT_0)
        ExitProcess(81);
}

static DWORD WINAPI FirstCrash(PVOID)
{
    printf("fault_thread=%lu\n", GetCurrentThreadId());
    fflush(stdout);
    BT_CallCppFilter();
    return 0;
}

static LONG SecondFilter(EXCEPTION_POINTERS* ep)
{
    SetEvent(secondEntered);
    LONG result = BT_SehFilter(ep);
    if (result != EXCEPTION_EXECUTE_HANDLER) ExitProcess(82);
    return result;
}

static DWORD WINAPI SecondCrash(PVOID)
{
    __try { *(volatile DWORD*)0 = 43; }
    __except(SecondFilter(GetExceptionInformation())) { return 0; }
    return 83;
}

static int ConcurrentCrashes()
{
    char name[100];
    sprintf_s(name, "Local\\SRPProbeWriterStarted-%lu", GetCurrentProcessId());
    HANDLE started = CreateEventA(NULL, TRUE, FALSE, name);
    sprintf_s(name, "Local\\SRPProbeWriterRelease-%lu", GetCurrentProcessId());
    HANDLE release = CreateEventA(NULL, TRUE, FALSE, name);
    callbackRelease = CreateEvent(NULL, TRUE, FALSE, NULL);
    secondEntered = CreateEvent(NULL, TRUE, FALSE, NULL);
    BT_SetPreErrHandler(HoldAfterDump, 0);
    HANDLE first = CreateThread(NULL, 0, FirstCrash, NULL, 0, NULL);
    if (WaitForSingleObject(started, 10000) != WAIT_OBJECT_0) return 84;
    HANDLE second = CreateThread(NULL, 0, SecondCrash, NULL, 0, NULL);
    if (WaitForSingleObject(secondEntered, 10000) != WAIT_OBJECT_0) return 85;
    if (WaitForSingleObject(second, 250) != WAIT_TIMEOUT) return 86;
    SetEvent(release);
    if (WaitForSingleObject(second, 10000) != WAIT_OBJECT_0) return 87;
    // First remains blocked in its callback; second has already returned.
    if (WaitForSingleObject(first, 0) != WAIT_TIMEOUT) return 88;
    SetEvent(callbackRelease);
    if (WaitForSingleObject(first, 10000) != WAIT_OBJECT_0) return 89;
    puts("concurrent_crash_waited_for_dump_only");
    return 0;
}

static void CALLBACK RecursiveCapture(INT_PTR)
{
    // Exercise recursion on the report worker after persistence. Neither call
    // may wait on its own worker or overwrite the primary request.
    if (BT_SaveSnapshot(NULL) || GetLastError() != ERROR_BUSY) ExitProcess(75);
    CONTEXT context = {0};
    EXCEPTION_RECORD record = {0};
    EXCEPTION_POINTERS ep = { &record, &context };
    record.ExceptionCode = EXCEPTION_ACCESS_VIOLATION;
    if (BT_SehFilter(&ep) != EXCEPTION_CONTINUE_SEARCH) ExitProcess(76);
    puts("recursive_capture_rejected");
    fflush(stdout);
}

static DWORD WINAPI Snapshot(PVOID)
{
    if (!BT_SaveSnapshot(NULL)) ExitProcess(70);
    puts("snapshot_saved");
    return 0;
}

static DWORD WINAPI CrashDuringSnapshot(PVOID)
{
    printf("fault_thread=%lu\n", GetCurrentThreadId());
    fflush(stdout);
    return SecondCrash(NULL);
}

static int SnapshotOverlap()
{
    char name[100];
    sprintf_s(name, "Local\\SRPProbeWriterStarted-%lu", GetCurrentProcessId());
    HANDLE started = CreateEventA(NULL, TRUE, FALSE, name);
    sprintf_s(name, "Local\\SRPProbeWriterRelease-%lu", GetCurrentProcessId());
    HANDLE release = CreateEventA(NULL, TRUE, FALSE, name);
    secondEntered = CreateEvent(NULL, TRUE, FALSE, NULL);
    // A snapshot must not run this callback; the subsequent crash must.
    BT_SetPreErrHandler(RecursiveCapture, 0);
    HANDLE snapshot = CreateThread(NULL, 0, Snapshot, NULL, 0, NULL);
    if (WaitForSingleObject(started, 10000) != WAIT_OBJECT_0) return 71;
    HANDLE crash = CreateThread(NULL, 0, CrashDuringSnapshot, NULL, 0, NULL);
    if (WaitForSingleObject(secondEntered, 10000) != WAIT_OBJECT_0) return 72;
    if (WaitForSingleObject(crash, 250) != WAIT_TIMEOUT) return 73;
    SetEvent(release);
    if (WaitForSingleObject(snapshot, 10000) != WAIT_OBJECT_0) return 74;
    if (WaitForSingleObject(crash, 10000) != WAIT_OBJECT_0) return 79;
    puts("snapshot_overlap_completed");
    return 0;
}

static int SyntheticMode(const char* mode)
{
    void (__cdecl *entry)() = strstr(mode, "seh") ? BT_CallSehFilter :
        strstr(mode, "net") ? BT_CallNetFilter : BT_CallCppFilter;
    BOOL propagate = !strncmp(mode, "continue-", 9);
    BT_SetExitMode(propagate ? BTEM_CONTINUESEARCH : BTEM_EXECUTEHANDLER);
    printf("fault_thread=%lu\n", GetCurrentThreadId());
    fflush(stdout);
    __try { entry(); }
    __except(EXCEPTION_EXECUTE_HANDLER)
    {
        if (!propagate || GetExceptionCode() != EXCEPTION_ACCESS_VIOLATION) return 77;
        puts("outer_handler_reached");
        return 0;
    }
    if (propagate) return 78;
    puts("synthetic_returned");
    return 0;
}

static HINSTANCE WINAPI BrowserProbe(HWND, LPCWSTR verb, LPCWSTR url,
                                    LPCWSTR parameters, LPCWSTR directory, INT)
{
    if (wcscmp(verb, L"open") || wcscmp(url, L"https://discord.gg/QUmJg3gBhg") ||
        parameters || directory) ExitProcess(95);
    InterlockedIncrement(&browserCalls);
    return (HINSTANCE)33;
}

static BOOL InterceptBrowser()
{
    // Intercept only this DLL's browser import. Exercise its button handler
    // without launching a real browser or contacting Discord from the test.
    BYTE* module = (BYTE*)GetModuleHandleA("BugTrap.dll");
    IMAGE_DOS_HEADER* dos = (IMAGE_DOS_HEADER*)module;
    IMAGE_NT_HEADERS* pe = (IMAGE_NT_HEADERS*)(module + dos->e_lfanew);
    IMAGE_IMPORT_DESCRIPTOR* import = (IMAGE_IMPORT_DESCRIPTOR*)(module +
        pe->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT].VirtualAddress);
    for (; import->Name; ++import) {
        if (_stricmp((char*)module + import->Name, "shell32.dll")) continue;
        IMAGE_THUNK_DATA* name = (IMAGE_THUNK_DATA*)(module + import->OriginalFirstThunk);
        IMAGE_THUNK_DATA* slot = (IMAGE_THUNK_DATA*)(module + import->FirstThunk);
        for (; name->u1.AddressOfData; ++name, ++slot) {
            if (IMAGE_SNAP_BY_ORDINAL(name->u1.Ordinal)) continue;
            IMAGE_IMPORT_BY_NAME* symbol = (IMAGE_IMPORT_BY_NAME*)(module + name->u1.AddressOfData);
            if (strcmp((char*)symbol->Name, "ShellExecuteW")) continue;
            DWORD old;
            if (!VirtualProtect(&slot->u1.Function, sizeof(DWORD), PAGE_READWRITE, &old)) return FALSE;
            slot->u1.Function = (DWORD)BrowserProbe;
            return VirtualProtect(&slot->u1.Function, sizeof(DWORD), old, &old);
        }
    }
    return FALSE;
}

static BOOL CALLBACK CloseDialog(HWND window, LPARAM)
{
    DWORD process;
    GetWindowThreadProcessId(window, &process);
    if (process != GetCurrentProcessId() || !GetDlgItem(window, IDC_INTRO2))
        return TRUE;
    WCHAR text[33000];
    // Dialog text follows Windows' UI language, independently of the game.
    if (!GetDlgItemTextW(window, IDC_INTRO2, text, 33000))
        return TRUE;
    if (!!IsWindowVisible(GetDlgItem(window, 1034)) == !!expectFailure)
        ExitProcess(99);
    if (!IsWindowEnabled(GetDlgItem(window, IDC_SUBMIT_BUG)))
        ExitProcess(92);
    if (testDiscord) {
        if (browserCalls != 0) ExitProcess(96);
        SendMessage(window, WM_COMMAND, IDC_SUBMIT_BUG, 0);
        if (browserCalls != 1 || !IsWindow(window)) ExitProcess(97);
        puts("discord_invite_opened_dialog_kept");
    }
    puts(expectFailure ? "failed_dump_dialog" : "closed_dialog");
    fflush(stdout);
    PostMessage(window, WM_COMMAND, IDCANCEL, 0);
    return FALSE;
}

static DWORD WINAPI DismissUI(PVOID)
{
    for (int attempt = 0; attempt < 200; ++attempt)
    {
        if (!EnumWindows(CloseDialog, 0))
            return 0;
        Sleep(100);
    }
    ExitProcess(91);
    return 0;
}

static void CALLBACK StopBeforeUI(INT_PTR)
{
    // A broken callback must not prevent the primary crash dump from surviving.
    ExitProcess(23);
}

__declspec(noinline) static void Overflow(unsigned depth)
{
    volatile char buffer[4096];
    memset((void*)buffer, (int)depth, sizeof(buffer));
    Overflow(depth + 1);
    printf("%d", buffer[depth % sizeof(buffer)]);
}

static DWORD WINAPI Crash(PVOID mode)
{
    ULONG guarantee = 64 * 1024;
    SetThreadStackGuarantee(&guarantee);
    printf("fault_thread=%lu\n", GetCurrentThreadId());
    fflush(stdout);
    if (!strcmp((const char*)mode, "stack"))
    {
        __try { Overflow(0); }
        __except ((BT_SehFilter(GetExceptionInformation()), EXCEPTION_EXECUTE_HANDLER))
        { ExitProcess(0); }
    }
    else
        *(volatile DWORD*)0 = 42;
    return 0;
}

int main(int argc, char** argv)
{
    const char* mode = argc > 1 ? argv[1] : "silent";
    if (argc > 2)
    {
        // Wine initializes TEMP/TMP itself; set these inside the Windows process.
        SetEnvironmentVariableA("TEMP", argv[2]);
        SetEnvironmentVariableA("TMP", argv[2]);
    }
    BT_SetAppName("SRP crash probe");
    if (strcmp(BT_GetAppName(), "SRP crash probe")) return 94;
    BT_SetUserMessage("SRP probe: preserved assertion message");
    BT_SetSupportEMail("former-developer@example.invalid");
    BT_SetSupportServer("former-developer.example.invalid", 9999);
    BT_SetSupportURL("https://example.invalid/former-developer");
    if (*BT_GetSupportEMail() || *BT_GetSupportHost() || *BT_GetSupportURL())
        return 90;
    if (BT_SendSnapshot() || BT_SendSnapshotEx(NULL) || BT_MailSnapshot() || BT_MailSnapshotEx(NULL))
        return 93;
    BT_SetDumpType(MiniDumpNoDump);
    BT_SetFlags(BTF_NONE);
    BT_SetActivityType(BTA_SAVEREPORT);
    BT_SetExitMode(BTEM_EXECUTEHANDLER);
    BT_InstallSehFilter();
    if (!strncmp(mode, "continue-", 9) || !strncmp(mode, "execute-", 8))
        return SyntheticMode(mode);
    if (!strcmp(mode, "concurrent")) return ConcurrentCrashes();
    if (!strcmp(mode, "snapshot-overlap")) return SnapshotOverlap();
    if (!strcmp(mode, "recursive")) BT_SetPreErrHandler(RecursiveCapture, 0);
    expectFailure = !strcmp(mode, "failure");
    testDiscord = !strcmp(mode, "discord");
    if (testDiscord && !InterceptBrowser()) return 98;
    if (!strcmp(mode, "cancel") || expectFailure || testDiscord)
    {
        BT_SetActivityType(BTA_SHOWUI);
        CloseHandle(CreateThread(NULL, 0, DismissUI, NULL, 0, NULL));
    }
    if (!strcmp(mode, "pre-handler"))
    {
        BT_SetActivityType(BTA_SHOWUI);
        BT_SetPreErrHandler(StopBeforeUI, 0);
    }
    if (!strcmp(mode, "cpp") || !strcmp(mode, "recursive"))
    {
        printf("fault_thread=%lu\n", GetCurrentThreadId());
        fflush(stdout);
        BT_CallCppFilter();
        return 0;
    }
    if (!strcmp(mode, "thread"))
    {
        HANDLE thread = CreateThread(NULL, 0, Crash, (PVOID)mode, 0, NULL);
        WaitForSingleObject(thread, INFINITE);
        return 0;
    }
    Crash((PVOID)mode);
    return 0;
}
