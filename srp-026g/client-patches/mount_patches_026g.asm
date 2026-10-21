; The game mounts resources.db's patches (build 2777, patch revision 12,
; client_patch.md section 0.2). Two entries; patch_026g.py checks the hooks'
; targets and the state address at the end, and fills `needed` (patch 49):
;   nasm -f bin -DCAVE=0xA425400 -DSTATE=0xA428800 mount_patches_026g.asm -o mount_patches_026g.section.bin
;
; The start-up mount (mount_by_config_helper, 0xE7D20) mounts one archive:
; query_db 0xE7D90 formats "../../%s.db" with "resources" and calls
; query_mount_archive 0xE7EF0 once, at 0xE7E3E. That call comes here
; (mount_all): first resources.db as the game asked, then
; ../../resources.db.patch_<n> for every n from 1 to MAX_PATCHES whose file
; is there, in that order. A patch is numbered by the patch revision that
; brought it, and not every revision has one: a missing number is skipped.
; All of them are mounted at "resources"; the file system gives a path to
; the archive mounted last that has it (hash_multiset::insert 0x45900: a
; node of a later mount goes in front), so a patch replaces files of the
; archives before it and adds new ones.
;
; Each mount calls the same callback when it is done, on_mounted_resources
; 0x147E0, in the order they were queued (the file system runs its tasks
; one after another). It keeps the mount in engine_world+2D4h
; (m_resources_mount) and goes on with the start-up, so only the last call
; may get there. Its first six bytes jump here (on_mounted): the call of
; each start-up mount takes a reference of its mount (a mount nobody holds
; is unmounted) and keeps it in the state; all but the last return. The
; last calls the game's code with resources.db's mount, which takes its own
; reference for engine_world and goes on with the start-up, as the game
; without patches; then resources.db's reference taken here goes, so
; resources.db is held by engine_world alone, as in the retail game. Each
; patch is held by the state alone, for the life of the process: the game
; never lets engine_world+2D4h go either (the engine's end, 0x15C80, ends
; in TerminateProcess with the mounts held). A call when no start-up mount
; is pending is the game's own.
;
; A patch file is "not there" when CreateFileA fails with
; ERROR_FILE_NOT_FOUND or ERROR_PATH_NOT_FOUND. Any other failure (a folder
; by that name, access denied, a sharing violation) stops the game with
; "Cannot open resources.db.patch_<n> (error <e>)". So does a patch that is
; not there but that this exe requires (patch 49): `needed`, before the
; state's address at the end, is a bitmap of n (bit n of byte n/8) that
; patch_026g.py fills at build time with the release's patches
; (--required-patches=, which release/build-zip takes from the offered-patch
; list); the checked-in .bin holds zeros, so a build without the option
; requires none. A mount that failed
; gives a null mount: for resources.db the game's own "Cannot mount
; resources", for a patch "Cannot mount resources.db.patch_<n>" (fatal
; 0x113580, which does not return). A patch that is there is never skipped.
;
; The state is in `.srpd`, the late join's writable page (STATE, at its
; 0x800). The image may be rebased: every address of the exe and of this
; code is taken plus the delta; calls and jumps into the game are relative.
BITS 32
ORG CAVE

QUERY_MOUNT_ARCHIVE equ 0xE7EF0         ; esi = allocator; five arguments, cdecl
FORMAT_PATH         equ 0x18190         ; esi = native path string; (format, ...), cdecl
FATAL               equ 0x113580        ; (format, ...), never returns
ON_MOUNTED_BODY     equ 0x147E6         ; on_mounted_resources after the six bytes
UNMOUNT             equ 0xDFE30         ; eax = a mount nobody holds: queues its unmount
CloseHandle         equ 0x870040        ; the import table's slots
CreateFileA         equ 0x870048
GetLastError        equ 0x87005C

MAX_PATCHES equ 255                     ; the patch revision is a byte
REFERENCES  equ 0x1C                    ; a mount's reference count
ERROR_FILE_NOT_FOUND equ 2
ERROR_PATH_NOT_FOUND equ 3

; The state (zeroed with the page).
REMAINING   equ 0                       ; start-up mounts whose callback has not come
SEEN        equ 4                       ; start-up mounts held so far
MOUNTS      equ 8                       ; [1 + MAX_PATCHES], resources.db's first
NUMBERS     equ MOUNTS + 4 * (1 + MAX_PATCHES) ; [MAX_PATCHES] bytes: the n of
                                        ; each patch found, in order
STATE_SIZE  equ NUMBERS + MAX_PATCHES

; mount_all's frame, above it pushad's registers, the return address and
; query_mount_archive's five arguments.
DELTA       equ 0
COUNT       equ 4                       ; patches found
INDEX       equ 8                       ; the n looked for, then the patch mounted
PATH        equ 0xC                     ; a native path string: begin, end of the
                                        ; text, end of the buffer, 260 characters,
                                        ; the separator (as query_db's)
PATH_TEXT   equ PATH + 0xC
PATH_SEPARATOR equ PATH + 0x110
FRAME       equ PATH + 0x114
SAVED_ESI   equ FRAME + 4               ; pushad's esi: the allocator
ARGS        equ FRAME + 0x24            ; virtual path, fat path, archive path,
                                        ; descriptor, callback

; on_mounted_resources: [esp+4] where the mount is kept, [esp+8] the
; start-up's next step, [esp+0Ch] the address of the mount. eax, ecx and edx
; are free, as at any function's start.
on_mounted:
    call .pc
.pc:
    pop edx
    sub edx, .pc                        ; edx = the rebase delta, 0 if none
    lea ecx, [edx+STATE]
    cmp dword [ecx+REMAINING], 0
    je game                             ; not a start-up mount
    mov eax, [esp+0xC]
    mov eax, [eax]
    test eax, eax
    jz .failed
    lock inc dword [eax+REFERENCES]     ; held here from now on
    push esi
    mov esi, [ecx+SEEN]
    mov [ecx+MOUNTS+4*esi], eax
    inc esi
    mov [ecx+SEEN], esi
    pop esi
    lock dec dword [ecx+REMAINING]
    jz .last
    ret                                 ; cdecl: the caller takes the arguments
.last:
    ; Every archive is mounted: the game's code with resources.db's mount,
    ; as without patches (it takes engine_world's reference and starts the
    ; next step), then resources.db's reference of the state goes.
    push esi
    lea esi, [ecx+MOUNTS]
    push esi                            ; &resources.db's mount
    push dword [esp+0x10]               ; the next step
    push dword [esp+0x10]               ; where it is kept
    call game                           ; cdecl; esi is kept
    add esp, 0xC
    mov eax, [esi]
    or ecx, -1
    lock xadd [eax+REFERENCES], ecx     ; as the game lets a mount go
    dec ecx
    jnz .held                           ; engine_world holds it: always
    call UNMOUNT
.held:
    pop esi
    ret
.failed:
    mov eax, [ecx+SEEN]                 ; 0: resources.db, k: the k-th patch
    test eax, eax
    jz game                             ; the game's own "Cannot mount resources"
    movzx eax, byte [ecx+NUMBERS-1+eax] ; its n
    push eax
    lea eax, [edx+failed]
    push eax
    call FATAL
game:
    push ebp                            ; the six bytes the hook replaced
    mov ebp, esp
    and esp, -8
    jmp ON_MOUNTED_BODY

; In place of query_db's call of query_mount_archive: the same arguments,
; esi = the allocator, edi = the helper; the caller takes the arguments off.
mount_all:
    pushad
    sub esp, FRAME
    call .pc
.pc:
    pop eax
    sub eax, .pc
    mov [esp+DELTA], eax
    lea ecx, [esp+PATH_TEXT]
    mov [esp+PATH], ecx
    mov [esp+PATH+4], ecx
    mov byte [ecx], 0
    lea ecx, [esp+PATH_SEPARATOR]
    mov [esp+PATH+8], ecx
    mov byte [ecx], '\'
    xor eax, eax
    mov [esp+COUNT], eax
    mov [esp+INDEX], eax
    ; Which patches: every n from 1 to MAX_PATCHES whose file is there; one
    ; that is not there and is required stops the game.
.probe:
    mov eax, [esp+INDEX]
    cmp eax, MAX_PATCHES
    jae .probed
    inc eax
    mov [esp+INDEX], eax
    call format_patch
    mov edx, [esp+DELTA]
    push 0                              ; hTemplateFile
    push 0x80                           ; FILE_ATTRIBUTE_NORMAL
    push 3                              ; OPEN_EXISTING
    push 0                              ; lpSecurityAttributes
    push 7                              ; FILE_SHARE_READ | WRITE | DELETE
    push 0                              ; no access: whether it is there
    lea eax, [esp+24+PATH_TEXT]
    push eax
    call [edx+CreateFileA]
    cmp eax, -1                         ; INVALID_HANDLE_VALUE
    jne .there
    mov edx, [esp+DELTA]
    call [edx+GetLastError]
    cmp eax, ERROR_FILE_NOT_FOUND
    je .missing                         ; not there
    cmp eax, ERROR_PATH_NOT_FOUND
    jne .fatal                          ; there, and it cannot be opened
.missing:
    mov edx, [esp+DELTA]
    mov ecx, [esp+INDEX]
    bt [edx+needed], ecx
    jnc .probe                          ; not there, and not required
.fatal:                                 ; eax = the error
    push eax
    push dword [esp+4+INDEX]
    mov edx, [esp+8+DELTA]
    lea eax, [edx+unreadable]
    push eax
    call FATAL
.there:
    mov edx, [esp+DELTA]
    push eax
    call [edx+CloseHandle]
    mov edx, [esp+DELTA]
    mov ecx, [esp+COUNT]
    mov eax, [esp+INDEX]
    mov [edx+STATE+NUMBERS+ecx], al
    inc dword [esp+COUNT]
    jmp .probe
.probed:
    ; Every callback is counted before the first mount is queued.
    mov edx, [esp+DELTA]
    mov eax, [esp+COUNT]
    inc eax                             ; and resources.db
    mov dword [edx+STATE+SEEN], 0
    mov [edx+STATE+REMAINING], eax
    ; resources.db, as the game asked: the five arguments once more.
    mov esi, [esp+SAVED_ESI]
    push dword [esp+ARGS+0x10]
    push dword [esp+ARGS+0x10]
    push dword [esp+ARGS+0x10]
    push dword [esp+ARGS+0x10]
    push dword [esp+ARGS+0x10]
    call QUERY_MOUNT_ARCHIVE
    add esp, 0x14
    mov dword [esp+INDEX], 0
.mount:
    mov ecx, [esp+INDEX]
    cmp ecx, [esp+COUNT]
    jae .done
    inc dword [esp+INDEX]
    mov edx, [esp+DELTA]
    movzx eax, byte [edx+STATE+NUMBERS+ecx]
    call format_patch
    mov esi, [esp+SAVED_ESI]
    push dword [esp+ARGS+0x10]          ; the callback
    push dword [esp+ARGS+0x10]          ; the descriptor, "resources"
    lea eax, [esp+8+PATH]
    push eax                            ; the archive
    push eax                            ; its file table: the same file
    push dword [esp+ARGS+0x10]          ; the virtual path, "resources"
    call QUERY_MOUNT_ARCHIVE
    add esp, 0x14
    jmp .mount
.done:
    add esp, FRAME
    popad
    ret

; eax = n: the frame's path becomes "../../<descriptor>.db.patch_<n>", with
; '\' for '/' as query_db's. Every register but ebx, ebp and edi may change.
format_patch:
    push esi
    lea esi, [esp+8+PATH]               ; the frame, above the return address
    push eax
    push dword [esp+12+ARGS+0xC]        ; the descriptor
    mov edx, [esp+16+DELTA]
    lea eax, [edx+patch_format]
    push eax
    call FORMAT_PATH
    add esp, 12
    pop esi
    ret

patch_format:   db "../../%s.db.patch_%u", 0
failed:         db "Cannot mount resources.db.patch_%u. Please run the Skhid launcher again.", 0
unreadable:     db "Cannot open resources.db.patch_%u (error %u). Please run the Skhid launcher again.", 0
needed:         times (MAX_PATCHES + 1) / 8 db 0 ; the patches this exe requires: bit n
                                        ; of byte n/8 (bit 0 never set); patch_026g.py writes it
state_at:       dd STATE                ; for patch_026g.py: where the code keeps its state
