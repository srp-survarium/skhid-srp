// A real, unhandled crash for the text report (tests/check_report.py): a null
// write two calls deep, or an MSVC-style C++ throw of std::bad_alloc that
// nothing catches. It prints where its functions are, so the report's
// module+offset and stack can be checked. Builds with MinGW (no __try).
// Only for a disposable test profile: it redirects that profile's Documents.
#include <windows.h>
#include <stdio.h>
#include <wchar.h>

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
    HMODULE dll = LoadLibraryW(L"BugTrap.dll");
    if (!dll) return 4;
    typedef void (WINAPI *SetValue)(DWORD);
    ((SetValue)GetProcAddress(dll, "BT_SetActivityType"))(0); // no dialog
    Print("fault", (void*)Fault);
    Print("throw", (void*)Throw);
    Print("caller", (void*)Caller);
    return Caller(!wcscmp(argv[2], L"cpp"), nowhere) ? 5 : 6;
}
