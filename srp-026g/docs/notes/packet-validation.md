# Packet safety replacement stack

A new independent stack on revision-45 main `6a547df87`, extracted from #478
and updated for the current protocol. Old #1164–#1172 remain open. Merge one
stack, in dependency order, and retarget each successor after merging its base.

The first chunk is a no_std allocation-free library: message/transport checks
and a fixed-size receipt codec, with no runtime callers. It preserves current
input bit 29 and the SRPX Blackout options. Later chunks integrate server input
isolation, world checks, native bounds and negotiated recipient recovery.

No release revision or simulation compatibility record is changed. Tests and
Clippy run under the repository Nix environment. Native runtime changes need
separate recovery/abuse checks and a release-candidate match before shipping.

The revision-45 baseline's static call-shape audit mistook a generated lohook's
continuation dispatch for a function return. The inert foundation fixes that
test walker, with negative cases for a wrong native exit and a malformed dispatch.
It changes no shipped native code.
