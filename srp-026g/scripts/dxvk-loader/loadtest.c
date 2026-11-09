/* Preloads DXVK's DLLs from dxvk\ by their full paths, then asks for each
   plain name as the game does (its delay-loaded imports: the helper's
   LoadLibraryExA(name, NULL, 0); and a delay-loaded call of D3D11CreateDevice
   when built with the delay import library), and prints which file serves it. */
#include <windows.h>
#include <psapi.h>
#include <stdio.h>
#include <string.h>

typedef HRESULT (WINAPI *create_device)(void *, int, HMODULE, UINT, const void *, UINT, UINT, void **, void *, void **);
#ifdef DELAYED
HRESULT WINAPI D3D11CreateDevice(void *, int, HMODULE, UINT, const void *, UINT, UINT, void **, void *, void **);
#endif

static const char *path_of(HMODULE module, char *out) {
    if (!module || !GetModuleFileNameA(module, out, MAX_PATH)) strcpy(out, "(none)");
    return out;
}

int main(void) {
    char dir[MAX_PATH], path[2 * MAX_PATH], seen[MAX_PATH];
    GetModuleFileNameA(NULL, dir, MAX_PATH);
    *strrchr(dir, '\\') = 0;
    const char *order[] = {"dxgi.dll", "d3d11.dll", "d3d10core.dll", "d3d9.dll", "d3d8.dll"};
    HMODULE ours[5];
#ifdef CONFIG
    /* as patch 83 does: the process's own Win32 environment, before DXVK's DLLs load */
    printf("SetEnvironmentVariableA(DXVK_CONFIG) -> %d\n", SetEnvironmentVariableA("DXVK_CONFIG", CONFIG));
#endif
    for (int i = 0; i < 5; i++) {
        snprintf(path, sizeof path, "%s\\dxvk\\%s", dir, order[i]);
        ours[i] = LoadLibraryA(path);
        printf("preload %-14s -> %s (error %lu)\n", order[i], path_of(ours[i], seen), ours[i] ? 0 : GetLastError());
    }
    int same = 1;
    for (int i = 0; i < 5; i++) {
        HMODULE plain = LoadLibraryExA(order[i], NULL, 0);
        printf("plain   %-14s -> %s %s\n", order[i], path_of(plain, seen), plain == ours[i] ? "(the preloaded one)" : "(ANOTHER)");
        same &= plain == ours[i];
    }
    /* d3d11's own import of dxgi.dll: the one module named dxgi.dll */
    HMODULE modules[1024]; DWORD bytes; int count[5] = {0};
    EnumProcessModules(GetCurrentProcess(), modules, sizeof modules, &bytes);
    for (DWORD m = 0; m < bytes / sizeof(HMODULE); m++) {
        char base[MAX_PATH];
        GetModuleBaseNameA(GetCurrentProcess(), modules[m], base, MAX_PATH);
        for (int i = 0; i < 5; i++) if (!_stricmp(base, order[i])) count[i]++;
    }
    for (int i = 0; i < 5; i++) {
        printf("modules named %-14s: %d\n", order[i], count[i]);
        same &= count[i] == 1;
    }
#ifdef DELAYED
    /* the delay-loaded import, called: its helper resolves d3d11.dll then */
    void *device = NULL;
    HRESULT hr = D3D11CreateDevice(NULL, 1 /* hardware */, NULL, 0, NULL, 0, 7, &device, NULL, NULL);
    create_device direct = (create_device)GetProcAddress(ours[1], "D3D11CreateDevice");
    printf("delay-loaded D3D11CreateDevice: hr 0x%08lx; d3d11.dll now %s\n", (unsigned long)hr,
           path_of(GetModuleHandleA("d3d11.dll"), seen));
    same &= GetModuleHandleA("d3d11.dll") == ours[1] && direct != NULL;
#endif
    printf("RESULT %s\n", same ? "pass" : "fail");
    return same ? 0 : 1;
}
