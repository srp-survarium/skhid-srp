; Patch 96 (build 2777): a sound voice that cannot get an XAudio2 source
; voice is not made, and the sound thread never spins looking for one
; (rust/patches/src/sound_voices.toml's three `code` hooks; the voice hand-on
; is the Rust patch sound_voices, docs/notes/sound_voices.md).
;
; source_voice_pool::acquire (0x5E5C0: eax the pool, its mono or stereo
; voices by the channel byte; [esp+8] the new owner) walks the voices
; round-robin from the cursor until one has no owner (+0Ch): with every one
; owned it never returns (0x5E5F0..0x5E603 count, but nothing compares the
; count), and no sound plays again. Retail 0.46c2 (0x8D1370) walks one pass
; and returns 0, and its create_sound_voice frees the half-built voice. The
; same here:
;
; - sound_voices_pick (0x5E5E7): the walk, at most once around, 0 back when
;   every voice is owned (or there is none);
; - sound_voices_picked (0x631D9, sound_voice's constructor after acquire):
;   a 0 is stored as the voice's source (+14h) and the constructor returns
;   (the scene's submix is not written through it at 0x631EC);
; - sound_voices_built (0x59F8E, create_sound_voice after the constructor):
;   a voice with no source is let go as ~sound_voice lets go of its sound
;   (0x5A47B: the sound's +28h call, its reference) and goes back to the
;   world's pool (+0A0h), as detach_voice frees one (0x5CBE5); the answer
;   is 0, as for a full pool.
;
; With 64 sound voices and 64 source voices of each kind the walk always
; finds one (a sound voice owns at most one and its release clears the
; owner, 0x5A474); this is the bound, not a change of what is heard.
bits 32
section .text

%define PICK_FOUND 0x5E605      ; acquire: ecx the free voice, its owner written, returned
%define CTOR_BACK 0x631DF       ; the constructor after `mov [esi+14h],eax`
%define CTOR_END 0x631EF        ; the constructor's epilogue after its pop edi (mov eax,esi / pop ebx / pop ecx / ret)
%define CREATE_ON 0x59F95       ; create_sound_voice's call of add_active_voice
%define CREATE_END 0x59F9A      ; its epilogue: mov eax,edi / ... / ret 0Ch
%define VOICE_RELEASE 0x5A47B   ; ~sound_voice after its source's owner clear: push ecx expected on entry

global sound_voices_pick
sound_voices_pick:              ; edi the count, esi the cursor, ebx the voices
    xor ebp, ebp
.next:
    cmp ebp, edi
    jae .none
    mov eax, [esi]
    mov ecx, [ebx + eax*4]
    inc eax
    xor edx, edx
    div edi
    mov [esi], edx
    cmp dword [ecx + 0Ch], 0
    je .found
    inc ebp
    jmp .next
.found:
    jmp PICK_FOUND
.none:
    pop edi
    pop esi
    pop ebp
    pop ebx
    xor eax, eax
    ret 8

global sound_voices_picked
sound_voices_picked:            ; eax the source voice or 0, esi the sound voice
    mov [esi + 14h], eax
    test eax, eax
    jz .none
    mov edx, [esi + 18h]
    jmp CTOR_BACK
.none:
    pop edi                     ; the constructor's own (0x631EB), the submix store after it skipped
    jmp CTOR_END

global sound_voices_built
sound_voices_built:             ; edi the sound voice (0 when the pool's allocate failed), esi the scene
    test edi, edi
    jz .stock
    cmp dword [edi + 14h], 0
    jne .stock
    push esi
    mov esi, edi
    call .release
    mov eax, [edi + 0Ch]        ; its world user
    mov eax, [eax + 118h]       ; the world
    mov eax, [eax + 0A0h]       ; its sound voices' pool
    mov edx, [eax + 20h]
    mov [edi], edx
    mov [eax + 20h], edi
    dec dword [eax + 24h]
    pop esi
    xor edi, edi
    jmp CREATE_END
.stock:
    push edi
    add esi, 2B0h
    jmp CREATE_ON
.release:
    push ecx
    jmp VOICE_RELEASE
