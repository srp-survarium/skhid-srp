// A real, unhandled crash for the text report (tests/check_report.py): a null
// write two calls deep, or an MSVC-style C++ throw of std::bad_alloc that
// nothing catches, or (from the review of PR 459) a null write with the
// stack 400 KB deep, ESP 4-aligned or not ("deep", "misaligned"). It prints where its functions are, so the report's
// module+offset and stack can be checked. Builds with MinGW (no __try).
// Only for a disposable test profile: it redirects that profile's Documents.
#include <windows.h>
#include <stdio.h>
#include <wchar.h>
#include <malloc.h>

#ifdef _MSC_VER
#define NOINLINE __declspec(noinline)
#else
#define NOINLINE __attribute__((noinline, noipa))
#endif

// Read at run time, so the compiler cannot see the write is to null.
static volatile DWORD* volatile nowhere = NULL;

static void Print(const char* name, void* function)
{
    printf("%s_rva=%lu\n", name, (unsigned long)((BYTE*)function - (BYTE*)GetModuleHandleW(NULL)));
    fflush(stdout);
}

NOINLINE static void Fault(volatile DWORD* target)
{
    *target = 0x5A5A;
}

// The fake MSVC throw: ThrowInfo -> CatchableTypeArray -> CatchableType
// -> TypeDescriptor, as a 32-bit MSVC program lays them out.
struct TypeDescriptor { const void* vtable; void* spare; char name[24]; };
struct CatchableType { DWORD properties; TypeDescriptor* type; int pmd[3]; int size; void* copy; };
struct CatchableTypeArray { int count; CatchableType* types[1]; };
struct ThrowInfo { DWORD attributes; void* unwind; void* forward; CatchableTypeArray* types; };
static TypeDescriptor badAlloc = {NULL, NULL, ".?AVbad_alloc@std@@"};
static CatchableType catchable = {0, &badAlloc, {0, -1, 0}, 4, NULL};
static CatchableTypeArray catchables = {1, {&catchable}};
static ThrowInfo throwInfo = {0, NULL, NULL, &catchables};

NOINLINE static void Throw()
{
    static int object;
    ULONG_PTR args[3] = {0x19930520, (ULONG_PTR)&object, (ULONG_PTR)&throwInfo};
    RaiseException(0xE06D7363, EXCEPTION_NONCONTINUABLE, 3, args);
}

NOINLINE static int Caller(int mode, volatile DWORD* target)
{
    if (mode) Throw(); else Fault(target);
    return GetTickCount() != 0; // no tail call: the return address stays on the stack
}

// The game's breadcrumb ring (srp-026g vostok/breadcrumbs.rs) in a `.srpx`
// section as the game's arena has it, after other words: a header naming its
// own address, a match, and three events, the last 1.5 s before the crash
// (the counter's ticks: QueryPerformanceFrequency's, read at the start).
#ifdef _MSC_VER
#pragma section(".srpx", read, write)
__declspec(allocate(".srpx")) __declspec(align(64))
#else
__attribute__((section(".srpx"), aligned(64)))
#endif
static volatile unsigned int arena[16 * 4 + 16 + 8 * 1024];

static void Breadcrumbs()
{
    LARGE_INTEGER now, frequency;
    QueryPerformanceCounter(&now);
    QueryPerformanceFrequency(&frequency);
    // Words before the ring that are no header: an address, then not it xor "BRCS".
    arena[0] = (unsigned int)(ULONG_PTR)&arena[0];
    arena[1] = 0x12345678;
    volatile unsigned int* h = &arena[16 * 4];
    unsigned int at = (unsigned int)(ULONG_PTR)h;
    unsigned long long counter = (unsigned long long)now.QuadPart, f = (unsigned long long)frequency.QuadPart;
    h[2] = 1 | 32 << 16;                         // version, entry size
    h[3] = 1024;                                  // capacity
    h[4] = 3;                                     // next
    h[6] = (unsigned int)f; h[7] = (unsigned int)(f >> 32);
    h[8] = 4242; h[9] = 12 | 8 << 8 | 5 << 16;    // match 4242, map 12, 8 places, level 5
    h[10] = 1u << 31;                             // flags: a match, live, not Blackout
    h[11] = 3 + 1;                                // local place 3
    struct { unsigned int kind_a_b; unsigned long long before; unsigned int v[4]; } e[3] = {
        {1 | 0 << 8 | 5 << 16, 3 * f, {4242, 12, 8, 0}},
        {2 | 3 << 8 | 4 << 16, 2 * f, {4500, 900, 0, 0}},
        {7 | 4 << 8, f + f / 2, {2, 61000, 0, 0}},
    };
    for (unsigned n = 0; n < 3; ++n) {
        volatile unsigned int* w = h + 16 + 8 * n;
        unsigned long long c = counter - e[n].before;
        w[1] = e[n].kind_a_b; w[2] = (unsigned int)c; w[3] = (unsigned int)(c >> 32);
        for (unsigned i = 0; i < 4; ++i) w[4 + i] = e[n].v[i];
        w[0] = n + 1;
    }
    h[1] = at ^ 0x53435242;
    h[0] = at;
}

int wmain(int argc, wchar_t** argv)
{
    if (argc != 3) return 2;
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
    Breadcrumbs();
    HMODULE dll = LoadLibraryW(L"BugTrap.dll");
    if (!dll) return 4;
    typedef void (WINAPI *SetValue)(DWORD);
    ((SetValue)GetProcAddress(dll, "BT_SetActivityType"))(0); // no dialog
    Print("fault", (void*)Fault);
    Print("throw", (void*)Throw);
    Print("caller", (void*)Caller);
    int misaligned = !wcscmp(argv[2], L"misaligned");
    if (misaligned || !wcscmp(argv[2], L"deep")) {
        // More than the scan's 256 KiB below the stack's top: with ESP not
        // 4-aligned, the scan's bound is not either.
        volatile char* deep = (volatile char*)_alloca(400 * 1024);
        deep[0] = 1;
#ifdef _MSC_VER
        if (misaligned) __asm { sub esp, 2 }
        __asm { xor eax, eax }
        __asm { mov dword ptr [eax], 0x5A5A }
#else
        if (misaligned)
            __asm__ volatile("subl $2, %%esp\n\tmovl $0, %%eax\n\tmovl $0x5A5A, (%%eax)" ::: "eax", "memory");
        else
            __asm__ volatile("movl $0, %%eax\n\tmovl $0x5A5A, (%%eax)" ::: "eax", "memory");
#endif
    }
    return Caller(!wcscmp(argv[2], L"cpp"), nowhere) ? 5 : 6;
}
