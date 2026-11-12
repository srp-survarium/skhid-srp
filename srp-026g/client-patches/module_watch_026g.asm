; Patch 125 (the module report, rust/patches/src/module_report.rs,
; docs/notes/module-report.md): the two functions Windows calls for it. The
; logic is the Rust patch's; these are its entries' callers, because Windows
; calls them stdcall and a generated entry returns with a bare `ret`.
;
; - module_watch_tls: the exe's TLS callback (PIMAGE_TLS_CALLBACK: the
;   module, the reason, reserved). The build points the TLS directory's
;   AddressOfCallBacks at module_watch_callbacks (the exe had no callback: its
;   list at 0x884AA4 is the terminator alone). Windows calls it with the
;   loader's lock held: on the main thread before the exe's entry point
;   (reason 1), then on each new thread before that thread's start (2), on
;   one that ends (3), and when the process ends (0).
; - module_watch_dll: ntdll's DLL notification (LDR_DLL_NOTIFICATION_FUNCTION:
;   the reason, the data, the context), which the patch registers at the
;   first call above. Its address is handed to the patch as an argument, so
;   no Rust names it.
;
; Each generated entry saves and restores the whole machine (flags,
; registers, FPU/SSE), so Windows sees none of it changed but the stack
; words below ESP.
bits 32
section .text

extern module_report_on_thread
extern module_report_on_dll
extern module_report_on_win_event

global module_watch_tls
module_watch_tls:
    push module_watch_win_event ; our window event callback, handed to Rust
    push module_watch_dll       ; our DLL notification, handed to Rust
    call module_report_on_thread
    lea esp, [esp + 8]          ; drop both; the flags as they were
    ret 12

global module_watch_dll
module_watch_dll:
    call module_report_on_dll
    ret 12

; - module_watch_win_event: user32's event callback (WINEVENTPROC: the hook,
;   the event, the window, idObject, idChild, idEventThread, dwmsEventTime),
;   handed to SetWinEventHook out of context. Windows calls it stdcall on the
;   thread that registered the hook (the game's main thread, which pumps
;   messages); the generated entry returns with a bare ret, so this caller
;   pops the seven arguments.
global module_watch_win_event
module_watch_win_event:
    call module_report_on_win_event
    ret 28

; The TLS directory's callback list (the block module_watch.data): ours, then
; the terminator.
section .data
align 4
global module_watch_callbacks
module_watch_callbacks:
    dd module_watch_tls
    dd 0
