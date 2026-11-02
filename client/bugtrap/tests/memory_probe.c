// What memory a dump holds (check_memory.py). Three heap objects, each with
// a 32-byte marker written at run time (no copy in the EXE or on a stack):
//   register: only in the crash context's ESI, as for `mov eax, [esi+0x116]`
//             on an object whose field is null;
//   stack:    only in a local of this thread's stack;
//   heap:     referenced from nowhere.
// A player's build asks for the first two (Windows' dbghelp writes them;
// Wine's does not), a full-memory build holds all three.
// Builds with cl, or with MinGW as below (README.md, Verify).
#include <windows.h>
#include <stdio.h>

typedef LONG(WINAPI *SehFilter)(PEXCEPTION_POINTERS);
typedef void(WINAPI *SetDword)(DWORD);

static CONTEXT context;
static EXCEPTION_RECORD record;
static EXCEPTION_POINTERS pointers = {&record, &context};

// "SRP-MEMORY-PROBE-<name>", padded with '#' to 32 bytes, at +0x100.
static char *object(const char *name)
{
    char *p = (char *)HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, 4096);
    _snprintf(p + 0x100, 33, "%c%c%c-MEMORY-PROBE-%s################################", 'S', 'R', 'P',
              name);
    return p;
}

static __declspec(noinline) void setup(void)
{
    object("heap"); // leaked: no pointer to it is kept
    RtlCaptureContext(&context);
    context.Esi = (DWORD)(ULONG_PTR)object("register");
    record.ExceptionCode = EXCEPTION_ACCESS_VIOLATION;
    record.ExceptionAddress = (PVOID)(ULONG_PTR)context.Eip;
    record.NumberParameters = 2;
    record.ExceptionInformation[1] = 0x116;
}

static int run(void)
{
    HMODULE bugtrap = LoadLibraryA("BugTrap.dll");
    if (!bugtrap) return 90;
    SehFilter filter = (SehFilter)GetProcAddress(bugtrap, "BT_SehFilter");
    SetDword activity = (SetDword)GetProcAddress(bugtrap, "BT_SetActivityType");
    SetDword exitMode = (SetDword)GetProcAddress(bugtrap, "BT_SetExitMode");
    if (!filter || !activity || !exitMode) return 91;
    activity(2); // BTA_SAVEREPORT: no dialog
    exitMode(2); // BTEM_EXECUTEHANDLER
    char *volatile stack = object("stack");
    setup();
    LONG result = filter(&pointers);
    // The stack local stays live (on the stack) past the capture.
    return result == EXCEPTION_EXECUTE_HANDLER && stack[0x100] == 'S' ? 0 : 92;
}

#ifdef PROBE_NO_CRT
// MinGW without its C runtime start-up (Nix's links a thread library):
// i686-w64-mingw32-gcc -O2 -DPROBE_NO_CRT -nostdlib -Wl,-e,_probe_start@0
//   tests/memory_probe.c -lmsvcrt -lkernel32 -o memory_probe.exe
void WINAPI probe_start(void)
{
    ExitProcess(run());
}
#else
int main(void)
{
    return run();
}
#endif
