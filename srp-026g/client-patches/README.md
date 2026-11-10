# Client patches for 0.26g (build 2777)

What a copy of the **original** Survarium 0.26g needs to play on this server:
a patched `survarium.exe` and the login certificate. The game is not here;
the tools below work on your own copy. The patched exe itself is never
published.

**The revisions below are the releases'.** Each release sets its own
revision, the live exe's + 1, and builds with `--revision=<n>`; once it went
out, `release/release.sh` itself commits `REVISION = <n>` in
`patch_026g.py` and the `Revision <n>` row here (the UTC time, the exe's
sha256 without and with the server, the ZIP's, what it shipped) and tags the
commit it built `026g-rev<n>`. A feature PR never changes `REVISION` or
this table, even when it changes the exe: it describes its patch in the
sections below, and a server rule for it asks for `REVISION + 1`, the next
release's. The "Result" row is the build of the revision named in it.

| | |
|---|---|
| Original exe | `game/binaries/x86/survarium.exe`, 12 894 840 bytes, sha256 `ac33d9937015c2299bf62cc6b127cb308b48a3475b30fff2ddd65479ae3bf153` (the tools check it and refuse anything else) |
| Result (full patch, no embedded server) | 12 907 128 bytes, sha256 `9ef299c042d77e127d8e574176869c1e44ee4ae5de5e538576bbbe92fd277aa1` (patch revision 16); embedding a server changes the hash. With `--server=skhid.link:1234`: sha256 `b937e86fd561c2cd7267617717fed12de29f6b6a671efd24a875eb18e9173ad0`. Revision 15 was `b0bc3e02…7e5a` and, with that server, `e4368b3d…0ccd`. Revision 14 was `df9e6646…60ae` and, with that server, `0ad0132a…cf49`. Revision 13 was `7929b863…d842` and, with that server, `94c3b0fc…16ae9`. Revision 12 was `94318482…6aad` and, with that server, `9d43b199…9cc0`. While it was written (never released) revision 12 was numbered 14: with patch 27 as first written it was `770f7c04…5b09` and, with that server, `9bbca801…9e1d`; without patch 27 `9a09425f…ce16` and `14cd031e…79f9`. The never-released revision 13 was `c86d13fe…f4ab` and, with that server, `54a0658d…7971`; the never-released first revision 12 was `19586e8b…222f` and, with that server, `5deb3f4c…1441`. Revision 11, as the tool of `main` d0ea090 builds it (the replay fixes included), was `3f7e0488…2ffa` and, with that server, `5a5e18e2…0cc1` (before the replay fixes `bf289387…42c6` and `6b752ed0…c46a`); revision 10 was `c43bfd8a…ef85e` and, with that server, `c2188027…746b`; revision 9 was `03202d5e…20b7` and, with that server, `f30c3c66…11a1`; revision 8 was `9673dc5a…4756` and, with that server, `5701a96d…6aea`. With `--no-late-join` the tool gives the revision 4 exe byte for byte: 12 895 352 bytes, `3bd35a2078ab9288c1e30773cb4ade74f719fee15b2b5bc6ee39ca2f237d39fd`, and with that server `ae3ddf43f38eecb9e5080b10aa6b8397ce5dbed0f133b381461c82cb17329f25`, the exe of the players' download since 2026-10-13 15:36 UTC |
| Revision 5 to 10 | Revision 5 is revision 4 with patch 11, the late join (a player enters a match that is being played), and patch 12, a game that goes on without the focus. Revision 6 adds patch 13, three faults the first late joins of real players showed (the scoreboard without the newcomer, and two crashes); 5 and 6 play together, and a server asks for 6 because a game of 5 can be killed by a newcomer. Revision 7 adds patch 14: a newcomer with a grenade or a trap in a kit was never let in; it does not play with 5 and 6 (the order of a world's game objects is another), so a server asks for 7. Revision 8 adds patch 15: the game's clock is brought to the match clock six times faster after a start, a late join and a resync; it plays with 7 (nothing a match computes changes), and a server need not ask for it. Revision 9 adds patch 16, a newcomer's score on the scoreboard of the others was counted again at every rollback and showed an earlier match's score until its first, and patch 17, the team points drawn while a match counts down (the original kept the last match's until play began); it plays with 7 and 8 (neither is in the world), and a server need not ask for it. Revision 10 adds patch 18: at every spawn the new kit's weapons get a random state (their shots' spread) every game works out alike, where a game that had loaded a world during a match kept the other kits' weapons at 1 and went out of step when a player respawned into one, and patch 19: a game's first world of a match puts its kit choices (the world's) and inputs into the lists in time order, where a newcomer had respawned a player in the kit in use instead of the one chosen, and patch 20: a spawn gives the new kit's weapons the time of their last tick, and at a change of kit the fire mode, ammunition, chamber, aim and recoil, a new weapon has, where those were what the kit's last life left in the games that played it; it does not play with 9 or before (they part at a match's first spawn), so **a server asks for 10 (`SRP_MIN_PATCH_REVISION=10`), put up with the ZIP**. **5 was the players' download from 2026-10-13 23:27 UTC, 6 from 2026-10-14 00:44 UTC, 7 from 2026-10-14 02:26 UTC, 8 from 2026-10-14 03:48 UTC; 9 and 10 are not in a download yet.** Revision 5 is the first revision that does not play with the ones before it: such a game keeps room for twenty players in every match and sends other packets, so the server puts games from revision 5 on in a queue of their own (`docs/notes/matchmaking.md` section 9). A server that offers it asks for it (`SRP_MIN_PATCH_REVISION=5`), or its players are two queues that never meet. Against the revision 4 exe made the same way it differs in the revision byte, the 15 sites of patch 11, the two bytes of patch 12, four bytes of the ready hook and the headers of the two sections, and in nothing else of the original's bytes (the self-test's check 15). What was run and what was not: patch 11 below |
| Revision 11 | Revision 10 with patch 21: Team Deathmatch's death handler counts a death after the rule's own end (match.md section 28; below), and patch 22, a field-of-view setting (the video options' slider bound to `uro_fov`, which the camera follows, a scope's view staying the original's; a camera is not in what a match computes), and patch 23, a battery under the floor one stands on can be picked up (a use is in what a match computes). With patch 21 alone, made as the download is (`--server=skhid.link:1234`) it was `b728a80c62a8da6b0dca46de55236d1e7c4b010190f9c0fddf884e0c998d6ece`, without a server `985ca3410f024a9e391a8f6fd5750295d6c0add8185fc244245c15371400b75a`; both differ from revision 10 made the same way in six bytes (file 0x3054F1), the revision byte and the PE checksum, and in nothing else (compared byte by byte). It does not play with 10 or before (they part at the first death after a team is out), so **a server asks for 11 (`SRP_MIN_PATCH_REVISION=11`), put up with the ZIP**. **Not in a download yet.** The handler was run outside the game (`run_tdm_rule.ps1`); a game was not |
| Revision 12, first writing (never released) | Revision 11 with patch 24 as it was then: the game started only with a `resources.db` signed by Skhid's key; the retail one stopped it at start ("Resources archive is corrupted"). It differed from revision 11 in one call's target, the revision byte, the PE checksum and 413 bytes of `.srp`. 11 and 12 play together. **Never released; replaced by the revision 12 below** |
| Revision 13 (never released) | That first revision 12 with patch 25: the sign-in reports in byte 8 of the version field whether `%TEMP%\shell_32.dll` exists, a file one known cheat drops and never removes; the login server logs it. It differed from that revision 12 in two instructions at 0x380889 (a call and a nop), the revision byte, the PE checksum and 192 bytes of `.srp`. **Never released; patch 25 is in the revision 12 below** |
| Revision 12 | Revision 11 with patch 24 (the game checks no `resources.db` signature, so the retail archive and one with new files both start it), patch 25 (the sign-in reports `%TEMP%\shell_32.dll` in byte 8 of the version field, as the never-released revision 13 did) and patch 27 (the game mounts every `resources.db.patch_<n>` there, n = 1 to 255 in order, over `resources.db`; with no patch file it mounts `resources.db` alone, as before). Owner, 2026-10-16: this build is revision 12; while it was written it was numbered 13 and then 14 (the rows above), none of them released. Against the build numbered 14 (`770f7c04…5b09`) it differs in the revision byte (file 0x36FC66), the PE checksum, the call at 0xE7E3E (file 0xD723F: mount_all moved to +88h) and patch 27's code at 0x400 of `.srp` (file 0xC4AE00, 549 bytes before, 707 now: resources.db's reference of the state is let go once engine_world holds it, and a patch file that is there but cannot be opened stops the game), and in nothing else (compared byte by byte). Against the never-released revision 13 that build differed in the five bytes of the call at 0x135D3 (file 0x29D3), the revision byte, the PE checksum and the 413 bytes of the first revision 12's signature code at 0xC00 of `.srp`, now int3 padding, plus patch 27: the five bytes at 0xE7E3E, the six at 0x147E0 (file 0x3BE0) and its code at 0x400 of `.srp`. 11 and 12 play together: nothing a match computes changed. Run in a game: the build numbered 14 (`770f7c04…5b09`) mounted patches 14, 15 and 16 made by our writer in Wine on 2026-10-16, and they worked; its quit was not watched, and this build, with patch 27's fixes, has not been run. **Not in a download yet** |
| Revision 13 | Revision 12 with patch 28: the transparent effects (forward_default) scroll their textures as in 0.29 when a material asks for it (`use_uv_scrolling` and the four `*_map_scroll_*_direction` values), and the renderer binds `scene_slomo`, which 0.29's shaders multiply the time by, to 1.0. With the retail `resources.db` no shader the game loads changes (its `shaders/masks` lists no scrolling for `forward_base`); the scrolling shaders come with a `resources.db.patch_<n>` (London). Against revision 12 made by the tool of revision 12 (`bbba328`) from the same original, it differs in the three sites at 0x1D90DC (file 0x1C84DC, 7 bytes), 0x1D94CB (file 0x1C88CB, 7 bytes) and 0x14990F (file 0x138D0F, 10 bytes), 540 bytes at 0xB00 of `.srp` (file 0xC4B500, of which 503 differ from the int3 padding there), the revision byte (file 0x36FC66) and the PE checksum, and nothing else (530 bytes in all; compared byte by byte by `check_forward_scroll_026g.py --rev12=`, with and without that server). Rendering only, nothing a match computes: 12 and 13 play together; a server that offers a patch with the scrolling shaders asks for 13 (it must anyway: `SRP_MIN_PATCH_REVISION` is at least the highest patch). Written as revision 15 on the never-released build numbered 14; the number 13 of the never-released build above was never in a download. **Not in a download yet; not run in a game** |
| Revision 14 | Revision 13 with patch 29, "Watch replay" (below): the lobby's link callback starts `updater_skhid.exe` beside the game for a link `replay <id> <token> <sha256> <size>` and opens nothing else, where the original opened any text the server sent with ShellExecute. Nothing a match computes: 11 to 14 play together, so the minimum stays what it is; a server sends a replay link only to a game of 14 or more. Without a server sha256 `df9e66463bd8cb4bc4fa2536d87bc39fd7bfa8b17a0ebd184cb18392264a60ae`, with `--server=skhid.link:1234` `0ad0132a767dc87c571065b1c9cbe7071f152ae5fc26cc1b5f0fbdc629ebcf49`; against revision 13 made the same way by the tool of `main` decacfa they differ in the six bytes of the hook (file 0x27C1B6), the revision byte (file 0x36FC66), the 472 bytes of code at `.srp`+220h (file 0xC4AC20) and the PE checksum, and in nothing else (481 bytes; compared byte by byte). Written as revision 13 (and before that 12) while other patches took those numbers; never released under them. Released 16 Oct 2026, 15:10 UTC, with London as `resources.db.patch_14` |
| Revision 15 | Revision 14 with nothing else changed: the revision byte (file 0x36FC66) and the PE checksum, and nothing else (compared byte by byte with revision 14 made by the tool of `main` 374c349). The release that brings updater 0.3.0 (patch mirrors, the quick check) to the players needs a revision above the live one. 11 to 15 play together. Without a server sha256 `b0bc3e028edcc831d918fae7dab95de90576cea76a75613cf86acc26156f7e5a`, with `--server=skhid.link:1234` `e4368b3d6eb613535c7d22faea3e9339c2143e44668417069fb5afbdef740ccd`. Released 16 Oct 2026, 16:28 UTC |
| Revision 16 | Revision 15 with nothing else changed: the revision byte (file 0x36FC66) and the PE checksum, and nothing else (compared byte by byte with revision 15). For the server release of the weighted map draw (#138) and the release flow (#139); 11 to 16 play together. Without a server sha256 `9ef299c042d77e127d8e574176869c1e44ee4ae5de5e538576bbbe92fd277aa1`, with `--server=skhid.link:1234` `b937e86fd561c2cd7267617717fed12de29f6b6a671efd24a875eb18e9173ad0`. No content patch of its own |
| Revision 17 | Released 2026-10-17 12:30 UTC (tag `026g-rev17`, `main` 8511083). Shipped: #143 AGENTS.md: releases set the revision and tag; where the game's symbols are; #141 release.sh: the go first, the maintenance at once, the build during the countdown; --now; #142 0.26g: every map from the game data, nothing by level; #144 Launcher without SHOP, site without the silver and gold exchange; #145 release.sh: each release sets its revision (live + 1) and records and tags itself; #148 The third kit for every account, premium or not; #149 One cosmetic worn by many items (exe patch 30: taking it off names the item); #151 Replays of older games: refused, said why; a release raises the oldest replayable revision; #150 Patch 31: a kit chosen before the match starts is played at once; #152 Patch 31: the switch looks only at the match's places and always gives the new kit its protection. Without a server sha256 `8c37032d49548ed1bf697552dbf67ac509d63e7901f3bf325bf330d613b6b73b`, with `--server=skhid.link:1234` `441c84cdae515490212222e4a1d1a27e3dc3061a3432c691ed9f7f957b354af7`. The ZIP: 6861547 bytes, sha256 `195586fa8ebe24e725824594e715975445c1f95322fe330e4824c20c1f83a989`; the server data: sha256 `83d4593ad75dd6e467bca72b74baf4560896fbf8f45f2ff400b312ace4578e92`. Its content: `resources.db.patch_17` (76740663 bytes, sha256 `807b0e784c679a41009940d56ee76919d2bb5f14559154b927faf43405624081`) |
| Revision 18 | Released 2026-10-17 15:38 UTC (tag `026g-rev18`, `main` c5e6d39). Shipped: #157 Revision 17's SVD: 10 damage points less, checked on the candidate data; #156 026g client patch 33: the archive reads' order is a strict weak ordering; #160 026g client patch 35: a baked camouflage texture missing from the baker's list is baked again instead of stopping the game; #161 patches.tsv: patch_17's Google Drive mirror; #163 Shop: the factions' camouflages, the Vityaz and the Lancaster are sold; #162 Client patch 36: the memory arena "ai" is 4 MiB instead of 256 KiB. Without a server sha256 `b0246d0b8580572cc8171c1004352d0ed29d9e0dfc7da06a08f61bf4bf8fe272`, with `--server=skhid.link:1234` `7e89aec6a9d2ec816aad8e66626a0a0da7bcc825be22404d2441de896317353b`. The ZIP: 6861516 bytes, sha256 `04b5adbe9bdcb35e3732d2ca937520cd64aaaa353adc14778bd69b89eedd8f9f`; the server data: sha256 `c72d63c3b5dfdbc07a3a71f53344cf73d736582cbe94f912a415e070f79ef983`. No content patch of its own |
| Revision 19 | Released 2026-10-17 18:12 UTC (tag `026g-rev19`, `main` 7686a0b). Shipped: #168 Release of weapon scopes and the 0.29a3 weapons: #153 + #155 + #159 on main. Without a server sha256 `6f3b1c62054f3ba08766317207dd1237ad24a9a4bba481721fd9b701ecbb3930`, with `--server=skhid.link:1234` `ce4062cec6254bdcea1d2d469679c4971ab25872f1a502b309ccfa2d8ac90a4d`. The ZIP: 6861910 bytes, sha256 `47da8e2ee40b7345ef5061db8bd0e393d0d04b0848768cdb280333a72e3be45b`; the server data: sha256 `ae651889c2d9a2c9bf24d120ae530082e430ed4a6bb144803de811cd9c817642`. No content patch of its own |
| Revision 20 | Released 2026-10-18 02:10 UTC (tag `026g-rev20`, `main` a5ffe98). Shipped: #171 Squads of two: SRP_SQUAD_SIZE, an invite into a full squad is refused; #172 Revert "Squads of two" (#171); #175 0.26g matches: 8 a side by default, not 10. Without a server sha256 `6932dc7716c47b1cf4b6848768001de3ce7d10c113be14411b0ed878aeea504f`, with `--server=skhid.link:1234` `c064bbd72d137492992f3afe4a6872f9895d07d7056d9a15ddd3ad925b7743cc`. The ZIP: 6861910 bytes, sha256 `bca1f8b57fcf367e4bec9877b10e8b5fd50a1397193073405144c455ba74d7b6`; the server data: sha256 `40af0a1106207a47acf04577fe40c399af04dc911bccfbe7abb63e3e3aa7495c`. Its content: `resources.db.patch_20` (464470411 bytes, sha256 `7830ce1f175ea69c0e5fe34dbd3a47dfd6d606f102ff451422cf71471c723cc1`) |
| Revision 21 | Released 2026-10-18 14:43 UTC (tag `026g-rev21`, `main` 37bb872). Shipped: #164 Crash reporter zips its dump (SRP-crash-…-N.zip); #177 BugTrap: automatically localize crash reporting instructions; #182 Weapon balance 1: calibre-correct piercing, over-performer cuts, beginner rifles, SVD 5 rounds, magazine line on the card, stamina-bonus and sight-glow fixes. Without a server sha256 `c2a77b24eab7663902c2ee4ca82398b80d7ac894b777d37a60dae39a50bdb9b5`, with `--server=skhid.link:1234` `48dc9a2b73570a442200761f0cc4e1bda8a6e11fef253028c11d8521260bbcb8`. The ZIP: 6881533 bytes, sha256 `073e3b7c24ffc237613baba058fef79f6f461c695d6f1da4b99f104388cfce39`; the server data: sha256 `bb43a6f64da4ecfea32af05ca2736a240a1e7d887f0ff371aa29943f73938ec7`. No content patch of its own |
| Revision 22 | Released 2026-10-18 21:55 UTC (tag `026g-rev22`, `main` b4e7cea). Shipped: #184 Changelog: game update 21; #185 Shop: stop selling the Russian and Belarusian flags and coats of arms, the USSR flag and the red star; #187 Admin page: reset a forgotten password while srp runs; #186 Site: damage calculator tab (/damage), English and Ukrainian; #189 Password reset: close the sign-in race, write only the account, report errors. Without a server sha256 `6d082912ea51f6098b51c0ceba06341c2f4b6efd463e1e9ad3f275b0819987ef`, with `--server=skhid.link:1234` `f9e9dbeb45eed7a690d0e8f228c864beef4098546dbd60196546c609ed2462c6`. The ZIP: 6881533 bytes, sha256 `81d40f91c49f93487ba7df702e2a53f7af36df422a5633727005b6f822c9e399`; the server data: sha256 `c5615e24624e4cb146fceaefc9925fa20987fee0c5cf6333764cc8a094233dda`. Its content: `resources.db.patch_22` (845094613 bytes, sha256 `8d89d7a472d3659f370822212874aa237edb2257c8a3640d70c21c6fda0c65a5`) |
| Revision 23 | Released 2026-10-19 10:52 UTC (tag `026g-rev23`, `main` 2145a91). Shipped: #191 Site in the calculator's look; calculator: healing table, play-area map, aligned bonuses; #192 Damage calculator: armour picker by icon and level, item protection by its bonuses; #193 Website: Maps tab (spawns, batteries, chargers, containers, artefacts); #194 Reroll offers three on items of few types; backpacks may roll health regeneration; #188 Patch 31: before the start a drag turns the camera and the wheel moves it, as in the lobby; #180 Match native game memory defaults to launcher budgets; #178 Fix launcher crash when an install starts with stale status information; #196 Shop: sell the STK2 Novice's knit cap to everyone; #197 Site: Maps tab and calculator built with revision 23's patch. Without a server sha256 `bccb7d7b905112cbcf2cbe156a44774520b34a8de6370fe752b51c5281edb9a2`, with `--server=skhid.link:1234` `d4154dbefef865457f7c363b823995e009c419ec1b7c895ae8eb85811714186e`. The ZIP: 6881868 bytes, sha256 `a946f4442433d59c09ca41b311907ab13de1d2c64442c42a0b01325204608e38`; the server data: sha256 `fdee2965e1a16ae26a27ddb7ee414af65a44676ae43048861716b482b7fe4d89`. Its content: `resources.db.patch_23` (55877739 bytes, sha256 `f504ed4ceba7642df009dede3405236a4774f728c500f736f5a4921fe33141b0`) |
| Revision 24 | Released 2026-10-19 18:03 UTC (tag `026g-rev24`, `main` c52751b). Shipped: #204 Patch list: Google Drive mirrors of patches 19 to 23; #202 No tutorial: new accounts start with 10k silver, 500 gold, the Scavengers and ammo; #203 Rooms: every PvP match is a room's (the in-game Rooms window); the old queue goes; #206 Patch 39: the Tab list shows the places in the match, none whose player left; #207 Site: Maps tab and calculator built with revision 24's patch. Without a server sha256 `f1e91352927aeeda9aaae91d18eea88d5cfccdf979dfc4e56b102a37c378b9c0`, with `--server=skhid.link:1234` `809177184f37f8d69bb8edac8404c1f0ff1559dfe3f88a2e27959ce48988aaf5`. The ZIP: 6881999 bytes, sha256 `da6136600bf68e95d26f6c316873ccc746acf9c620036c00f32734ea78a8d536`; the server data: sha256 `6e0631e74542aa0c0eb92321c733062c02945590b57dc186a6560879e83e5f7a`. Its content: `resources.db.patch_24` (40387410 bytes, sha256 `c87623b9f80238a6b9c21507bd194bfbce6622cfd26b3a6be66839c9046a21b4`) |
| Revision 25 | Released 2026-10-20 08:36 UTC (tag `026g-rev25`, `main` 1948a6d). Shipped: #213 Lobby: every Get Gold opens the exchange, not the online store; #214 Exchange: a typed amount over the balance becomes the most it pays for; #215 Lobby: Buy on an owned item: modified ones too, no "already purchased" question; #216 Lobby: craft window: right-click Buy and Sell; Continue and Exit after an upgrade; #217 Lobby: Optics in a weapon's right-click menu (opens the scope editor); #201 Patch 38: spotting has no cooldown, and a battery's body spots it (simulation change); #200 Fall damage as in 0.27: fatal_damage 1.0 (simulation change); #205 Patch 40: a model's LOD and screen factor at a wider field of view are the original's; #212 Rooms: the map vote runs until the match starts; #218 Lobby movie: one driver builds every lobby change, in order; #219 Rooms: the room's window under a match's results; #210 Late joiners: fewer players first, then the side they played on, then the balance; #211 Rooms: a member sees it is in a room (yellow tab, red Leave room); #222 Ukrainian: the Duty faction is Долг; #209 Late joins: a failed joiner closes nothing, and no joiner waits for a game that is not playing; #221 Battery pick-up 20 % faster (take_time 0.6 -> 0.5); #199 Release: rebuild the Maps tab and the calculator from the release's patches; #224 re-kit 0.26g: weapon draw/holster, the modifier getter, the battery's states. Without a server sha256 `12006dd9346171e3fcadbd6ac1b63fed7e0c73ca7e4c031198bc371d24f9a8b4`, with `--server=skhid.link:1234` `0142022a113c03c61e1cb9cc1658d68e0e9d1ba6b04886f48785a43441396ca1`. The ZIP: 6882118 bytes, sha256 `6e27343932698b25a5b707be7121dffe93ae1988ab5eef6e8bedd17670fa9a0d`; the server data: sha256 `781087d5fdc07c0651fa1d8599a30a749b4819a5c4e4cb8d68f3deb502395288`. Its content: `resources.db.patch_25` (40649751 bytes, sha256 `cf5078143de863dceefc3976575ac94f08b9e5672d95bbff7522bd1ef3b076ac`) |
| Revision 26 | Released 2026-10-20 20:38 UTC (tag `026g-rev26`, `main` d7b9ca0). Shipped: #228 Clippy clean, and clippy in the dev process (AGENTS.md); #229 Rooms: the create window remembers the last room made by hand; #236 Patch 43: the player's own row in the Tab list is gold; #232 Rankings: right-click a player to see their equipment; #233 Rooms list: the selected room's players below State; right-click for their equipment; #234 Rooms: a match starts at the room's minimum, or is called off; #237 selftest: the relocation-under-site checks include one starting 3 bytes before; #240 Admin page: take silver, gold and items from an account, with a preview; #243 Drop cards: one per player of a match, own matches only; log sales and exchanges; #242 Weapon balance 2: AK-12 50 / 35 (premium: silver only), Socom16 damage 60; #239 Battery events pay per match at most as many times as the map has batteries (anti-farming); #245 Rooms with a PIN: their matches pay no rewards. Without a server sha256 `f02c1534a7631cc8eb628ed80a3e4d397e2826d6ca74e8521c253dfa7e431d2c`, with `--server=skhid.link:1234` `865d79efca2fac450f01938b5a22add57a7a551d570e9269db159542f9cc9c01`. The ZIP: 6882132 bytes, sha256 `ced6f17ed2e61622846da4f88b820a73185691950baeb33324698e1835ccc6ae`; the server data: sha256 `c7bb103a116ed7564b30c92dfe85429c5b3cb839ea7a42810487dc3469b8fa4b`. Its content: `resources.db.patch_26` (40426564 bytes, sha256 `42d9161d2c1ee9cf89a04015f1b6daa071922bf6f72f6dfdaf3c63d0e76fa17c`) |
| Revision 27 | Released 2026-10-21 13:10 UTC (tag `026g-rev27`, `main` 750277f). Shipped: #248 Flash movies as committed ActionScript source (baseline: all eight trees rebuild exactly); #253 Rooms: room chat and Join room from a friend's menu (source tree); #247 Fall damage: how speed counts; #254 Rooms: who came by Play leaves its room after the match it played (without squads); #255 Rooms: a restarted game learns its room before the room plays with it (source tree); #256 Replay tests: fixtures of today's revision, not 11-15; #257 Rooms: Switch room as the game's normal button, with why to switch; #259 Client programs: ordinary Windows binaries (std, static MSVC CRT, version info, icon); #258 Updater: PLAY only for a game the server lets in; release puts the ZIP in before srp starts; #260 Weapon balance 2, retuned: AK-12 piercing 40, Socom16 damage 70; #264 HUD at 4K: hit marks and crosshair centred at any UI scale; #265 Weapon balance 2: the A-545 takes 0.28a's nerf; #266 Grenade bounces louder: impact sound settings (content-edit grenade-bounce-1); #261 Results screen: drop card slot greys once used or refused; messages show over the screen; #262 Lobby source: names for every placeholder, named numbers in every movie; #263 Golden weapons: only the silver bonus, as the weapon's own modification; upgradable as the plain weapon; #267 release/build-content: the next content patch from a manifest of what should be live; #268 Reroll: keep the old modification (the price stays paid); #269 Ported masks: the Army balaclava (1823) and five 0.69d masks, one content patch; #270 Patch 46: on the death screen a drag turns the camera; the wheel does nothing; #271 Death poses 1: remove the face-down death pose that looks into the arms; #272 Crouched steps audible at 40 % of walking; #273 Weapon camo, the workbench, decals and instant preview (#244) on main for revision 27; #274 Grenade bounce retuned: only the distance preset changes (mid_range). Without a server sha256 `8c25b2d9deda746a53f218380b3acafa76241697b6238d231817bfe6300f1147`, with `--server=skhid.link:1234` `459031e87bdce63a1564eb84df2de81079a878c8444603a6ecf0aac59641b2f6`. The ZIP: 7089320 bytes, sha256 `0a691de4291013660132de5dd09b2eaa46451c5109b0ae683f538c6af21af4a9`; the server data: sha256 `334ac3b45ebaaa6538d541cb861c73b94515afce4d3f5b782e12f3735eff1d4b`. Its content: `resources.db.patch_27` (311105123 bytes, sha256 `7c540c3de5aee63b5c2e856eaea8276df4f6bbfe3bf8a42054f8aab27ed548ab`) |
| Revision 28 | Released 2026-10-22 14:12 UTC (tag `026g-rev28`, `main` 62a2b51). Shipped: #276 Rooms: the results stay open when Play's member leaves the room after a match; #277 One pocket an item type (server rule, kit migration, lobby drop targets, 4th-grenade fix); #278 Battery modes: respawn 10 s; #279 UI at 4K: Esc menu, Settings, cursor, replay controls and lobby pop-ups take the 1080p scale; #281 v0.11 maps: spawn protection 6 s; #280 Patch 49: the game stops at its start without a patch its release requires; #283 Lobby: item search (always open, typing starts it, levels and kinds) and lobby keys (R, Enter, Esc); #290 Golden weapons: the shop and bundles show the silver bonus again; #292 Rerolls: no wait between silver ones; no Upgrade at three modifications; #291 Lobby: best bonus in gold, and the bonus range in the reroll window; #284 Release overlay test: expect required_patches (after #280); #287 Content: 38 decals and the Steam camouflage from 0.69d; #293 Premium armour: its bonuses rerollable modifications of its own, upgradable as the plain piece; #294 Crafts: a golden item is a donor for its plain one too; #295 Lobby: compare with the item being upgraded; premium bonuses in gold; #296 Patch 52: a key in the lobby no longer kills the game when the focus is on a dead object; #298 Lobby: craft list and compare colours; sight card's kit weapons in gold; character menu in the shop; #299 Shop: Attachment on a kit item no longer crashes; its window lists the inventory; #300 Rooms: browse All rooms from a room (My room / All rooms tabs); #301 Lobby: no New Year tree, gifts or Christmas sound; two bulbs light the workbench; #302 Lobby: the camera turns again after the upgrade window closes. Without a server sha256 `aa7aa0ade158204397b50e3312df3700b837b8d93b85e5dc61cbbca91dadae88`, with `--server=skhid.link:1234` `9bbdfc89b09b1b98a2f0b69d337332bd5894e792c0f18f3792e69cd234bd5baa`. The ZIP: 7089347 bytes, sha256 `dee77d8b81a43acd7453e6ad8284843bb2d803af4a5a2d019c77ed10390ff496`; the server data: sha256 `ca6badd576aedd00d86a072aeb49a8b93bae62611cd9a5f0b94bff4dab542607`. Its content: `resources.db.patch_28` (96458122 bytes, sha256 `6174bd30b426b1d1c73db0b738832a895d4f28639b4cb629fa93533088ef5830`) |
| Revision 29 | Released 2026-10-22 14:57 UTC (tag `026g-rev29`, `main` 16a4969). Shipped: #304 Pockets: equipping a medkit, grenade or trap works again (rev 28 regression). Without a server sha256 `2ffb4b5306923ea06a9e128d7d6e8c59e516774445adea8f8e07ec291fde86f3`, with `--server=skhid.link:1234` `8d74468acf0ebe88a1b1fb8454681b4e473c40190d2f437e905850422f0c8d8e`. The ZIP: 7089349 bytes, sha256 `7bc077c1404f59de8e4f05ec4c875c8fd5b779b92751bd551d0f555a4eec7682`; the server data: sha256 `3079fef7c91ba463e57cdeef10ad80ebdc17aa60c4b59c966cd0c40e37c33ec8`. Its content: `resources.db.patch_29` (42888467 bytes, sha256 `a36e6f77f707a33519a6c1061f6f4a1a6e7a1d2e5fc9cb3598a479f4d753b7b6`) |
| Revision 30 | Released 2026-10-23 04:36 UTC (tag `026g-rev30`, `main` 261dd6e). Shipped: #297 Research (mode 5) as 0.27d shipped it: exe patch 53, 25 maps, hud, lobby, srp (for the release after revision 28); #307 Generic tooling: .srpx arena, call-shape models, content value-type check; #310 Research: review fixes (slot 7 argument order, scoreboard, draw at the goal) and slots driven through the game's caller; #309 Any modification of an item may be rerolled; #306 Ban texts: no prohibited-software header, a neutral fallback reason; #312 Lobby: the Attachments window opens again after it was closed once (rev 28 regression); #313 Patch 48: an upgrade window closed at once leaves the kit's weapon in the hands; #308 Artefacts buffed: Spring's aim time and no slow, Lifebone's full heal, one press for every artefact of a kind (patch 54); #317 Engine-function library: plain cdecl calls into LTO-ABI engine functions, checked against the re-kit rows; #320 Exe patches: link the arena (.srpx) from C/NASM sources, no pinned addresses; #314 Kit presets: five kit sets, the active one is the kits, a Character dropdown, and a preset offer on room join; #321 Lobby: Play and the rooms' entries greyed until the reasons are known; a refused Play closes the rooms window; #324 Shop shows only the bundles: reputations again after the price list; #322 Reliability never rolls: upgrades, rerolls and drops leave it out; #323 Updater: a file Windows Security blocks stops the update, never "up to date"; #325 Artefact hints say the new numbers (Spring, Lifebone, Onyx); #326 Reroll window: remove the one-modification lock code left after #309; #327 Upgrade: a failed craft no longer takes a modification away; #328 Reroll window: the game's own Cancel in place of "Keep old"; #329 Craft window: Buy from its slots; the TT-33 in the shop at 500; #315 Patch 45: the camera flies to the bench at once; #330 Research HUD: "Capturing unit..." on one line, not "Captu". Without a server sha256 `0aafdae2e9a091a3d1e33622295a5cc9705e9304774d92d90d2a517789993308`, with `--server=skhid.link:1234` `be00472e81cb8e70dcd956f64f4f0729933feea674c449543f64f6b6fdf6562d`. The ZIP: 7101063 bytes, sha256 `910982bc2febdd49a06a0e7c605b26a07bab908c97a58ffdd636455c2f58572f`; the server data: sha256 `44e48efbef974ac53655df9e6315fbdbd858ae6cda151cb719fd2e742e1f38ba`. Its content: `resources.db.patch_30` (72222773 bytes, sha256 `ddcd540ab7135b821740170c7424a00a526de6597d6e06a2d1c1862f75642cd1`) |
| Revision 31 | Released 2026-10-23 13:10 UTC (tag `026g-rev31`, `main` c106d42). Shipped: #334 Modification window: hovering the item shows its stats tip; #332 Shop: a kit item's menu acts on that item (Modification refused on rev 30); #336 T16: Regeneration modification works, it is just small (findings + re-kit rows); #333 Research HUD: units under the timer from the first frame, below the respawn line; #339 Crouched steps fire: the moving crouch clips get the step event channel (T27); #340 Larkspur (Lifebone): active cooldown 15 s -> 45 s; #338 Lobby: Autorepair box gone, Autobuy on by default, larger loadout switcher, inventory loadout badges; #337 Protective mask (1816) is just a mask: worn with a helmet (T28); #341 Master set: its camouflage net drawn as a normal alpha-tested material, not a glass-like forward one (T31); #225 Patch 42: dynamic anomalies as the release had them, on London and Cologne (exe, data, bots); #342 Match HUD: keys 1/2/3 pick a kit and Hide chat in match (patch 58, T29); #345 Take the dynamic anomalies (#225, patch 42) out of revision 31; #349 Lobby: Esc closes the topmost modal only, from one modal stack; #350 Esc follow-up to #349: box over results, invite Esc hides, craft search, loading, dropdown order; #352 Research: count captures in the player's own kit cell; #353 Research HUD: units back at rev 30's height. Without a server sha256 `cd3a02919d3da190b7f5aaad0fdfe237fa1f73e2759e8a54cd2ad49e1971220a`, with `--server=skhid.link:1234` `0c302c8c409fb7f061d63219d65e8ff730c4cb1d5f9aeea59d546d90ae514283`. The ZIP: 7101346 bytes, sha256 `e261c267733ee2794ddb47edc8c97b095cfa32addbb74314aed01b71845bdd36`; the server data: sha256 `f0c992ad35bb698d006891ac383af800e97204bffe99f250d56066557933e435`. Its content: `resources.db.patch_31` (54639459 bytes, sha256 `ba1428be44e4c161681780b9eff0e52755f370fd49fb8b0c929db6e0813fa6b2`) |
| Revision 32 | Released 2026-10-25 12:14 UTC (tag `026g-rev32`, `main` 93726bd). Shipped: #356 Upgrade window: loadout badges on the item and donor slots; #368 Patch 62: F11 hides the whole interface (match and lobby); no-UI battery mode spec; #397 Bandages: hands and legs full, pain, body and head 35 %; #400 New accounts start with a starter kit; #408 Onyx nerf: -60 % for 3 s, +1 s for each other Onyx; #414 Artefacts: the carrier sees his own glow in first person; #423 Onyx: -75 % when active; #422 Rooms: the create window opens with its saved settings, preloaded; #424 Lobby card: stats over their scale show their real value; #418 Kill feed: wall, ricochet and pain-shock icons after the weapon (patch 67). Without a server sha256 `a74d42e802ca8fd71217d1872ce9b9ab9666ecbb0f549f2c462b2f1ce4ba158b`, with `--server=skhid.link:1234` `2cdc466b6c4ab576115a9b12b5340996db4d82a51a2e3c81976c3fe0a669ba5a`. The ZIP: 7103036 bytes, sha256 `b2c9e19dde9b851e1ab6028cf64f6100dbb051f26b8cb26ff5b286481c50b8e8`; the server data: sha256 `db598bc52d34ea95552f8d26bd55c78892d976c55c6d4ea6115caca23ae48c5b`. Its content: `resources.db.patch_32` (53353663 bytes, sha256 `1c65af3eea6a4edd65d61f278405b061d3e98fd78e4bb990880461c3874779ba`) |
| Revision 33 | Released 2026-10-25 13:52 UTC (tag `026g-rev33`, `main` d4c60d1). Shipped: #436 Kill feed: a suicide row with the pain icon no longer empties the feed. Without a server sha256 `1f9caa52a72ab26ece2604f12f6a04982ab7c88ebc51670a6dc07d708dbeba2d`, with `--server=skhid.link:1234` `8de3a2cbfc966343a430344fa1b71c13f7300775f4ffe7cbacff969d36c1a501`. The ZIP: 7103036 bytes, sha256 `f3d43dbf94727d018997af5c778b922294e971416cf6eb358f40564108fdad01`; the server data: sha256 `38a5499906f7e74bcdd2689fab94e211b459d57c12c8efe6266e3cce6753bc68`. Its content: `resources.db.patch_33` (7634659 bytes, sha256 `e689f159a0a98528297f03a8f320b3d8af6803725811edd49e63cd498a16ef2e`) |
| Revision 34 | Released 2026-10-25 18:43 UTC (tag `026g-rev34`, `main` ce26883). Shipped: #443 SOCOM 16 nerf: damage 70 -> 60, armour piercing 45 -> 40, equipment level 15; #444 Patch 72: falls gain speed for the time they fell, not per step of a split tick; #367 Research: capture timers and the units' markers on the screen; #450 Reroll: every modification change costs the same; #440 Patch 70: baked camouflage textures kept across games, made once, held for the game; #453 Network help: clearer "no connection" errors in every language, VPN.txt, firewall script; #456 SVD at equipment level 15; #457 AGENTS.md: types and states (illegal states unrepresentable, parse don't validate); #458 Revert Research HUD markers (#367) for rev 34. Without a server sha256 `a9832c0c19a00eec6ddb1943451778088524eee250b9c5ce95a5f20c410fc0aa`, with `--server=skhid.link:1234` `e7d680c9fd72c439d66a833f5824fb87e417d89dc960aab335ab40a83a6d4061`. The ZIP: 7115226 bytes, sha256 `45ca6b342c7dd558723a571e05f03603a75fb95aa63d3683f442f20b100e8a37`; the server data: sha256 `228c7d34bcaf358636eab09c9ff6cb98b5abfeecfe720ab815f1bac5f9e92e95`. Its content: `resources.db.patch_34` (2072159 bytes, sha256 `4e791036d7aac333c4ecc9ea2104acc944d1e9d015fbc6b9e0216a8c7033190c`) |
| Revision 35 | Released 2026-10-26 15:08 UTC (tag `026g-rev35`, `main` 273fb02). Shipped: #459 Crash reporter: crash details even when the dump fails; #462 hud: keep a hidden minimap hidden when a match's map loads; #512 Kill feed: the ported weapons show their own icons (hud icons 53-68, combat_log_icon edit list); #518 Crash reporter: game logs in the game's code page are transcoded, not dropped; #475 Rust patches: the framework (workspace, hook ABI, bindings, types, panics and log, the equivalence gate, one example); #529 HUD: the view fades over the respawn countdown; #527 Rooms: a private room's owner arranges the teams and starts the match (1 v 0 too); #464 Ragdoll on death: patch 76 on the Rust framework, the solver and the Bodycam look; #538 Self-test: the hud's stage also holds the death fade (#529); #540 AGENTS.md: quick checks while iterating, the full run before pushing; #537 Blackout on main: a battery mode with no interface (patch 65 in Rust); #439 London: remove anomalies and use the editor's spawn placements. Without a server sha256 `9c72939fd8cbf423f572d7a443411505cc23739e6c958e67446634ad59db7b7c`, with `--server=skhid.link:1234` `ce94d1dc7685b5741b2e7c92afb7a69ac5db146f39074605c2b2d37c8c734195`. The ZIP: 7172000 bytes, sha256 `3ad570140b69e3efaa34197588c671ec6b08fd1701af1df4a9426221f8b936f4`; the server data: sha256 `123edb74791fd3a01efb178ed5857b29c2a9c3e3f4b8a6f73f4cdddcb8647965`. Its content: `resources.db.patch_35` (55727464 bytes, sha256 `886c41bb800c16007efb6cefda9cbf88a11bf1ca71b091c0a08737c84389a36f`) |
| Revision 36 | Released 2026-10-27 21:18 UTC (tag `026g-rev36`, `main` f4e1c28). Shipped: #556 Changelog: revision 35 for players; #558 release: ship and check the bots' data files (maps, routes table) with the release; #566 release: the bots' foliage file in the bot data list; #578 Bots at every equipment level, in the top players' kits; #569 Rooms: a private room has no max level; #599 Firewall script: in Russian, and it asks for administrator rights itself; #686 Fix Research markers and countdown layout using Rust; #689 Medicines: accurate card values, activation sweep and effect countdown in the quick slots, gray while pending. Without a server sha256 `a948d9635f2d8ba16682016c99634cb47badbd3b3cf7ec6c7a8d2f69f929d985`, with `--server=skhid.link:1234` `70c0abf574b2f10b3fd00d60a86f2591474225f9b234c97fab6a21b43ce12905`. The ZIP: 7176266 bytes, sha256 `1c35aa5395044c89d75bdd77cf05261c4266b7c22d65c613ffc47070efcf7ce9`; the server data: sha256 `66426bd0c8492c20417ef88661d522b9f4c937cbed252cba73f941de74946df1`. No content patch of its own |
| Revision 37 | Released 2026-10-28 18:37 UTC (tag `026g-rev37`, `main` b61e89c). Shipped: #691 Pay: target farming, cap PvE rewards, and retain solo bot replays; #692 Show enemy capture warnings in Research mode; #463 Patch 58: kit keys pick nothing if pressed before the kits came or in their first 300 ms; #694 Create public rooms on demand and fix queue transitions; #695 Remove editor vegetation data from the four supported ported maps; #696 Avoid downloading every patch again during releases. Without a server sha256 `b8d08857d6bd4aa8b2edad07b21ea8359eb3adf4b78da595a198b6d85381696f`, with `--server=skhid.link:1234` `5c5801af6080b197d9eb754cf1205bbe2745d46689d5ebbf6f7032142d15b7fd`. The ZIP: 7176336 bytes, sha256 `419ead320493e2be811098f56d53044541164681e1e72adb713f798dcd95690c`; the server data: sha256 `66e8799faf6f5e13bb35f5a839e466f3ba8ed97f33d97c7964800c51946c364c`. Its content: `resources.db.patch_37` (54905609 bytes, sha256 `9a0d0b7b9e8ffbdab755baee10cb6f5d481cf3d818dfb2f82a1cb4133cff1a95`) |
| Revision 38 | Released 2026-10-29 20:32 UTC (tag `026g-rev38`, `main` ff4fe0a). Shipped: #698 Changelog: revision 37 for players, including rewards and solo replays; #727 Add weapons wiki, recoil comparison and detailed game tooltips; #739 Shop: buy an item by dropping it on its slot; #721 Game languages: ten dictionaries as TSVs, Belarusian, Czech/Hungarian/Finnish/Serbian, Language in Settings; #741 Rust patches: typed raw handles, layout only in raw; #745 Weapon rolls: recoil and aim time take effect; chamber time on the card; #716 Flag of Serbia decal: red over blue over white; #746 Site: the wiki takes the calculator and the maps, and gets a modifiers page; #750 Equipment through 0.29: 84 pieces, balanced against native gear; #752 Weapons wiki: shots at their own moments, views hold at their last shot, no Next shot; #699 Detach selected helmets on fatal headshots; #754 Equipment: its texts in all ten languages (release 38 content build); #753 Weapons wiki: sideways kick at a degree a unit; smooth camera shake; #757 Wiki: Equipment, every armour piece with its protection, weight and bonuses; #755 Notes: what the 0.26g armour parameters do; #756 Headwear: the 0.29 helmets fly off on fatal headshots; the knit cap doesn't; #758 Wiki modifiers: Can roll leaves out what an item comes with; #760 Exoskeleton boots and gloves without their black areas (pack exo-gear@1); #763 Exo boots' lobby icons at the stock boots' angle (exo-gear@2); #762 Release: per-release item tables (equipment, weapons) in the record; #761 Wiki equipment: flying off only in the card's details; #759 Built-in bonuses at the second-strongest strength; exoskeleton suits buffed; #751 Carry and share artefacts with animated hand poses and death drops; #765 Match: an anomaly artefact pickup scores 15 (5 with full containers); #770 Equipment 0.29: no donor placeholder descriptions; #771 Equipment 0.29: German placeholder descriptions too; #773 Weapon card: chambering time shows only the new value; #774 Exoskeleton gloves keep their stock icons (exo-gear@3); #775 Equipment 0.29: no shader 0.26g lacks (Watchman gloves crash); #777 Artefact carry: H held, then a quick key, raises the artefact; #779 Artefact carry: H's up comes from the key handler (stuck carry key). Without a server sha256 `955e2208d3b05ddf117ffafc66cca041edb8093ad66c7ef1cfa764e05744032b`, with `--server=skhid.link:1234` `a9da4ad1428b8b6e55cf2ee12c879aecddf247b1cc589072ab9d8fc511608e78`. The ZIP: 7197600 bytes, sha256 `53d273e95594a50ae4a3c86afd2512930d9b0ce55afcebfb064c148c8e435708`; the server data: sha256 `e30f7b12cacbe4147745fb5af0cf95d43e816a74bb9d9f3b8022fa6bf7553c44`. Its content: `resources.db.patch_38` (390848411 bytes, sha256 `307cd527ac6cfaeb1085ba156f4d45f81308e8189def3c18ecb6d0e245850823`) |
| Revision 39 | Released 2026-10-30 17:44 UTC (tag `026g-rev39`, `main` e3c9a01). Shipped: #804 caddy26: plain TCP only (net.mptcp.enabled=0); #787 Launcher: the language is the game's, every language, kept in user.cfg; #783 Artefact carry: H's key states as the engine defines them; #790 Artefact carry: no gun falls from a carrier's death; #792 Late join: a loaded world puts its alive players into it (patch 82); #806 Equipment from 0.31a2 and 0.46c2: 35 of the owner's 38 picks (pack equipment-046@4); #825 Backpacks from 0.69: the Medic's bag and the Soldier's backpack (pack equipment-069@1), into main; #793 Rooms: the chosen room's card keeps one layout through the vote; #786 Release: build and stage before the go, then the maintenance and a quick swap; #789 Lobby: the kit's movement speed beside the weight; #828 Release: close four gaps found in releases 36-38; #831 Minimap: a late joiner's allies get the ally's icon; #832 Revert #792: the world load already inserts its players (patch 82 never acts); #829 Hud: the Double cylinder's quick-slot icon; #834 Wear the exoskeleton boots under full-length trousers; #781 Artefact carry: the held hand breathes and flexes its fingers; #835 Ragdoll: no headshot launch (exp_neg is 0 from 20); #764 Add breakable map lamps with the Rust patch framework; #838 Wiki icons: read the shop filters from FiltersWindow's list functions. Without a server sha256 `24e6ceb4c2e0897e18e89b7a7fe061e58be7c4dbee53a24967bc54c14f0a3e77`, with `--server=skhid.link:1234` `b0da71a57d786d2d2971bd3fe618ec25a877d68a049f6f08b6e9cca1345a70c9`. The ZIP: 7211382 bytes, sha256 `f1ff487017770366b8275e314dd78da33729ab41a3b370f205637c5443917f46`; the server data: sha256 `a19b64cef5a49374b0b0fce9189834c97c9547a63bb68ca0422800488c402f10`. Its content: `resources.db.patch_39` (355552277 bytes, sha256 `b1fde5c35b09764f8a1292ef3ece295251b975f0516acd73cd397649d6a93036`) |
| Revision 40 | Released 2026-10-31 19:04 UTC (tag `026g-rev40`, `main` 2a8e9d4). Shipped: #842 Shop filters: one table the lobby build and the wiki read; #843 Lamps: orient each shatter burst in a frame the game can build; #841 Rooms: drop the vote sentence from the room's bar; #845 Lobby: a right-click menu's Buy buys the item it was opened on; #852 Play that would queue offers the other bracket's room as Join does; #857 Tab map: names over the player icons while Alt is held (patch 82); #860 Tab list: Ctrl shows a cursor, and a row's menu sends a friend request (patch 86); #876 Rooms: members by Play stay after the match; the results stay up under room changes; #863 Settings: Hide hit markers, Hide player scores (exe patch 83); #862 Lobby: transient windows (History closes when anything else opens; PvP/bots list over Rooms); #879 Settings HUD group: "Hidden minimap" as "Hidden crosshair"; #878 Mute chat: two state-dependent entries in the lobby's player menu and the Tab list's row menu (patch 86); #880 World frame check: the match server and the game refuse a world no game could load (patch 87, crate world-frame); #895 Short scope slot label and stock scope names that fit the upgrade window; #884 Squads of two: Play and Join take the mate along, marked in the rooms; #881 Music: the menus' and maps' themes, 15 s past the start (patch 88, pack music@1); #851 STK2 outfits: the bandit's, the Novice's and the loner's pieces (packs stk2-bandit, stk2-novice, stk2-loner); #883 Patch 84: a bullet that hits a player stops the healing drug in use; #901 Blackout: show teammates' markers all match, not only on Tab; #882 Medicines: three coloured medkits, all five made from one table; #805 Rudnya (v0.69): 0.69d's Rudnya as map 204 and its battery and Research modes (216-218); #909 Launcher: read the installer's values without opening any key; #778 Lobby scene picked in a lobby window, the squad commander's in a squad, shown at once behind a fade (patch 81); patch 45 round 7; #776 Content: the 0.46c2 hangar and 0.69d tent-camp lobbies (lobby-hangar-046@7, lobby-camp-069@9); #802 STK2 bomb shelter lobby (lobby-stk2-bunker@4): relit warm, the whole shelter; the Bar dropped; #915 Lobby selection review: keep the lobby under the upgrade window, drop content-rev40; #899 Start Skhid without the updater; split the Defender exclusion from the network setup; #913 Lobby button: drawn at its box's size (it vanished in the game); #919 Rudnya (v0.69): ammo only at the bases; #916 Lobby packs: textures 0.26g already has drawn from its own files (hangar @9, camp @17); #920 Drops: no camouflage or decal the player already has. Without a server sha256 `c957be5a4b67e03e628cf0c2b8b7362943b8b238498947117a789b86a5d7c312`, with `--server=skhid.link:1234` `87e3c688db81c2da61ffd034f6319a59c50279df0a134fd5bc50206eadaf5687`. The ZIP: 7236315 bytes, sha256 `b7d2d029bd53e4166e983a9d3ac1f633dbd94f7eeb928b6c4bbb8498163a0583`; the server data: sha256 `141b4d3f1e61340ca863cf1082525c145630f244d216ff1092d8be78679eef20`. Its content: `resources.db.patch_40` (1151219934 bytes, sha256 `6c549e77a18d07d7226d460072ea940e3329f7fed3f07316e00abcd10a7011e8`) |
| Revision 41 | Released 2026-11-01 22:38 UTC (tag `026g-rev41`, `main` 958eaa8). Shipped: #921 Remove dead Rust code; #922 Remove stale docs, unused 0.100b tools and unused example files; #923 Remove the dead tutorial code from the lobby and hud movies; #924 Small audit fixes: Blackout in Matches, one on/off switch, one mask length, RUST_PATCHES; #926 Remove old notes and one-off audits; #946 Rudnya (v0.69): each base ammo crate refills its own team only; #991 Notes: the 0.26g renderer as a light bake needs it; #999 Login: ask for the nickname, not the e-mail; #983 Shop: a check mark on owned items, and a Hide owned box; #985 Rooms: rejoin the room's running match through the room after a relaunch; #982 Queue of a full room offers a loadout as Join does; #1000 Batteries: points for a battery's new progress toward its container (patch 92); #984 Lobby: buying an item owned asks first; only the inventory's right-click Buy skips it; #1002 Player vs Bots: drops roll at match level 7 at most, drop cards too; #893 Weapon kit, step A: the kit transport and a second decal on weapons (patch 68, Rust); #996 Patch 91: a weapon shown while aimed keeps its body; #1004 Upgrade window: a double click puts a camouflage, decal or scope in its slot; #1003 Shop: a consumable's buy starts at 30; #1009 Download mirrors: the game's archive and the patches from many addresses, resumed, logged; #990 build-content: files an earlier version of a pack left stay, listed; #1013 Setup scripts: English as well as Russian; the Defender setup puts back deleted files; #1017 release/test.sh: Ctrl-C tests signal a release.sh that can be interrupted; #1018 Crash dumps: referenced memory live, full memory for local tests; #939 Balance the sides by K/D at the match's equipment level; #1023 Battery points: progress paid at the delivery; found 5, delivery share 5; #1024 Patch 85: pick up a dead player's gun; #951 Patch 19's 0.29a3 weapons: melee k_damage 1.5, as every native weapon's; #1001 Content: 0.31a2's lower screen shake (shake-031-1); #1011 Content: HK416 and M4A1 kick nearer the AK-12 (recoil-hk416-m4a1-1); #1031 Revert #1011 (HK416/M4A1 recoil) for revision 41. Without a server sha256 `d17482fc54393068eb1aaf4a9476c0c4f7e38f636c4743731e76ddee3172352f`, with `--server=skhid.link:1234` `985acc388f212dcdc063a77ba9a80c1ff8929fba93acba9f84d1e61e208af2e7`. The ZIP: 7294307 bytes, sha256 `569832a70e9a3d694fca524fd2b7a8bb6b897f2f9b12e757c6198fc9fe6ebb82`; the server data: sha256 `618462254d8865cff465a99feca3a5501dad0974da7603ef72b00ac17427272e`. Its content: `resources.db.patch_41` (69553610 bytes, sha256 `8e12593edb9efb5037fc0a6769d319f50d0c674ee1a3486e1301977994005faf`) |
| Revision 42 | Released 2026-11-02 03:58 UTC (tag `026g-rev42`, `main` 92723ff). Shipped: #1026 Admin chat mute: a muted account's lines reach nobody, it is told why; #1037 Put #1011 (HK416/M4A1 recoil) back after revision 41; #1032 Weapon stats as a table (weapon-stats.tsv); cards written from the configs; #1041 Readers of content.tsv edit rows skip stats tables; #1044 Admin page: the level field's label says 0 to 100; #1028 Scaleform: the render thread's capture walk never reaches a queue being destroyed (patch 93); #1034 Weapon reload rolls: checked, they work as the card shows (notes + re-kit); #1046 Shotguns: Saiga-12 and Vepr-12 armour piercing 0.1 -> 0.3; #1043 Dead duplicate keys out of the item configs (dead-keys-1); #1048 A-545: damage 50 and 900 rounds a minute again; #1047 Stats tables: list rows, several files a row, entry selectors, key/value tables; #1033 Lobby: hold Alt to see a weapon's stats with kit and skills; #988 Room lobby: camp @18, no hard-edged ambient boxes at the ceiling; #1052 Site: serve every file of damage/ (Rudnya's render was not served); #1045 Patch 85: fix the rev41 late-join desync, simplify the pickup's ammo; #1049 Fit the rooms' Leave on Play in every language; #1050 re-kit: one name for the weapon states' get_weapon_lexeme; #1054 Home page: trimmed download steps, then a short FAQ; #1063 Check that a resync converges: world load+save round trip; #1040 Shop: smooth Hide owned tick, Hide owned beside Comparing items, no sort (always by level); #1061 Skhid-collect-logs.cmd: logs and the game folder's state in one ZIP to send; #1056 Bug-report Discord invite: discord.gg/QUmJg3gBhg; #1065 Ragdolls: rest on the bullet collision too (dropped guns no longer sink on Rudnya); #1055 Ammo and throwables as tables (ammo.tsv, throwables.tsv); #1066 Maps: the game's minimap by default, the render on the player's pick; #1067 Self-test ragdoll: the arena bounds the Rust statics; replayed calls allowed; #1062 Updater: a box per problem of the site's list, its title what failed; #1069 Home page: quote the launcher's new box titles; #1058 Artefacts as a table (artefacts.tsv); artefacts-1 goes; #1068 re-kit 0.26g: the weapon of a kill, a hit and a shot (research); #1059 Character and battery item as tables (character.tsv, battery.tsv); #1051 SR-3M 850 rounds a minute and +5 % movement; Vector armour piercing 0.3; #1030 Renderer study: 0.26g frame reference, light shader sources and PCSS prototype; #1070 MSBS: full damage to 35 m, 60 % from 70 m (was 20 / 50); #1073 Saiga-12 and Vepr-12: slugs take three, buckshot two only with every pellet; #1076 ASh-12: 425 rounds a minute (was 400); #1075 MP7: armour piercing 0.3 -> 0.35; #1077 Weapon pickup: a picked gun drops at death, and a drop lasts until that player's next death. Without a server sha256 `ad671509a013575bb87f74b34c2321a646dffe700348d87dce1632d4239d5cc6`, with `--server=skhid.link:1234` `f605fedc2d6d63d77d9bb15cdc3c57f1e98720b65171c2f5a7bd4604588530f6`. The ZIP: 7320290 bytes, sha256 `6622c83eec1d2000ba7dff982704a1a9b802decc80555419c2963a3c83adfe7b`; the server data: sha256 `2471c8519a179e451bc5d03f0ece53bd487b0460fa44c3deecadb4e27fed585c`. Its content: `resources.db.patch_42` (55384400 bytes, sha256 `442f376d154da3455c59ced8e7ef6470c07065ec3fb819c11d6c841d1c047872`) |
| Revision 43 | Released 2026-11-02 19:46 UTC (tag `026g-rev43`, `main` d6e046a). Shipped: #1071 Game modes as a table (modes.tsv); battery-respawn-1 goes; #1074 Modifications and defects as tables; weapon-rolls-1 goes; #1072 Maps' rules as a table (maps.tsv), one value a level; game changes 3-5, 10; #1064 Rooms: one table for the room settings (rooms.tsv, read by srp); #1084 PKP: recoil power 1 -> 0.85 (kicks a little less); #1082 Rooms: the vote's third card is always Blackout; #1096 Pickup: the drop's gun is its copy, from the death to the drop's end; #1080 Lobby: a kit slot's card stays that slot's item after a swap; #1085 Armour as a table (armor.tsv); five edit lists go; #1099 SOCOM 16: 330 rounds a minute (was 300); #1097 Saiga-12 / Vepr-12 buckshot x0.14 -> x0.15; Saiga-12 magazine 5 -> 6; #1101 Saiga-12 descriptions: drop the magazine's round count; #1102 Blackout: show the use prompt and its progress; #1086 Medicines as a table (medicines.tsv); #1103 Military Medkit: 2 s to activate, its heal 1 s as before; #1104 Medicines: one source for their numbers; #1108 New accounts: the grant in one table, the Novice's backpack on the back; #949 Weapons batch 1 from 0.31a2: OTs-14 Groza, FN SCAR-H, VHS-2, FN Minimi (pack weapons-031@1); #948 Weapons from 0.46c2: MP5, SVU, L85A2, Fort-401, Jackhammer and five pistols (packs weapons-046, weapons-046-camo); #947 Weapons from 0.69d0: AKMSU, FAMAS, G3SG/1, Mauser M.712, KS-23M, PPSH-41 "Drum" (pack weapons-069@1); #989 Tactical reload for the live one-clip guns from 0.29a3's clips (pack tactical-clips-029@1); #995 Tactical reload for the one-clip guns 0.29a3 lacks, from 0.69d0's clips (pack tactical-clips-069@1); #993 PKP Pecheneg: 0.69d0's animations (pack pkp-animations-069@1); #1117 Fix replay crash from live-client ragdoll lookups; #1118 Keep autobuy enabled across refill item refreshes; #1120 Balance imported weapons against Zubr +8 with distinct roles; #1113 Items as a table (items.tsv): golden and premium items are ordinary rows; #1115 Shop rules as a table (shop.tsv); one price, the item's base_cost; #1114 Golden weapons in the weapon table; golden-weapons-1 goes; #1122 Fix pickup state leaking between matches and causing resyncs; #1128 Fix Autobuy turning itself off when enabled. Without a server sha256 `4f3ffe97d4e3240cc87dafd84b4ea8871a3a047b829ff9d90efbcb4e761b9854`, with `--server=skhid.link:1234` `bdca89c64438b697860a89a97e954e396a2033e316b7ea7fd8d78041d3a0f62e`. The ZIP: 7324486 bytes, sha256 `c0cf6d181153f17d949da268fed33d729508f181a13251b62cfa6ead67ccb2d4`; the server data: sha256 `e130ac0d77f5c47946cfa774ab7e93e8aea99ba7c384b668608ebceb74377345`. Its content: `resources.db.patch_43` (888993912 bytes, sha256 `73d0efd06891daee682b0cdb07ce3cdbd16a611113f9c38bb5f69bc232b713f9`) |
| Revision 44 | Released 2026-11-03 06:36 UTC (tag `026g-rev44`, `main` 2881900). Shipped: #1130 Publish revision 43 patch notes; #1132 Render new weapon icons in retail 0.26 side profile; #1134 Fix STK2 clothing fits and reject boot-class mismatches; #1137 Fix AKMSU magazine and PPSh drum icon poses. Without a server sha256 `d5f1890ccbe2f27d4217bd9b172c800f49ec0e93273ebde282aaa141335b5d85`, with `--server=skhid.link:1234` `0f3e3ab4c637a743777b496d43c87014bb8048265be8bab712b7619c113c5622`. The ZIP: 7324641 bytes, sha256 `f5cb33d42148b61e96fa5dc002721da2739694a6250334a89800a6b83d6a2e18`; the server data: sha256 `93f71f699ca9706adca6a11ed44b743a6e65ab62deec6b2bd249c8003a0dc86e`. Its content: `resources.db.patch_44` (50452990 bytes, sha256 `6d3dd4a32909583bbcb38014f74ce54134e47da8ee22a98d4a3429cf2368fffa`) |
| Revision 45 | Released 2026-11-03 20:12 UTC (tag `026g-rev45`, `main` 8d6ae11). Shipped: #1144 Publish revision 44 patch notes; #1145 Preserve retail brightness in new weapon icons; #1150 Require modern WoW64 in the Linux launcher; #1151 Restore the Novice hood's original neckline; #1152 Simplify the Linux launcher and remove executable patching; #1153 Give SCAR-H AP-only 20-round ammunition; #1160 Install Wine Gecko silently for the Linux launcher; #1161 Store Linux game settings in the XDG config directory; #1158 Linux: finish ntsync setup before starting Wine; #1163 Fix orange and yellow medkit pain-healing totals; #1173 Set explicit reload timings and align tactical reload stats; #1175 Explain Blackout in System chat for players' first three matches; #1177 Add medical skills and shared Larkspur effects. Without a server sha256 `1cd1c4f6bbbd1cd3846772046f57aa82401b17f8d2820af70c4059eb6842e7fd`, with `--server=skhid.link:1234` `20262b562d2ed25cbf171e05121e04c33ca1f6d9732b67d3c7e4e0b299947f83`. The ZIP: 7327134 bytes, sha256 `676ed7decb71d48c077f049634cf8b4800ab597f2d950327f9e626322d7f9e9f`; the server data: sha256 `3ca182115b464cc3a2edd184c3a5d8461406a50173851ea9ee4d8de0a17bee7c`. Its content: `resources.db.patch_45` (116398844 bytes, sha256 `fc11460688e72f06faa5ea8f4a12b1858e7ebc467353b2e374eff3f348d5de91`) |
| Revision 46 | Released 2026-11-04 08:49 UTC (tag `026g-rev46`, `main` 74145dc). Shipped: #1179 Publish revision 45 patch notes; #1191 Fix Medic activation, booster limits and mode notices; #1154 Fix bullet lifetime disagreement after snapshot loads; #1192 Fix floating-point tails in weapon stat tooltips; #1193 Fix battery marker state updates while hidden; #1194 Keep hit markers visible when the crosshair is hidden; #1157 Stop resync retries from repeatedly adding the clock margin; #1195 Balance VSS capacity and falloff; shorten FAMAS reloads; #1196 New accounts: AK-74N and Remington 700 kits; the AKM, AKMSU and 10 stash maps; #1197 Vostok v0.11: the v0.26g spawn layout; #1198 Mode notice: draw its text with the countdown's embedded font. Without a server sha256 `9bb4da0db033cfc8f626d786810233047dd75268ba1682ec1ad71672f30af654`, with `--server=skhid.link:1234` `3b16fef66bdab98e7362f700b03b1358518983751b69281a0286f98c1ad3a9db`. The ZIP: 7327337 bytes, sha256 `19d4b1712717173d46c15580a9f199f5af79d27069bb3ef10af3a642335a2868`; the server data: sha256 `454f6da3b1988707f12f5fa99bbe11ae321312832b1fc07407d2b93934321219`. Its content: `resources.db.patch_46` (90159592 bytes, sha256 `a9de61802f49b8442f6c6281dbdcb5fba5ff4e312089a72d8a1b065d4cccd792`) |
| Revision 47 | Released 2026-11-04 10:11 UTC (tag `026g-rev47`, `main` f0c5c6d). Shipped: #1201 Mode notice: the countdown's whole text format, its bold cut included; #1202 Match drop: always a drop card's roll, whole; #1203 Team Deathmatch: a kill scores three times; #1204 Mode notice: 10 seconds after play starts. Without a server sha256 `85b63686ae86b3b1dbdbf988f57dcfd79346861ff8f9cda20da99a9cc982f7e5`, with `--server=skhid.link:1234` `81c1ba5284232dca9116376f8bdb0e5986a8a19b2a305fd7e14b8be740f2a227`. The ZIP: 7327438 bytes, sha256 `35716be522ec19386af819df7b729259ddae83248bf1dae0d108093ef00cd57a`; the server data: sha256 `89e4ae95be6e9136a943e406fd64f39b0caf5bfe8804d3c945ec14062c3ee53e`. Its content: `resources.db.patch_47` (7729141 bytes, sha256 `006ac2fbe30d8f0dad044b44a641bed88b1f70674593d4671d3358625940bcdd`) |
| Revision 48 | Released 2026-11-04 21:29 UTC (tag `026g-rev48`, `main` 9fb2913). Shipped: #1206 Drops: never a camouflage or decal the player has. Without a server sha256 `8761786b4f994a6e7e2fea86cefbc90a5394251cbd06de487d2fb44327268d41`, with `--server=skhid.link:1234` `b4c3fc52127f3551a7d87e2090c3c49d51082b1e12ff03fec5105ca233da355d`. The ZIP: 7327433 bytes, sha256 `d3f0567bb2016151757d23682e84281fe836c609907fbe08519ee28768f37406`; the server data: sha256 `89e4ae95be6e9136a943e406fd64f39b0caf5bfe8804d3c945ec14062c3ee53e`. No content patch of its own |
| Revision 49 | Released 2026-11-05 09:40 UTC (tag `026g-rev49`, `main` 68d3a70). Shipped: #1240 Linux launcher: use Wine's Wayland driver under xwayland-satellite; #1243 Blackout: a mode of its own maps; type the configs' modes; #1238 Research: never read a departed place's player; #1242 Results screen: a NULL map name reads as ""; #1231 Lobby: hold Alt to see a medicine's numbers after the medical skills; #1245 Patch 95: log the animation pair the registry has no id for (rev47 crash at 0x1F18C8); #1253 Patch 99: a world takes no player input whose rotation no client makes (0.27d's "input is invalid"); #1257 Patch 102: a side mouse button is no left click in the menus; #1255 Expect starter kit level 7 in the bot-match end-to-end test; #1270 srp-recording-to-sur: read the recording's dictionary; keep chosen key frames; #1256 Patch 101: a footstep first made by a re-simulation is heard (random step drop-outs); #1258 Patch 100: the sound follows the audio output device (B43); #1247 Patch 96: shots stay audible under heavy fire; #1265 Ported guns: other players' shots heard to 200-240 m (pack weapons-reverb-layers@1); #1248 Hash worlds up to 0xF000 bytes; history item sections grow on the heap (patch 98); #1254 Match server: skip a resync whose game already agrees; capture both worlds at a split's first tick; #1249 Crash dumps: full memory on SKHID_CRASH_DUMP=full, not a build feature; #1260 Crash report: the patch log's tail in crash.txt and the ZIP; the time in UTC; #1261 Crash report: the match and the last 1024 notable events (breadcrumbs in the arena's state); #1264 Crash report: what the game was reading when a load crashed (player and stream offset); #1267 Crash dialog: with SKHID_CRASH_DUMP=full, send the report and keep the .dmp; Copy fits one Discord message; #1266 Player log tools: the crash ZIPs' text in the collected logs; the crash files' labels on Linux; #1259 Server logs: one size-rotated file per server in the data volume; #1246 Weapon pickup: every client installs the recipe's gun (rev47 affects_count crash); #1273 Weapon pickup: a player can take its own drop; #1275 Animation ids: the cause of the 0x1F18C8 / 0x3B336C crashes (docs); #1286 Ammo cards: bullet speed, range and pellets, in the weapon card's rows; #1281 Linux: install the font Calibri into the Wine prefix; #1276 An RGD-5 takes two pocket units; #1289 Note the one-off RGD-5 pocket reset at release; #1290 Grenades: CCD sweeps a sphere inside the capsule, no more falling under the ground (patch 105); #1288 Settings: Names on map (Tab map names without Alt); #1296 Groza: less damage (0.34) and a harder upward kick; #1292 Groza: 0.31a2's own aim locator (weapons-031@13); #1293 Collimators: the 9A-91's Cobra dot at the screen's centre; #1200 Publish the players' notes of revisions 46, 47 and 48; #1155 Preserve battery taker statistics across snapshot loads; #1299 Remove the two unlearnable physical skills; Packrat to column 4; #1301 Pre-release fixes for revision 49; #1295 Wiki: Skills, every skill tree as the lobby draws it; #1302 Chat text follows the keyboard layout under Wine (patch 107). Without a server sha256 `b29b7897dd4aecda02eba07879bc4405d59b64ad9fd3596cdb30509107d8c86d`, with `--server=skhid.link:1234` `d6d05e896f9788556b97458469689976462a6c1d8479b1d0213105e4fbedac17`. The ZIP: 7349372 bytes, sha256 `528559910913770cc8c28555144fb7c7d5b18b570f0758aee4948834804ec97c`; the server data: sha256 `728548761e040ff6c3ba4ceda00a545c167365c08f19c049413761de058a7866`. Its content: `resources.db.patch_49` (78724421 bytes, sha256 `2b61de5e2953072eb95c9665ae21921d2d4f57f51ea825f3adcc20f99b78b3ab`) |
| Revision 50 | Released 2026-11-05 19:51 UTC (tag `026g-rev50`, `main` 897258a). Shipped: #1308 Lobby: only a buy started in the shop asks about an item owned already; #1311 Grenade and trap cards, with Alt after skills; #1312 Artifact-master: artefacts recharge 20% faster (built-in bonus 1310); #1268 Medical tree retune: combo skills, fewer ranks, tier-5 either/or; #1315 Regeneration booster: regenerates to full health, starts 2 s sooner; #1313 Blackout: Tab no longer stops a held Use (artefact search, refill); #1316 Gun-stock hits to the hand kill (patch 108); #1330 Patch 110: keys the desktop moves are moved in the game under Wine's Wayland driver; #1283 Test bullet hits at the crossing millisecond, not the 50 ms grid; #1323 Opt-in Vulkan renderer (DXVK) for Windows players; #1326 Force Protein: gun-stock kills by its carrier send the ragdoll flying (visual); #1338 Ragdoll: no travel cap, a thrown body flies on until friction stops it; #1334 Calculator: a wall in front of the target; #1339 Login and main menu: closing a message gives the focus back (Esc, close, Enter signs in); #1341 Renderer list: open under its box, and the restart hint; #1331 Patch 70 (bake_cache) in Rust; #1343 Lobby: held Alt, Bear Arms shows a cancelled movement penalty; #1342 Calculator: the formula as numbered steps with this shot's numbers; #1347 Rooms: private rooms with bots; #1328 Mouse look: "Lock vertical sensitivity": 16:9's look at every window when locked, 1:1 when unlocked (patch 110); #1348 VPN.txt: zapret settings and AmneziaVPN's white-list mode; #1349 hud-options harness: Controls part passes again; mouse sliders at their borders; #1346 Calculator: the glass wall is just Glass; #1340 Weapon card: Pierces, the best round per stat, shotguns by kind; #1350 tools/check: run only the checks a change affects, cached by their inputs. Without a server sha256 `c787458848ead38a576d0d4cfb459f7671e32a0296b10d3389d9acf4f7362b18`, with `--server=skhid.link:1234` `e280d52c8323a58880c4b08341d7b0c701b32f00c05f84d33960896732a2436e`. The ZIP: 16772952 bytes, sha256 `d9d6df1923cd28b9ffb5c9f0882dbbaf56dc8a5b40485a211e63d00a45d03348`; the server data: sha256 `d565c500eda083a7fdfc28f68f2d309336af6f804ebbcc68f7595d9fa819ccf2`. Its content: `resources.db.patch_50` (78271648 bytes, sha256 `54b08986b12559260b02550e4e2dd4788ac3ab5d6ecf838de99ffe379f6192d2`) |
| Revision 51 | Released 2026-11-07 20:47 UTC (tag `026g-rev51`, `main` 1a93e40). Shipped: #1351 web-server tests: give the stock account the silver the admin takes; #1355 build-content: cache unchanged steps (no change 54 s, lobby change 112 s, was 424 s); #1359 DXVK on Windows: bind the game's dxgi, d3d11 and d3d9 to DXVK's DLLs; #1358 release.sh: swap as soon as no match has run for 30 s; #1354 Lock vertical sensitivity: Apply keeps an unlock; the greyed slider follows set values; #1363 Patch 108: a gun-stock hit kills wherever it lands; #1368 Trap 0.55 a foot; the painkiller's pain protection covers it; #1369 Take off what a skill opened when the skills change; #1372 Patch 113: kits and cursor start over at each match; the main menu covers the hud's cursor; #1370 Item search: "+" words for levels, factions and bonuses; #1373 Everyone at level 15's last point; new accounts with five skills; #1388 release/moderate: watch registrations and sign-ins, ban, mute, close sign-ups; #1380 VSSK rifles: Vychlop gets aiming time -10 %, Dusk reload +15 % as their one built-in; #1389 Boosters: every built-in bonus shows gold on the card; #1390 Lobby: held Alt, medicine cards show Numb Legs, Unflinching, Concentration and Take Cover; #1375 Modification window: the item's card is the inventory's, at its size; #1376 PPSh-41 Drum: seat the drum in the magazine well (weapons-069@15); #1391 SVU: its iron sights raised, its eye on them (weapons-046@13); #1382 Item search follow-up: Shift+= types +, penalties are no bonus, bonus words by type; #1360 Equipment 046@9: four new pieces; the NVD as open goggles; the Sniper set at level 11 and the Veles set at level 13 (the owner's numbers); #1364 Equipment 069@5: the Snow-20 set (the level-11 Sniper's twin) and the Delta mask; #1393 Drop one-off checks of shipped changes; #1397 One Ruffle runner for the movies' harnesses; #1399 Release tests: the early swap's 30 s scale with the test's minute; #1400 build-content: read the pierce lines' files only when a list asks for one; #1402 Medical skills' texts in the language TSVs; #1395 Release: build the server image here, load it on the host; #1408 Wiki weapons: choose the round; #1412 release/moderate: reset-password, watch, give, money, fuzzy who; #1367 Graphics: Vulkan options (frame limit, overlay, low latency, tear-free, dedicated GPU); FOV to Gameplay; #1398 Fix the artefact-carry and language-option harnesses; #1407 Self-test: fix the four stale exe-patch checks (lohook dispatch, patch 96 exits, artefacts, configs.start chain); #1414 Self-test: derive the pinned checks from the declarations, run it at a release; #1415 build-content: packs keyed by the movies they edit, lamps cached, a quick build by default (--full for releases); #1180 [Packet safety v2 1/11] Shared message parsers (inert); #1434 World checks: take the worlds the game writes for a NaN player and a shooter who left; #1458 Read uploaded worlds as typed views, every field held to what the game writes; #1423 World check cost: a bench over recorded matches, sorted compatibility pairs; #1421 Packet safety v2: catch a world check that is wrong before a release, and loudly in a match; #1463 check_bench: check with the server's Roster (main does not compile); #1461 Patch 116: srp's ban reasons and the ban's end on the sign-in screen; #1466 Notes: what players' games send each other, and how it is checked; #1468 match-server: fall back to the games' own check for a kind in doubt (temporary); #1464 Send every kit to the match with its pockets packed (crash of match 0x55809714); #1467 Patch 117: a presenter ignores an event for an effect it holds no entry of (rev50 crash 2026-11-06); #1471 test(srp): fix stale v026g_web assertions; #1474 Sell the rank and faction badges in the shop; #1469 Patch 117: a character controller's step never leaves its position non-finite; #1470 tools/check: world-check and world-drift over a private world corpus; #1475 AGENTS.md: the world format and its check; #1476 World check: no NaN in a player's speed, last position or a bullet's origin (after patch 117 is live); #1477 Results screen: a map is always usable, bought on click, its item shown again; #1478 Renumber controller_position to patch 118; #1479 Lobby build: drop RESULT_SHA256; #1480 Lobby: the match pop-up over the room window, gone at the next press; #1481 simulation_changes: controller position fix is exe patch 118; #1486 Rooms: four ready Play waiters open another room (was six); #1485 Read the refusals' clock from GetTickCount, not GetCurrentProcessId; #1487 Do not count invalid inputs of a world out of step toward removal; #1490 Input safety: 100 invalid inputs in step remove a sender (was 8); #1492 match-server: never relay a refused world; give every waiting game an end; #1493 Rooms: say why a match can't take a joiner when it has no world (J;-1;8); #1494 Rooms: J;-1;8 tells the member it joins the next match; #1495 Patch 119: a kicked game goes back to the lobby as a lost connection does; #1392 Artefact skill tree: the fifth tab, its eleven skills and Forest Protection's glow; #1489 Receipts: one per event, none dropped for time; #1482 World load receipts: admit on Loaded, answer each refusal once, no receipt timer; #1498 simulation_changes: restore the separator before the patch 118 clause; #1499 Refused worlds: count only a game's own refusals; end a match with no first world by events; #1500 Receipts: number refusals per connection, a waiting slot per kind; #1501 Patch 120: a match its game never played closes too (kick/timeout/lost link while loading). Without a server sha256 `e3535b4de5d1e15d448dbb8f7cbdcc46edfa95800b4854bfb7aa22ea50b818ba`, with `--server=skhid.link:1234` `8119f24cb6f1b04b70c77ceccb8c64e6f29f0b1766b97a5615eb69d2f567eceb`. The ZIP: 16825683 bytes, sha256 `f099cafcc4a772fd0dd6257e680e934631bd6edb15335ab19ac3a9bb2a7b0dc0`; the server data: sha256 `58e09e73440042dc90a51c1fd4232eb20585e52b3d278edf1fcff8e592aba10c`. Its content: `resources.db.patch_51` (272668357 bytes, sha256 `55051281da0cd73bf3dcb95e7333625b9c2c43fb518f3970e57de8deeda8e928`) |
| Revision 52 | Released 2026-11-07 21:40 UTC (tag `026g-rev52`, `main` 13d1b71). Shipped: #1505 Release: exe self-test beside step 2, on every core; --no-selftest; #1506 Patch 109: fix the crash at a rollback over a Keeper's respawn into another kit. Without a server sha256 `4458593de9f834f12cfc31a5f60a7116c5fa18d47bb9e513e7f07b57cd7677de`, with `--server=skhid.link:1234` `2235be3c078695b1f4d9709ce9a498506c796d6158a3faf38be5b378bb7d5ca4`. The ZIP: 16825944 bytes, sha256 `a306cb24db938fcbcb772c6f37405443ffb442e0f61242e9c92ac74a347eae02`; the server data: sha256 `58e09e73440042dc90a51c1fd4232eb20585e52b3d278edf1fcff8e592aba10c`. No content patch of its own |
| Revision 53 | Released 2026-11-07 23:16 UTC (tag `026g-rev53`, `main` 164c311). Shipped: #1510 Patch 109: every kit's artefact slots made the world's at each world load (rev52's start crash); #1509 Cascade: every kind's activation shared (Medic's path); Resonance 9 m, Cascade 6 m. Without a server sha256 `a048ccb2671290e9bc670e792ce8217cce05779f7803265c031a7b0025b4edef`, with `--server=skhid.link:1234` `ab49f5483701c2197e2e5542a9cd427a5c01c8113a610439c2394962e3cfae9e`. The ZIP: 16833325 bytes, sha256 `92dd2e8987a14d5f34432f7d8df0b7b77b7e88ee79ea721aa6e3a777f7e6ab5c`; the server data: sha256 `d9330961e00975ec1b91baefaf97554acd8877dd0c612718318f2b4d62e2564c`. Its content: `resources.db.patch_53` (3149249 bytes, sha256 `83b504298fd2fca957b881724d4bcd9d3d48e0ba88e69b8b6dd6f1cf5d04be28`) |
| Revision 54 | Released 2026-11-09 16:06 UTC (tag `026g-rev54`, `main` f12d76c). Shipped: #1457 feat(client): macOS support; #1526 Wiki skills: levels, their formula and a graph; #1528 Wiki: no footer, the header is enough; #1527 Wiki: quests page; #1530 Content: ten more 0.69d decals (pack decals-069d@2); #1536 Content: the "Alpha" Squad badge costs 10,000,000 silver; #1520 New accounts: the Keeper set in preset 2 (AK-12, Saiga-12); premium bonuses +20 % / +50 %; #1548 Patch 113: never take the hud's cursor movie off a scene twice (crash at exe+0x14EE8D); #1542 Local launcher: refuse an exe older than the server takes; #1549 Lobby: Esc over the results after a match closes them, not the game's menu; #1523 Silver only: remove gold from the game; #1551 Patch 83: a DXVK fullscreen window DXGI refuses becomes a popup (Vulkan on AMD); #1550 Patch 113: the main menu gives up an awaited key before it closes (a latent crash at exe+0x14EE8D); #1472 fix(updater): report progress while extracting; #1539 Nerf the SCAR-H, vz.58 and VSK-94; #1517 SV-1367 Dusk: base rarity 0, so it can craft and roll modifications; #1502 Keeper of the Artifacts: Onyx rare (48/48/4); #1518 Delta mask and NVD armour 5, NVD sway -10 %, Sniper vest pocket back; #1557 Dev shell: share one sccache across worktrees; #1519 Give the VKS Vychlop weight -15% instead of aiming speed; #1558 Hold every kit to its layout: pockets, pocket size, second weapon; #1556 Patch 124: resources helper arena 32 MiB, render arena 320 MiB by default; #1560 Artefact tree: Forest Protection and Keeper of the Artifacts go together; #1473 fix(client): Linux Calibri from a pinned mirror; #1562 Concentration: sway -5% a rank while the painkiller is active; #1563 Wiki skills: how many players took each skill; #1561 No premium account: its half again is everyone's pay; #1565 Skills: seven stock skills and Quick Hands take 3 ranks, each worth more; #1540 fix(harness): measure text with the movie's fonts; #1566 Wiki skills: plan a build by clicking skills; #1569 Patch 128: engine threads get 2 MiB stacks, not 8; #1567 Sides: strong players dealt evenly, at the start and on late joins; #1564 Patch 126: the game reports its memory headroom to the match server; #1570 Refuse a hit effect with a non-zero root-emitter registry index; #1571 Token backpack back at level 10; #1516 Token backpack: pocket capacity +1 beside its artefact container; #1491 Use: Rust detect_usable_object; a usable battery beats a dropped gun; #1568 Patch 127: a memory profiler of the engine's arenas (SKHID_MEMORY_PROFILE=on); #1572 Make the 0.21d Radar's electric anomalies hit the legs only; #1574 Use scan: deep review follow-ups; #1573 Patch 129: the multithreaded arena is at least 1024 MiB; #1579 release: the swap check reads the new srp26's whole log. Without a server sha256 `99596d9be4c8777bdd833bbcf20265150c27d0939ccf886319bce4e51955fcd3`, with `--server=skhid.link:1234` `7e534b8de1a5bfc671a054d1d3c6ceb9824937af73b941e0906bc0c6fa58aa33`. The ZIP: 16852379 bytes, sha256 `3514528737a7a5045fbaa01693cb01add74b6391b82fb9ecddcc473d101c3409`; the server data: sha256 `1b74758ae799ad13737258de9b83b1fea50888000341e53f56a51c8c9c55e64f`. Its content: `resources.db.patch_54` (73593265 bytes, sha256 `8314c83d46f7813d8f2d45620add72179b3f0aa1c956192ac1ec8c2974bcbbfa`) |
| Revision 4 | Revision 3 with patch 10 (a start with a Documents folder the game may not write to). **In the players' download since 2026-10-13 15:36 UTC.** Made as the download is, it differs from the revision 3 exe (`ac947abd…6d7b`; without a server `eedc5b5c…cf0d`) in the four bytes at file 0x35F0B, two bytes of an address at file 0x8410C8, the revision byte and the PE checksum, and in nothing else (compared byte by byte). The built exe was started in both situations of patch 10, not past the sign-in screen, before the release; the first sign-in with revision 4 on the players' server was logged three minutes after it. It plays together with revision 3 |
| Revision 3 | Revision 3 is revision 2 with patch 9 (the battery rule). **It was the players' download from 2026-10-13 12:22 to 15:36 UTC** (the rows below are the downloads so far): it and the revision 2 exe differ in the 13 bytes of patch 9, the revision byte and the PE checksum, and in nothing else (compared byte by byte). Its rule was run outside the game (`run_battery_rule.ps1`). Eight sign-ins with revision 3 were logged in those hours; whether one of them played a match to a team's last battery was not looked at. A game of revision 3 or later and an older one part at a team's last battery (patch 9) |
| The players' download | `Survarium-0.26g-srp-files.zip` of skhid.link (and 026g.skhid.link), since 2026-10-13 15:36 UTC: 6 793 103 bytes, sha256 `29b2ac002a1787c1352d021065f18a87ea5caa66fb16298c8e603a5347416bb4`, 23 files, made by `make_client_026g.py --pack-zip --server=skhid.link:1234 --localization=english` of `main` dbd70f0 with `--bugtrap=` naming the build of `client/bugtrap` that had been run in the game (that DLL does not build to the same bytes twice). It differs from the download before in two files: the game exe (sha256 `ae3ddf43f38eecb9e5080b10aa6b8397ce5dbed0f133b381461c82cb17329f25`, patch revision 4: patch 10) and `bugtrap.dll` (39 936 bytes, sha256 `9e3295c1312854c455c474e1dbd457b0a792b5ae406ff0b55d3b19b4f2fc4353`: the start-up notice of `client/bugtrap/README.md`); the other 21 files are byte for byte the same. Built twice: the same ZIP. Checked after the replacement: `/api/client-files` and `/download` of both names give this sha256. No restart: nothing on the server changed |
| The download of 12:22 UTC | 2026-10-13, 12:22 to 15:36 UTC: 6 788 856 bytes, sha256 `a63842de812d0d8057f1d357795f02e4176a1e9ff022742bde24f020988da9a9`, 23 files, of `main` 78070c5. It differs from the one before in five files: the game exe (sha256 `ac947abd212d05f3172035967076517f057019b9e1499be06c28b622baf36d7b`, patch revision 3: the battery rule), `updater_skhid.exe` (93 184 bytes, sha256 `6502180e038fe384451adb5216100072a9e985e2eae1014c541a113baef60d8e`: it installs the original game for a new player), `7zr.exe` (new; 602 624 bytes, sha256 `256feca8e274e5da655e2a284fabafd9f554365eb164862089dacd4e8276d282`), `README.txt` and `LINUX.txt`. An updater with the ZIP before installed took the five files at its next check |
| The download of 10:30 UTC | 2026-10-13, 10:30 to 12:22 UTC: 6 440 522 bytes, sha256 `efccf0ce1f36a8bf0ec21b6751c4094d5cdf777cc1b03968d69f975697d9cd13`, 22 files, of `main` 3d6ab12. It differs from the one before in the launcher's ten local pages only (the original artwork, a Discord link). The first release that reached an installed launcher by itself: an updater with the ZIP of 09:50 installed took the ten files at its next check |
| The download of 09:50 UTC, the first with the launcher | 2026-10-13, 09:50 to 10:30 UTC: 6 293 491 bytes, sha256 `adb90bccf0254bc2c9fbc8710ffaa9152f95948fd46dcb96a390e04124cc0c4f`, 22 files, made by `make_client_026g.py --pack-zip --server=skhid.link:1234 --localization=english` of `main` 4a68dc7 with `--bugtrap=` naming the `bugtrap.dll` of the download before. It is that download with the launcher (`launcher/README.md`) added: `launcher_skhid.exe` (4 735 488 bytes, sha256 `9110bf7526794d73c2d3713f61b509c526dd8d442e442ca5e08db77427b97d38`), `updater_skhid.exe` (79 360 bytes, sha256 `0ae3c3463b95073fe29797d87dd2ffda3be911832fb489ffb1b9daec299949fd`), `updater_skhid.url` (`https://skhid.link`), the ten pages of `launcher_skhid/`, and `Skhid.bat` and `Skhid.sh` beside `game/`. The game exe (sha256 `30e928dd…4883`, patch revision 2), `bugtrap.dll` (`3d8190b3…1d57`) and the login certificate are byte for byte those of the download before; `play_skhid.bat` is too, and `play_skhid.sh`, `README.txt` and `LINUX.txt` have the launcher in their texts. From this download on the launcher keeps a player's Skhid files the same as the ZIP the site offers, so a release is: build the ZIP, replace the file |
| Checked, the launcher | 2026-10-13. The same build made for the test server (it differs in the two addresses only), over HTTPS with a real game folder on Windows 11: unpacked over a fresh copy of the game it added 20 files and replaced two (`bugtrap.dll`, the certificate); the first start took the ZIP once and changed nothing; a second ZIP put on the server, differing in the launcher, its pages, `updater_skhid.url` and `README.txt`, was installed at the next start (13 files, the launcher replaced itself and came up again, 2 seconds in all; of 58 files in the folder 43 were untouched and the other two were the updater's own state and log); PLAY started the game, which signed in and reached the lobby. This download itself: taken from the site, unpacked, and `updater_skhid.exe check` run in it (the ZIP fetched, 22 files right, nothing written but its state). Not run: Wine; a Windows other than 11 |
| The third download | 2026-10-12 21:20 UTC to 2026-10-13 09:50 UTC: 4 918 126 bytes, sha256 `fa73afa4c897dd099acd88021576b3fc173ae7c0b6b917e2795b318a6a05ff48`, seven files, made by `make_client_026g.py --pack-zip --server=skhid.link:1234 --localization=english` of `main` 955e37d (English is the language the launchers start the game in; a player changes it in `play_skhid.bat` or `play_skhid.sh`). From 21:10 to 21:20 UTC the site had the same zip with `ukrainian` in the two launchers (sha256 `4ac6ecf61c2869fb3c8587a89e7f020c55de79f8641cf5ef854b1a1b87b2e8fc`); every other file is the same in both. Its exe is the full section patch with that address inside it (patch revision 2, 12 895 352 bytes, sha256 `30e928dd3c19a25452293c11be5e0bc85fdcacbbe0b957941a1e34c7be944883`); `bugtrap.dll` is the Rust crash reporter built that day from the same commit (32 256 bytes, sha256 `3d8190b377a7534b76f7c21f99d95106abafdf91dc342a7ad034db0cd52f1d57`); beside them `play_skhid.bat`, `play_skhid.sh`, the login certificate, `README.txt` and `LINUX.txt` |
| Checked | 2026-10-12: the exe differs from the one of the download before it (revision 1) in the ten bytes of the missing-server hook, the revision byte, one relocation entry, the 67 bytes after the upload code (the stub and the address) and two header fields (the checksum and the section's size), and in nothing else (compared byte by byte). An exe from the same tools with another address inside (it differs from this one in the address and those two header fields only) was started with no `-server` option, signed in to the test server (logged as patch revision 2) and began a match there; the exe of the download itself was started with no option and came up. The crash reporter: one crash made on purpose in the game on Windows 11 gave its dialog and a dump that reads as a proper minidump (191 MB: it holds the game's data segments). The Linux launcher was not run by us. **`bugtrap.dll` cannot be made again byte for byte**: a second build of the same source gave another sha256, so the file in the download is kept, not rebuilt |
| The second download | 2026-10-12, 18:43 to 21:10 UTC: 4 871 691 bytes, sha256 `16eb1f23c99d5d73c5228c168021c58adff5e0ebf3d543984f916b416cef9528`, four files; its exe is the full patch of revision 1 (sha256 `3c7d7bf0195db000f0c6dc81e57fa15a50decec4fa0c0ea5494f361f91049c9d`), which the revision-1 tools gave byte for byte. A game with it signed in to a local server (logged as patch revision 1) and played a match. Revisions 0, 1 and 2 play together: none of the differences is in what a match computes (by the comparison of the bytes; a match of a revision 1 and a revision 2 game was not watched) |
| The first download | 2026-10-12, until 18:43 UTC: exe sha256 `166d25bcc49f6e1b83fc0d85c25e822c85c7284d4d846143358ca2f28bd31ddf`, the full patch without patches 6 and 7 (it says patch revision 0). The revision-1 exe differs from it in the one byte of patch 6, the six bytes and one relocation entry of patch 7 and the PE checksum, and in nothing else (compared byte by byte). The two play together |

## The files

| File | What |
|---|---|
| `patch_026g.py` | Builds the patched exe from the original. Python 3, and for the arena gcc (`-m32`), GNU ld and NASM 2.16 (`nix develop ./srp-026g`). Holds `ARENA` (the `.srpx` blocks and their sources), `INSTALLS`/`CALLBACKS`/`INSTALLED_VTABLES` (every installed function's model) and the checks of both (arena and call shapes, above; `docs/notes/client_patch.md` 0.3, 0.4) |
| `snapshot_upload_026g.asm` | The source of the upload code (nasm) |
| `snapshot_upload_026g.section.bin`, `.hash-mismatch.bin` | That code assembled, for the two places it can go; `patch_026g.py` reads them. Assembling the source again gives the same bytes |
| `late_join_026g.asm`, `late_join_validate_026g.c`, `late_join_runtime_026g.c`, `late_join_install_026g.c`, `late_join_runtime_026g.h` | The source of the late-join code (patch 11): the hooks and the game's calling conventions in assembly, the checks of what the server sends and the bookkeeping in C |
| `late_join_026g.section.bin`, `late_join_026g.section.json` | That code built, which `patch_026g.py` reads, and beside it where the C functions are that the assembly calls and what built it (gcc 13.2.0, GNU ld 2.41.90, NASM 2.16.01). The same tools give the same bytes; another C compiler gives other bytes that do the same, so the file is kept, not rebuilt for a release |
| `late_join_026g.py` | Builds those two files (`--write-bin`), says whether they are what the sources give on this machine (`--check-bin`), or builds an exe straight from the sources for work on the code. Needs nasm, a C compiler for x86 and GNU ld |
| `check_late_join_026g.py`, `test_late_join_runtime_026g.c` | Checks of the late-join code: the first runs the patched machine code on made-up objects (needs Unicorn 2.1.4), the second the C bookkeeping under the sanitizers |
| `check_fov_026g.py` | Runs the original and patch 22's camera projection, the game's matrix builder included, and compares the matrices (needs Unicorn) |
| `check_fov_lod_026g.py` | Runs patch 40 (a model's LOD and screen factor at a wider field of view are the original's): the projection's k at several settings and zooms, the game's select_lod and screen factor of the original and the build on made-up models (needs Unicorn) |
| `check_use_scan_026g.py` | Runs the original's use scan and the build's (patch 23, the Rust `use_scan`) on made-up ray hits; `--against <exe>` requires another build (the asm patch 23's) to answer every case the same (needs Unicorn) |
| `replay_seek_026g.asm`, `replay_seek_026g.section.bin` | The source of patch 26's code (a replay's seek empties the weapons' sound queues) and that code assembled, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `default_server_026g.asm`, `.section.bin`, `.hash-mismatch.bin` | Optional default-login-address stub, appended to the snapshot cave; source and assembled code for both placements |
| `make_client_026g.py` | Makes the players' overlay folder or ZIP: patched exe, Rust BugTrap DLL, certificate, start file and README; with the launcher, DXVK 3.1.1's x32 DLLs (`winetricks dxvk`'s set: d3d8, d3d9, d3d10core, d3d11, dxgi) in `game/binaries/x86/dxvk/` (its release archive and DLLs pinned, downloaded once into `~/.cache/skhid`; `../docs/notes/dxvk-option.md`) |
| `play_skhid.sh`, `LINUX.txt` | Linux Wine/DXVK launcher and instructions, included in the generated folder and ZIP |
| `play_skhid_mac.sh`, `MACOS.txt` | macOS (Apple silicon) launcher and instructions, included in the generated folder and ZIP: downloads a pinned Wine (athei/wine-build `cx-26.3.0-8`: CrossOver 26.3's sources, msync, DXMT) and SWTOR on Mac's raw-mouse Mac driver once into `~/Library/Application Support/Skhid`, laid out as `Survarium.app` (games category, `LSSupportsGameMode`) so macOS Game Mode applies; runs under Rosetta with the engine's x87sidecar off (it desyncs); `Skhid.sh` runs it on macOS, `Skhid.command` from Finder |
| `stale_dll_026g.asm`, `stale_dll_026g.section.bin` | The code of patch 25 (the sign-in reports a stale `shell_32.dll` in `%TEMP%`, a file one known cheat leaves behind) and that code assembled for 0xE00 of `.srp`; `patch_026g.py` reads the .bin |
| `check_stale_dll_026g.py` | Runs patch 25's check on made-up machines, loaded as is and rebased (needs Unicorn) |
| `mount_patches_026g.asm`, `mount_patches_026g.section.bin` | The code of patch 27 (the game mounts each `resources.db.patch_<n>` over `resources.db`) and patch 49 (it stops at its start without a patch the exe requires) and that code assembled for 0x400 of `.srp`, its state at 0x800 of `.srpd`; `patch_026g.py` reads the .bin and writes the required patches' bitmap into its copy. Assembling the source again gives the same bytes |
| `check_mount_patches_026g.py` | Runs patch 27's two hooks and its code on made-up game folders, with no patch required and with patch 49's required patches, loaded as is and rebased (needs Unicorn) |
| `forward_scroll_026g.asm`, `forward_scroll_026g.section.bin` | The code of patch 28 (the transparent effects scroll as in 0.29, `scene_slomo` bound to 1.0) and that code assembled for 0xB00 of `.srp`; `patch_026g.py` reads the .bin. Assembling the source again gives the same bytes |
| `replay_link_026g.asm`, `replay_link_026g.section.bin` | The source of patch 29's code ("Watch replay": the lobby's link starts the updater) and that code assembled, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `check_replay_link_026g.py` | Runs the original and patch 29's link callback on made-up links (needs Unicorn) |
| `shared_upgrades_026g.asm`, `shared_upgrades_026g.section.bin` | The source of patch 30's code (one upgrade on several items: taking it off names the item) and that code assembled for 0x700 of `.srp`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `check_shared_upgrades_026g.py` | Runs the original's and patch 30's upgrade-window drop and the `83 46 01` / `02` answers on made-up inventories with one upgrade on several items (needs Unicorn) |
| `kit_change_026g.asm`, `kit_change_026g.section.bin` | The source of patch 31's code (a kit chosen before the match starts is played at once) and that code assembled for 0xEC0 of `.srp`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `warmup_look_026g.asm`, `warmup_look_026g.section.bin` | The source of patch 31's camera before the start (while the kits are shown, a drag turns the warmup camera and the wheel moves it, as the lobby's character view) and that code assembled for 0xD40 of `.srp`, its state at 0xE20 of `.srpd`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `death_camera_026g.asm`, `death_camera_026g.section.bin` | The source of patch 46's camera on the death screen (a drag turns the view from the corpse's eyes; the wheel does nothing) and that code assembled for `.srpc` (0xA429000), its state at 0xE60 of `.srpd`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `warmup_keys_026g.asm`, `warmup_keys_026g.section.bin` | The same patch's buttons: one held from before the start (a drag of its camera) is not the player's until it is let go, and that code assembled for 0x9A8 of `.srp`, its state at 0xE30 of `.srpd`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `warmup_limits_026g.asm`, `warmup_limits_026g.section.bin` | The same camera's end of a drag and its limits (the lobby camera's: distance 1.5 to 4.6, pitch within 60 degrees) and that code assembled for 0xA98 of `.srp`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `read_order_026g.asm`, `read_order_026g.bin` | The source of patch 33 (the order of the archive reads is a strict weak ordering) and that code assembled at 0x10BCC0, the 0x1A0 bytes of `hdd_manager_sorting_predicate` it replaces, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `check_read_order_026g.py` | Runs the original's and patch 33's predicate, with the game's own path, string and offset helpers, and the game's own sort on made-up read queries over several archives (needs Unicorn) |
| `rust/patches/` (`src/suicide_key.rs`, `src/suicide_key.toml`), `hooks_026g.py`, `port/equiv/suicide_key.py` | Patch 34 (the commit_suicide key kills one's own player), the Rust framework's example: the patch module and its hook TOML (a `[[hook]]` naming the site `input_builder.spot_query`), generated into the block `patches.code`; and its spec for the port-only equivalence gate. Until the framework, NASM at 0x980 of `.srp` |
| `rust/patches/src/vostok/`, `engine_026g.tsv` + `fields_026g.tsv` + `sites_026g.tsv` -> `raw_026g.rs` (`engine_026g.py`) | The framework: `raw` (generated: a typed handle per kind with its fields, the functions typed by them, each site's typed registers), `vostok` (the safe API over those handles and the hook system), the patch modules. `docs/notes/rust_patches.md` |
| `sites_026g.tsv` | The game sites the Rust patches hook, by name: each a VA, a kind (call/splice/...), the callee it uses and a short note. A patch names a site; the addresses live here, not in the patch (`docs/notes/rust_patches.md`, "The design principle") |
| `spot_026g.asm`, `spot_026g.section.bin` | The source of patch 38's code (a spot's hit on a battery's body spots the battery) and that code assembled for 0x7C0 of `.srp`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `check_spot_026g.py` | Runs the original's and patch 38's input apply (the spot and its wait, the player's tick's countdown) with the game's own spottable code of players and batteries on made-up ray hits (needs Unicorn) |
| `artefacts_026g.asm`, `check_artefacts_026g.py` | Patch 54: a Spring's passive and active take aim time from its config and its active takes the bullets' slow away, a Lifebone's active fills pain, body and head, one press fires every ready artefact of its kind; the arena's block `artefacts.code` (linked by `link_026g.py`); its check runs Spring's loader and links, the modifier getter, Lifebone's active and the action with the game's own code on made-up artefacts, and the press through inventory::action and Spring's own enable/disable_active (needs Unicorn) |
| `check_artefact_glow_026g.py` | No patch: the artefacts' first-person glow is the content pack `artefact-glow` (`../scripts/artefact-glow/make_pack.py`, `../docs/notes/artefacts.md`). Runs the game's own effect code, in the original and the build: the particle cook's `view` (0x25A86F) on the pack's composite, the first- and third-person presenters' constructors (their particle presenters' views 0 and 1), the particle presenter's handler 0x2AC400 (an activation plays an effect only in the presenters of its view), the whole chain from player::set_effect_presenter and game_effect_player's dispatch 0x2DE3A0 and the artefact's effect calculator 0x313640 (the effect ends when the artefact is no longer active) (needs Unicorn) |
| `rust/patches/src/blackout.rs`, `src/blackout.toml`, `check_blackout_026g.py` | Patch 65 (a prototype): Blackout, a battery match with no interface, the teammates' markers kept and their names on Tab: six `lohook`s at sites of its own (`sites_026g.tsv`); its check runs each from the game's code before the site on made-up objects, the original beside (needs Unicorn) |
| `check_suicide_key_026g.py` | Runs the original's and patch 34's last action call of the input builder, with the game's own event buffer code, on made-up input handlers (needs Unicorn) |
| `check_tab_list_026g.py` | Runs the original's and patch 39's refresh of the scoreboard's rows (update_ui 0x23B421..0x23B477), with the game's own list code of initialize_match made again, on made-up huds whose places enter and leave (needs Unicorn) |
| `check_tab_self_026g.py` | Runs the original's and patch 43's end of game_world::on_activate, where the hud gets its player's name (the Tab list's own row gold), on a made-up world with the journal off, recording and played back (needs Unicorn) |
| `check_weapon_show_026g.py` | Runs the original's and patch 91's weapon::on_show and the next weapon::draw's scope (add and remove of the aimed sight, the game's own) on made-up weapons, aimed and not, first person and not, no optic, a red dot and a full-screen scope, with the renderer's in-scene byte kept (needs Unicorn) |
| `rust/patches/src/scaleform_capture.rs`, `src/scaleform_capture.toml`, `src/vostok/drawable_image.rs`, `check_scaleform_capture_026g.py` | Patch 93: the render thread's walk of a Scaleform DrawableImageContext's command queues (OnNextCapture 0x572660) never reaches a queue being destroyed (the rev-40 crash at a match's end): two lohooks; its check runs OnNextCapture and the queue's destructor on made-up contexts, the original beside (needs Unicorn) |
| `rust/patches/src/use_scan.rs`, `src/use_scan/rule.rs`, `src/use_scan.toml`, `src/vostok/use_scan.rs`, `check_use_scan_026g.py` | Patch 23 and the use scan it lives in: `base_player::detect_usable_object` (0x2D84C0) whole in Rust, a lohook at its start leaving by its `ret 8` (section 23); the rule (`rule.rs`) is pure, the game's calls `vostok`'s; patch 85 asks it whether Use is the game's |
| `rust/patches/src/subtick_hits.rs`, `src/subtick_hits.toml`, `src/vostok/ray_test.rs`, `check_subtick_hits_026g.py` | Patch 103: a bullet meets a player where he stood at the ms it crossed him, not where the 50 ms step left him (`../docs/notes/subtick_bullet_hits.md`); one lohook first in `bullet.ray_traced`'s chain; its check runs the game's own ray test (ray_test, rayTest, rayTestSingle) in both exes on made-up bodies (needs Unicorn) |
| `rust/patches/src/input_check.rs`, `src/input_check.toml`, `src/vostok/input.rs`, `check_input_check_026g.py` | Patch 99: a world takes no player input whose rotation no client makes (a NaN, an infinity, 1024 radians or more), as 0.27d's change_input refuses one: a lohook at change_input<player_input>'s start (0x1FAB50); its check runs that start on made-up inputs, the original beside (needs Unicorn) |
| `check_mouse_buttons_026g.py` | Patch 102 (`MOUSE_BUTTONS` in `patch_026g.py`): a side mouse button (mouse4..mouse8) is no left click in the lobby, login, options and match menus; its check runs the five handlers in both exes (needs Unicorn) |
| `rust/patches/src/mouse_pitch.rs`, `src/mouse_pitch.toml`, `src/vostok/mouse.rs`, `check_mouse_pitch_026g.py` | Patch 112: the mouse look's pitch with 16:9's h/w at every window while "Lock vertical sensitivity" is ticked, 1:1 at the vertical slider's value when unticked (user.cfg's `srp_mouse_vertical_locked`, `srp_mouse_vertical`); a lohook at on_mouse_move's pitch step, the two variables, the Settings' calls and the death camera's entry; its check runs on_mouse_move in both exes at 4:3 to 32:9, off and on, and the Settings' calls (needs Unicorn) |
| `rust/patches/src/footsteps.rs`, `src/footsteps.toml`, `src/vostok/footstep.rs`, `check_footsteps_026g.py` | Patch 101: a footstep that only a re-simulation makes (another player's late input) is heard instead of dropped; one lohook in on_player_sound_event's time test; its check runs the handler in both exes (needs Unicorn) |
| `rust/patches/src/keyboard_layout.rs`, `src/keyboard_layout.toml`, `src/vostok/keyboard_layout.rs`, `check_keyboard_layout_026g.py` | Patch 107: chat text follows the keyboard layout the window was last told of (under Wine the engine's logic thread never got it); three lohooks, in message_processor's two layout messages and after translate_text's GetKeyboardLayout(0); its check runs both functions in both exes (needs Unicorn) |
| `rust/patches/src/key_remap.rs`, `src/key_remap.toml`, `src/vostok/keyboard.rs` (`KeyEvents`, `KeyMoves`), `src/vostok/system.rs`, `check_key_remap_026g.py` | Patch 110: the keys a desktop moves (xkb `caps:swapescape`, `ctrl:nocaps`) are moved in the game under Wine's Wayland driver, whose DirectInput codes are the physical keys'; one lohook in keyboard::execute after GetDeviceData moves each event's function key to the one the told layout names it; its check runs execute whole in both exes (needs Unicorn) |
| `rust/patches/src/hud_cursor.rs`, `src/hud_cursor.toml`, `src/vostok/hud_input.rs` (`MenuChange`, `CursorMovie`), `check_hud_cursor_026g.py` | Patch 113: the hud's kits and cursor start over at each match (the last match's pick stayed on the kits' tab), and the main menu covers the hud's cursor (Esc over the kits left it drawn under the menu's); five lohooks, at initialize_match's start, show_cursor's show and hide and game_options' activate and deactivate; its check runs each from the game's code on a made-up game, the original beside (needs Unicorn) |
| `rust/patches/src/ban_time.rs`, `src/ban_time.toml`, `src/vostok/login.rs` (`BanStatusText`), `src/vostok/clock.rs` (`FileTime`), `../crates/ban-reason` | Patch 116: a banned account's sign-in shows srp's reason, when the ban ends and how long it has left, from what srp's login server sends after a ban's code; two lohooks, at the sign-in answer and at set_status's built text |
| `rust/patches/src/artefact_skills.rs`, `src/artefact_skills.toml`, `src/vostok/artefact_skills.rs`, `check_artefact_skills_026g.py`, `src/artefact_keeper.rs`, `src/artefact_keeper.toml`, `src/vostok/keeper.rs`, `check_artefact_keeper_026g.py` | Patch 109: the artefact tree's skills the exe computes (`../docs/notes/artefact-skills.md`), their ranks packed in kit modifier 40; Resistance (+0.05 armour a rank against the anomalous hit types, 0x32A083), Onyx II (the landing 0x2D8220), Larkspur II (the activation 0x313333), Spring II (patch 54's reload nodes, its entry), Resonance and Cascade (the aura at the getters 0x2D5AE6 / 0x2D5C18, the protectors 0x32A020, the regeneration 0x32A53F), Keeper of the Artifacts (a Keeper gets a dormant artefact of the match's kind from the map's hidden pool at the match's start, into its first empty container, once a match: register_artefacts 0x31B8E0, the player's tick 0x2DAAD3); their checks run the game's code for each (needs Unicorn) |
| `rust/patches/src/forest_glow.rs`, `src/forest_glow.toml`, `src/vostok/effect.rs`, `check_forest_glow_026g.py` | Patch 111: Forest Protection's grey glow on the arms while it protects a player (inside a damage zone, a rank of modifier 41 in its kit), seen by everyone; and the leg quarters, each passive artefact copy counting for a player (own and Resonance's shared, at most 4 a kind) lighting a quarter of its legs in its kind's colour, shown only by a game whose local player's current kit has Artifact Sight (513, modifier 40 bit 12) or that has no local player (pack artefact-glow@13); a place's third-person presenter given back to its current kit's object (the stock game left it on another kit's after a respawn, so other games showed none of that player's effects); played from the player's simulation tick with its own calculator, which ends each when the game no longer shows it; its check runs the tick, the effects' query and callback, the clock and the cook in the built exe (needs Unicorn) |
| `rust/patches/src/controller_position.rs`, `src/controller_position.toml`, `src/vostok/controller.rs`, `check_controller_position_026g.py` | Patch 118: a character controller's step never leaves its position non-finite: at player_step's epilogue (0x7C7D7) a position with an infinity or a NaN goes back to m_last_pos, where the step began; one lohook; a simulation change (`../docs/notes/client_patch.md` section 3.71) |
| `rust/patches/src/unplayed_match.rs`, `src/unplayed_match.toml`, `src/vostok/match_close.rs`, `check_unplayed_match_026g.py` | Patch 120: a match its game never played (the time rule's state 0: the handshake, a level loading, a level loaded before its first world) closes too, kicked, timed out, cut off or given its farewell: five lohooks (connect_to_match's connect, the logoff, close_current_match's guard, on_project_loaded's abort test, on_match_ready's end); its check runs each from the game's code on a made-up network client, the original beside (needs Unicorn; `../docs/notes/client_patch.md` section 3.74) |
| `check_kick_lobby_026g.py` | Patch 119 (`KICK_TO_LOBBY` in `patch_026g.py`): a game the match server kicks (0xFD) goes back to the lobby as one whose link was lost does: close_current_match's jump table sends reason 4 to reason 1's `31` and world end; its check runs on_match_disconnected in both exes for every reason and state (needs Unicorn; `../docs/notes/client_patch.md` section 3.72) |
| `rust/patches/src/memory_report.rs`, `src/memory_report.toml`, `src/vostok/memory.rs` | Patch 126: the game tells the match server how much memory it has left: each registered arena's size, use and largest free block, the address space and the video budget, when a match connection's first world is in and when the match closes on it; to `srp_patches.log` too, and in the generic client-report carrier (`srp_validate::report`); two lohooks on sites patch 97 has (`../docs/notes/memory-report.md`, `../docs/notes/client_patch.md` section 3.76) |
| `check_memory_defaults_026g.py` | Patch 37 (`MEMORY_DEFAULTS`), patch 124 (`ARENA_DEFAULTS`), patch 128 (`THREAD_STACKS`) and patch 129 (`MT_ARENA` in `patch_026g.py`): the resource floor (768 MiB), the resources helper (32 MiB) and render (320 MiB) arenas' defaults, a command-line value still taking over, an engine thread's stack (2 MiB), the multithreaded arena's floor (1024 MiB, with or without its key) and allocate_arenas' stack term; its check runs the native option reads, the registrations' sizes, threading::spawn to CreateThread and the stack term's sum in both exes (needs Unicorn; `../docs/notes/client_patch.md` sections 3.75, 3.77 and 3.78) |
| `rust/patches/src/memory_profile.rs`, `src/memory_profile.toml`, `src/vostok/memory_profile.rs`, `memory_profile_026g.asm`, `check_memory_profile_026g.py`, `../scripts/memory-profile/analyze.py` | Patch 127: a memory profiler of the engine's arenas for the owner and for workers' local tests, never players: on with `SKHID_MEMORY_PROFILE=on` in the game's environment. Off, the exe keeps the game's allocator bytes: its five gates (dlmalloc's three entries, ptmalloc3's two frees of mapped blocks) and the out-of-memory hook are installed on demand (`install = "on_demand"`, written at the start only when on). On, each block is counted to its arena and the call site that asked; snapshots (each arena's use, holes and sites; the address space) go to `srp_memory_profile.txt` in the user data folder at a world in, a match closing, a full arena and Scroll Lock; `analyze.py` prints the report; its check runs dlmalloc on a made-up arena in both exes and reads the snapshot (needs Unicorn; `../docs/notes/memory-profile.md`) |
| `rust/patches/src/constant_upload.toml`, `constant_upload_026g.asm`, `check_constant_upload_026g.py` | Patch 134: shader constants cost the render thread less, the same bytes on the GPU: shader_constant_buffer::set_memory (0x11B5A0) copies 16 bytes at a time, not one, and update (0x3DDFD0) maps a buffer only when its bytes differ from the ones it last uploaded (four `code` hooks); its check runs set_memory on 4000 made-up cases in both exes (every register, XMM0..7 and the memory alike), the constructor, and 150 draws over four buffers with a model of the device context (the GPU's bytes after every update alike) (needs Unicorn) |
| `audio_device_026g.c`, `audio_device_026g.h`, `audio_device_engine_026g.c`, `test_audio_device_026g.c` | Patch 100: the sound follows the audio output device (XAudio2's engine and voices proxied, made again on the default device after a lost device or a new default); the test runs the core on i686 under the sanitizers against a made-up XAudio2 that keeps its rules (no Unicorn) |
| `rust/patches/src/sound_voices.rs`, `src/sound_voices.toml`, `src/vostok/voices.rs`, `check_sound_voices_026g.py` | Patch 96: shots no longer go silent under heavy fire (`../docs/notes/sound_voices.md`): the game world's sound scene has 1024 sound instances and propagators (`SOUND_POOLS`, game_world::load 0x1F8DB6/0x1F8DBD), and a sound refused a voice takes the voice of the quietest one playing when that one is less than half as loud (a lohook after create_sound_voice, 0x5CAB0), and the source voice walk is bounded to one pass (`sound_voices_026g.asm`, 0x5E5E7, 0x631D9, 0x59F8E); the check runs the voice loop from that site on made-up scenes with a model of the world's voice pool, and the self-test's `test_sound_pools` (needs Unicorn) |
| `check_stage_focus_026g.py` | Runs the original's and patch 52's Stage focus getter (Scaleform's `stage.focus`, 0x771FA0) on a made-up movie: focused objects of each kind, none, one without its ActionScript object (the lobby's crash of 2026-10-22) (needs Unicorn) |
| `rust/patches/src/language.rs`, `src/language.toml`, `src/vostok/language.rs`, `languages_026g.rs`, `check_language_026g.py` | Patch 79: the game language is a setting kept in user.cfg (Settings, Gameplay, Language; `../docs/notes/language-option.md`): three lohooks (`sites_026g.tsv`); `languages_026g.rs` is generated from `localization_026g.py`'s registry; its check runs each hook from the game's code on made-up objects, the original beside (needs Unicorn); the movie's harness is `../scripts/language-option/check.sh` |
| `rust/patches/src/tab_cursor.rs`, `src/tab_cursor.toml`, `src/vostok/hud_input.rs`, `src/vostok/keyboard.rs`, `src/vostok/lobby.rs`, `check_tab_cursor_026g.py` | Patch 86: Ctrl on the Tab list shows a cursor, and its rows' menu sends the lobby's friend request (`../docs/notes/tab-cursor-friends.md`): two lohooks (`sites_026g.tsv`); its check runs both from the game's code on made-up objects, the original beside (needs Unicorn); the menu's harness is `../scripts/tab-row-menu/check.sh` |
| `rust/patches/src/music.rs`, `src/music.toml`, `src/vostok/sound.rs`, `src/vostok/resources.rs`, `check_music_026g.py` | Patch 88: the music (`../docs/notes/music.md`): the menus' and the maps' themes in a sound scene of its own, kept active and faded by the sound thread; its query callback the framework's first `callback` hook (handed to query_resources at run time, no site rewritten). The check runs its seven lohooks and the callback through the game's own boost invoker, bind, config lookups and add_scene_to_active (needs Unicorn) |
| `rust/patches/src/hud_options.rs`, `src/hud_options.toml`, `src/vostok/settings.rs`, `src/vostok/renderer.rs`, `check_hud_options_026g.py`, `../scripts/dxvk-loader/` | Patch 83: the Settings' HUD group ("Hide hit markers", "Hide player scores", the crosshair list's Hidden), settings kept in user.cfg (Settings, Gameplay; `../docs/notes/hide-hit-markers-scores.md`), the Gameplay tab's field of view (`uro_fov`, patch 22's variable: `root.srp_slider_option`), and the Graphics tab's Renderer (`srp_dxvk`): at the process's start (`memory.allocate_arenas`) user.cfg read, the Vulkan look and DXVK loaded from `dxvk\` by full path (the game's delayed dxgi, d3d11 and d3d9 bound to them by their module slots), and with it `DXVK_CONFIG` set from the Vulkan options (`srp_dxvk_frame_limit`, `_low_latency`, `_tear_free`, `_dedicated_gpu`, `_overlay`), a failed DXVK start turned off at the login screen (`../docs/notes/dxvk-option.md`; the loader's proof under Wine `../scripts/dxvk-loader/run.sh`), and with DXVK, right before each Present of the game's swap chain (`render_output.present`), a window DXGI refuses (topmost, no frame style: DXVK's fullscreen, set_size's style 0) made a popup, so AMD's driver can make its swap chain (`dxvk-option.md`, "The fullscreen window"): lohooks, those at a site patch 79 or 65 hooks too after that patch's; `vostok::settings` is the Settings' call, the save of user.cfg and a saved bool, shared with patch 79; its check runs each hook from the game's code on made-up objects, the original beside (needs Unicorn); the movies' harness is `../scripts/hud-options/check.sh` |
| `rust/patches/src/tab_cursor.rs`, `src/tab_cursor.toml`, `src/vostok/hud_input.rs`, `src/vostok/keyboard.rs`, `src/vostok/lobby.rs`, `check_tab_cursor_026g.py` | Patch 86: Ctrl on the Tab list shows a cursor, and its rows' menu sends the lobby's friend request and chat mute (`../docs/notes/tab-cursor-friends.md`, `chat-mute.md`): two lohooks (`sites_026g.tsv`); its check runs both from the game's code on made-up objects, the original beside (needs Unicorn); the menu's harness is `../scripts/tab-row-menu/check.sh` |
| `world_sections_026g.asm`, `check_world_sections_026g.py` | Patch 98: hash()'s world buffer and the upload's are 0xF000 bytes (two immediates), and the history item's six world sections grow on the heap (one jump at the end of its constructor, the arena's block `world_sections.code`); its check runs the constructor, the sections' writers, hash, deserialize and destroy of the game's own code on made-up items, the original beside (needs Unicorn) |
| `rust/patches/src/world_check.rs`, `src/world_check.toml`, `src/vostok/world_load.rs`, `../crates/world-frame`, `check_world_check_026g.py` | Patch 87: a game parses every world it is sent (0xE8) before it loads it, with the match server's own parse (crate `world-frame`: the frame to its end, no slot past 20, no state over the game's buffer, no body over 0xF000 bytes); a refused one is never loaded and the game leaves the match (`../docs/notes/world_frame_check.md`): two lohooks (`sites_026g.tsv`); its check runs both loads beside the original and, with `--drift`, kept worlds and mutated copies through the arena's build against the host's (needs Unicorn) |
| `check_world_roundtrip_026g.py`, `worlds_026g/` | A resync always converges: a late joiner and a resynced game load each recorded world (`worlds_026g/`, 8 dumps of rev41's match 0x1c0ebd31; or any `SRP_SNAPSHOT_DIR` dumps) and save it at once, through every patch hook inside the world's load and save (the 0xE8 preparation, patch 85's record, patch 78's carry); each must write back the bytes it read. Every patch site in a `::serialize` or `::deserialize` function is run there or named with its own check (`../docs/notes/client_patch.md`, "World round trip"; needs Unicorn) |
| `rust/patches/src/animation_id_log.rs`, `src/animation_id_log.toml`, `src/vostok/animation_id.rs`, `check_animation_id_log_026g.py` | Patch 95: a diagnostic (`../docs/notes/animation-ids.md`): an animation pair the registry has no id for (the lookup 0x2D6590 returns 0FFFFh, the rev47 crashes at 0x1F18C8) is logged to srp_patches.log once, with the clips' names, the registry's size and who asked; one lohook (`sites_026g.tsv` `animations.id_missing`); its check runs the lookup in the original and the patched exe (needs Unicorn) |
| `rust/patches/src/drug_interrupt.rs`, `src/drug_interrupt.toml`, `src/vostok/drug.rs`, `check_drug_interrupt_026g.py` | Patch 84: a bullet that hits a player stops the healing drug he is using (`../docs/notes/drug-interrupt.md`): one lohook (`sites_026g.tsv` `player.hit_applied`), the game's own `medkit::set_active(0)` through the engine library; its check runs the hook in the patched exe and the game's set_active(0) in the original (needs Unicorn) |
| `rust/patches/src/hand_kill.rs`, `src/hand_kill.toml`, `src/vostok/melee.rs`, `check_hand_kill_026g.py` | Patch 108: a gun-stock hit kills, wherever it lands (`../docs/notes/hand-kill.md`): one lohook on patch 94's `medical.hit` site (ours first in the chain); its check runs the chain in the patched exe (needs Unicorn) |
| `rust/patches/src/text_input.rs`, `check_text_input_026g.py` | Patch 52: ignore a character event whose Scaleform text field has lost its AS3 receiver; reproduces the PIN-entry crash and checks native register/stack behavior (needs Unicorn) |
| `weapon_camo_026g.asm`, `weapon_camo_026g.section.bin` | The source of patch 44's code (a weapon wears the camouflage its item holds) and that code assembled for `.srpw`, its own section on the page after `.srpc` (0xA42A000), which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `research_026g.asm`, `research_arena/`, `../crates/research-rule/` | Patch 53 (Research): four aligned assembly entries, the integer rules in Rust, and the cook, rule slots and UI integration in Rust |
| `link_026g.py`, `arena_026g.lock.json` | The arena's linker (below, "The arena"): every block's C and NASM sources compiled to ELF32 objects and linked by GNU ld at `.srpx`'s VA, at every build; `--write` writes the lock (the linked arena's SHA-256, its blocks and symbols), `--check` compares. Needs gcc (`-m32`), GNU ld and NASM 2.16 (`nix develop ./srp-026g`) |
| `engine_026g.tsv`, `engine_026g.py` | The engine-function library (below, "Calling the game's functions"): its functions, one row each, and the generator of the three files after it from those rows and the re-kit's ABI rows (`--write`, `--check`) |
| `engine_026g.h`, `engine_026g.inc`, `engine_026g.rs`, `raw_026g.rs`, `engine_026g.asm` | Generated, not edited: the C declarations, their externs for NASM, their declarations for Rust (C's types, research_arena's; typed by the kinds' handles, the Rust patches' `raw`), and the thunks, the arena's block `engine.code` |
| `check_engine_026g.py` | The library's ABI check: every thunk against the rows and the original exe, and wrong rows it must reject (needs Unicorn; the self-test runs it) |
| `match_keys_026g.asm`, `check_match_keys_026g.py` | Patch 58: the keys 1, 2 and 3 pick a kit while the kits are shown (none for a key pressed before they came or in their first 300 ms), and the match chat option (the arena's blocks `match_keys.code` and `match_keys.state`); its check runs the four entries from the game's sites and the game's own input tick, the game's own cc_value<bool> constructor, command lookup and execute on made-up objects (needs Unicorn) |
| `hide_ui_026g.asm`, `check_hide_ui_026g.py` | Patch 62: one key (F11, the game's `toggle_replay_ui`) hides the whole interface and shows it again, in a match and in the lobby (the arena's blocks `hide_ui.code` and `hide_ui.state`); and the options' 41st controls row (`hide_ui.data`); its check runs the three entries from the game's sites with the game's own show_ui and get_binded_action, and the options' key list, a key set, the accept and the binder's bind and save, on made-up objects (needs Unicorn) |
| `kill_feed_026g.asm`, `check_kill_feed_026g.py` | Patch 67: the kill feed's wall, ricochet and pain flags (the arena's block `kill_feed.code`): the kill event's byte +5 from the bullet's pierces and the victim's pools, passed to the hud movie as `penetration_icon`, `ricochet_icon` and `pain_icon`; its check runs the event, the hud's handler and the row from the game's sites on made-up victims, bullets and descriptions, with the game's own part lookup (needs Unicorn) |
| `check_fall_gravity_026g.py` | Patch 72: free fall takes gravity for the step's dt (`FALL_GRAVITY`, two sites rewritten in place in step_down, 0x7E6E6 and 0x7E720): a tick the world steps in parts (each precise input) no longer gives a falling player a whole tick of speed at every part; its check runs step_down's free fall on a made-up controller: whole ticks bit for bit the original's, a tick in 2, 5 or 2 uneven parts one tick of gravity in all (needs Unicorn) |
| `rust/patches/src/artefact_carry.rs`, `check_artefact_carry_026g.py` | Patch 78: H (the game's unused action `artefact`, default H, a controls row) + an artefact quick key lowers the weapon and raises the artefact above the right palm in both views; switching lowers the old item before raising the new one. Use takes an offered or death-dropped artefact into a free container, with the existing collection UI. Deterministic input/snapshots, native inventory ownership, model callbacks and matching damage poses. See [controls, architecture and validation](../docs/notes/held_artefact_transfer.md). |
| `rust/patches/src/bake_cache.rs`, `src/bake_cache.toml`, `src/vostok/baker.rs`, `src/vostok/files.rs`, `src/vostok/crc32.rs`, `check_bake_cache_026g.py` | Patch 70: the baked camouflage and decal textures kept from game to game (the folder deleted only for another revision, its files back in the baker's list) and baked or loaded once while asked for again: seven `lohook`s (`sites_026g.tsv` bake.* and crc32.step); its check runs game::game's delete, the game's own check_loaded, translate_query's look-up, the texture made by a bake and a load, and finish_query on a made-up baker, folder and queries (needs Unicorn) |
| `rust/patches/src/bake_async.rs`, `src/bake_async.toml`, `src/vostok/bake_gpu.rs`, `check_bake_async_026g.py` | Patch 121: the camouflage/decal bake does not wait for the GPU: level 0 a GPU copy of the target, the compressed levels copied to stagings and flushed, at most 8 bakes on the GPU (with all 8 taken, the oldest completed on the spot for the next), polled each dispatch with the no-wait map, the DDS allocated only then, and process_compression's own tail run, on a 6 ms budget a frame; nothing is put off or asked again; the save owns the MT DDS (no copy), at most 64 saves and 128 MB not written yet; nine `lohook`s (`sites_026g.tsv` bake.issue, bake.resume, bake.save, bake.free, save.freed, compress_mip.readback, resources.dispatched, engine.tick, game.exit_start); its check runs them in Unicorn on a made-up D3D11 context |
| `check_research_026g.py`, `../crates/research-rule/src/tests.rs` | Checks of patch 53: the first runs the built exe's hooks, the cook (with the game's own register_cook, rule constructor and config look-ups) and the rule's slots on made-up games, worlds and huds (needs Unicorn); the second the rules as Rust unit tests |
| `check_weapon_camo_026g.py` | Runs the original's and patch 44's 0x24B0D2 of weapon_cook::on_weapon_subresources_ready on made-up weapon configs and models, with the game's parameters constructor, string assign, release and boost invoker, and each bake's callback (needs Unicorn) |
| `check_kit_change_026g.py` | Runs the original and patch 31's respawn rule tick, with the places' tick and the game's own spawn, the view of a kit change, the hud's update of the local player, mouse moves and the mouse's buttons through the game world's input handler and the warmup mode's step on made-up objects (needs Unicorn) |
| `check_death_camera_026g.py` | Runs the original's and patch 46's mouse moves through the game world's input handler (the death screen, a live player, spectators, a death before the start) and update_camera's call of the view's update on a made-up eye transform (needs Unicorn) |
| `check_baked_texture_026g.py` | Runs the original's and patch 35's baked-texture list of the camouflage/decal baker (the look-up, the push and check_loaded_baked_texture) on made-up loads, among them those of the crash of 2026-10-17 (needs Unicorn) |
| `weapon_preview_026g.asm`, `weapon_preview_026g.section.bin` | The source of patch 32's code (a weapon's upgrade window shows the weapon) and that code assembled for 0x2E50 of `.srp`, its state at 0xE00 of `.srpd`, `-DFIND` the stash code's address and `-DCAMERA` patch 45's camera (weapon_table_026g.asm), which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `weapon_stash_026g.asm`, `weapon_stash_026g.section.bin`, `check_weapon_stash_026g.py` | Patch 32's find_kit grown: a weapon no kit holds is shown in the chosen kit's hands from a scratch copy of the account's profile; assembled for 0x360 of `.srpw`; its check runs it with the game's own profile and record copies (needs Unicorn) |
| `weapon_table_026g.asm`, `weapon_table_026g.section.bin`, `check_weapon_table_026g.py` | Patch 45: a weapon's upgrade window shows the weapon in the air above the lobby's workbench (the spot and the view from the lobby project's `srp_lobby`), turning about its middle, the camera flown there once it is, and a drag (the left button too, off the movie) turning it; assembled for 0x6B0 of `.srpw` (0x800 until round 5d), its state at 0xF00 of `.srpd`; its check runs the lobby's handlers, the camera's constructor and tick, its drag buttons and mt_draw's two sites with the game's own code (needs Unicorn) |
| `weapon_decals_026g.asm`, `weapon_decals_026g.section.bin`, `check_weapon_decals_026g.py` | Patch 47: a weapon wears a decal (the bake patch 44 asks for paints it; the code takes two, the server gives one slot) and the lobby's decal editor places them on the weapon above the workbench; assembled for 0x1200 of `.srpw` (after patch 45's code; 0x1000 until patch 45's round 6), its state at 0xFC0 of `.srpd`; its check runs the bake with the game's own unpacking and the editor's four sites with the game's own encode (needs Unicorn) |
| `upgrade_preview_026g.asm`, `upgrade_preview_026g.section.bin`, `check_upgrade_preview_026g.py` | Patch 48: an upgrade dropped on a weapon in its upgrade window is shown at once (the character made again from patch 32's scratch with the change, before the request leaves), the server's account staying the truth; on_player_ready logs the time the lobby character's player took; assembled for 0x2000 of `.srpw` (its third page), its state at 0xD60 of `.srpd`; its check runs the six sites with the game's own setup_profile, profile copy and config lookups (needs Unicorn) |
| `weapon_draw_026g.asm`, `weapon_draw_026g.section.bin` | The source of patch 32's draw (a weapon's window draws the player as the lobby does) and that code assembled for 0xD20 of `.srp`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `check_weapon_preview_026g.py` | Runs the original's and patch 32's upgrade window start and close (with the lobby camera's own setter), `setup_profile`'s entry, the lobby camera's three distance clamps and the player's decal pose on a made-up lobby, loaded as is and rebased (needs Unicorn) |
| `check_forward_scroll_026g.py` | Runs the original's and the build's forward-effect compile, with patch 28's two hooks, on made-up materials and compares what each sends; runs the third hook against the game's own binding of `scene_time`; loaded as is and rebased (needs Unicorn). |
| `selftest_026g.py` | Builds every variant and checks the general rules on each from what the build records and the patches declare (no byte changed undeclared, no relocation or branch into a rewritten byte, every hooked function's ABI, every branch target known, the PE, the .bin files current), and each patch's own code where it guards its ABI; needs the original exe, nasm (2.16), gcc (`-m32`), GNU ld and Python unicorn. `release/release.sh` runs it before a release (step 2, its Linux part): not during development |
| `run_battery_rule.ps1` | Runs the battery rule's handler (patch 9) out of any 0.26g exe on a made-up rule object, in the 32-bit Windows PowerShell: what an exe does with a put that is played again after a rollback. Starts nothing of the game |
| `survarium_login_server.crt` | The login certificate (the same file as `../certs/survarium_login_server.crt`) |
| `launcher/` | The game's launcher made to work again, and to keep these files current: its patch tool, the one piece of new code, the self-test and its own README. Its updater is `../../client/updater` |

## The arena (`.srpx`) and call shapes

Two things keep a new exe patch from the mistakes PR #297 shipped
(`docs/notes/client_patch.md` sections 0.3 and 0.4).

**`.srpx`, the arena.** The exe's section table holds twelve entries (it is
at file 0x210 and the headers end at 0x400); the game's seven plus `.srp`,
`.srpd`, `.srpc` and `.srpw` take eleven. The twelfth is `.srpx`, a growing
arena, so a later patch need not add a section of its own. **A patch is
plain C or NASM, and the tooling links it** (`link_026g.py`): it declares
its blocks in `patch_026g.py`'s `ARENA`, an `ArenaBlock(name, kind, align,
sources)` each (`kind` "code", "data" or "state"; `sources` its `.c` and
`.asm` files); every source becomes an ELF32 object (`nasm -f elf32`, `gcc
-m32 -c` with Research's flags) and GNU ld links them all at `.srpx`'s VA,
one output section per block in `ARENA`'s order (a code block takes its
sources' `.text` and `.rodata`, a data block `.data`, a state block `.bss`).
Sizes and VAs are what the link gives: nothing is reserved or pinned, and
two PRs adding blocks do not collide. Calls between patches are by symbol
(the engine library's `engine_...`, below); the game's addresses stay
absolute (the exe is never rebased); a hook site takes its target from the
link by symbol (`RESEARCH_HOOKS`, `ARTEFACTS_HOOKS`: site, bytes, symbol,
opcode). The link refuses, loudly, a missing tool, NASM other than 2.16
(NASM 3 assembles `jmp near` to a label as a short jump), a section no
block takes, a relocation other than R_386_32/R_386_PC32, an undefined or
doubly defined symbol and an arena past `ARENA_LIMIT`.

No prebuilt arena code is checked in. `arena_026g.lock.json` records the
linked arena's SHA-256, its blocks and its symbols; every build (the
self-test's, the release's: `release/build-zip` builds in the dev shell)
links from source and stops if the bytes are not the lock's. After changing
an arena source: `python link_026g.py --write` and commit the lock with it.

**A Rust crate as a source** (the framework: `../docs/notes/rust_patches.md`):
a block may name a crate's `Cargo.toml` (its name, its `[lib]` path and the
crates it names by `path`; nothing else of it is read). The link builds it
with the dev shell's nightly **rustc alone, offline** (no cargo:
`-Zbuild-std` resolves the standard library's whole workspace and needs
crates.io): `core` from the toolchain's rust-src and an empty
`compiler_builtins`, once a toolchain, into `srp-026g/target-arena/
sysroot-<key>`; then the path crates as rlibs and the crate as a staticlib,
all for `i686-unknown-linux-gnu` with `-C relocation-model=static -C
target-cpu=pentium4 -C opt-level=s -C panic=abort -C codegen-units=1`, fat
LTO, no debug information, paths remapped. It takes the crate's one object
out of the archive (a symbol it would need from `compiler_builtins` is an
undefined symbol, which the link refuses) and checks it as any other
object. Its build's files (the crate, the crates it names by path, what it
`include!`s) are the source's digest. The rustc version is in the lock's
`built_with`.

**Adding a block:** one `ARENA` row and its source file; `global` the labels
a hook jumps to, `extern` what it uses of other patches; its hooks as
(site, bytes, symbol, opcode) rows made in `build()`; each declared in
`INSTALLS` (below); `python link_026g.py --write`. A Rust patch:
`../docs/notes/rust_patches.md`. The arena's blocks today (`python
link_026g.py --sizes` prints their bytes, pages and languages):

| Block | Patch | Language |
|---|---|---|
| `research.code`, `research.state` | 53, Research (its state `rn_state`) | NASM, C |
| `artefacts.code` | 54, the artefacts | NASM |
| `engine.code` | the engine-function library | NASM (generated) |
| `match_keys.code`, `match_keys.state` | 58, the kit keys and the match chat option | NASM |
| `hide_ui.code`, `hide_ui.data` | 62, the key that hides the interface | NASM |
| `kill_feed.code` | 67, the kill feed's flags | NASM |
| `patches.code` | the Rust exe patches, one crate (the framework's example is 34, the commit_suicide key) | Rust, generated hook entries |

The older fixed sections stay as they are (client_patch.md 0.3 says what
moving one would take).

**Call shapes.** Every function a patch installs — a vtable slot, a delegate,
a callback, a hook the game enters — declares in `patch_026g.py` the game's
own function it stands in for (its model) or an explicit ABI (`INSTALLS`,
`CALLBACKS`, `INSTALLED_VTABLES`). `build()` refuses an install it does not
name (a call, jump or pointer into our code at any offset of any write), and
a `*_vtable` of the arena's C code without a model (`audit_vtables`). The
self-test (`test_call_shapes`) reads what ours pops from the built bytes
(every `ret` reachable) and the model's from the original exe,
`tools/re-kit/v026g/abi.tsv` having to agree where it has the row, and fails
on a mismatch for: every SLOT, ENTRY (the model is the function at the site),
CALLEE (the replaced call's target, less the pushes replaced with it) and ABI
site (its declared pops); every CALLBACK; every slot of ours of every
INSTALLED_VTABLES vtable (Research's rule, cook and link resolver: check 53's
comparison, now the rule for every patch). It also flags an INSTALLS row no
built variant installs. A negative test rejects a rule vtable slot 16 that
pops 4 where tdm's pops 8 (Research's first `rn_deserialize`).
**Driven through the game's caller.** A `ret` comparison cannot see the
order of the arguments (Research's slot 7 took `(type, time, data)` where the
game passes `(type, data, time)`: #297's review). So each slot of ours of each
INSTALLED_VTABLES vtable, and each CALLBACK, names a case of its patch's
Unicorn check that reaches it through the game's own caller (the loop or
dispatcher that calls it in the game, with what the game passes there): the
self-test fails a slot or callback with none, or a case its script does not
have, and a negative test shows both refused. **Not checked here:** INLINE sites (a call of ours over other instructions) and
JUMP sites (a jump into ours from the game's code), which are declared but
left to each patch's own Unicorn check.

### Calling the game's functions: the engine-function library

The game's compiler (LTO) gave many of its functions arguments in any
register, some in registers and some on the stack, and either side cleaning
up. A patch does not call them by hand: it calls the library's function,
an ordinary cdecl one, and the library's thunk puts each argument where the
game's function takes it (`docs/notes/client_patch.md` 0.5).

- **From C:** `#include "engine_026g.h"` and call it:
  `const struct engine_cfg *mode = engine_cfg_child(root, "mode");`. The
  link gives each call its address; no inline asm. (Research's C does
  this for the eleven game functions it calls; GetCurrentThreadId, a
  Windows import, it calls through its import slot as before.)
- **From NASM** (a block of the arena): `%include "engine_026g.inc"` (an
  `extern` each), push the arguments right to
  left, `call engine_...`, `add esp, 4 x their count`. EBX, ESI, EDI and EBP
  are kept; EAX, ECX and EDX are not. The result is in EAX (a byte result
  widened), EDX:EAX for 64 bits, ST0 for a float.
- **A function the library lacks:** add a row to `engine_026g.tsv` with
  its C name, VA, where each
  parameter goes with a word of the re-kit's role there (`EAX:table
  EDI:key`, `ESI:allocator EAX:size ESP+4:description`; `-` for none) and
  its C prototype. Its re-kit rows (`tools/re-kit/v026g/abi_inputs.tsv`, `abi.tsv`)
  must have every argument and the cleanup (`ret_cleanup`) and, if it
  returns something, the result; append the rows that are missing (from the
  function's body and its callers; the re-kit's README). Then
  `python engine_026g.py --write`, `python link_026g.py --write` and the
  self-test. The generator refuses a
  row whose locations are not the re-kit's, a parameter whose word is not in
  its location's role (two parameters swapped: the words do not fit), a
  stack slot missing, a cleanup that is no number or larger than the slots, a
  result the rows do not give, and what
  it cannot pass (an XMM argument, varargs).
- **A virtual function** (the game calls it through its object's vtable,
  `mov eax,[ecx] / call [eax+slot]`, and the object's class varies): its row's
  `call` is `virtual` and its `va` is `<class>+<slot>`, the class whose
  virtual it is and the slot's byte offset
  (`survarium::pure_game_effect_emitter+0x20`). Its implementations are the
  re-kit's `vtables.tsv` rows of that slot whose class is the class or one
  derived from it (`base`): each vtable, the function its slot holds there.
  Each implementation must have its rows, all the same and thiscall (`this`
  in ECX, the stack slots popped), and the row's words are checked against
  each. The thunk takes `this` as its first argument and calls the slot of
  its vtable. A class found later is one more `vtables.tsv` row (and its
  function's ABI rows); a slot of a class with none is refused.
- **The check** (`check_engine_026g.py`, in the self-test): for every
  function, what the exe's `ret`s pop is abi.tsv's cleanup; the thunk, called
  with a marker per argument, reaches the real function once with each marker
  in its register or slot, and comes back with the stack balanced, EBX ESI
  EDI EBP kept and the result where C takes it; the config's look-ups, the
  field writers, the statistics' listener, the pool lookup and the
  allocation's name run for real on made-up data. **Caught:** a spec row
  with parameters swapped or out of order (the generator), a wrong cleanup
  in abi.tsv (the exe's `ret`s), a thunk that does not follow its rows, and
  a wrong re-kit location for the functions run for real; negative cases
  show each. **Not caught by the self-test:** a re-kit row whose location is
  itself wrong, role and all, for a function not run for real (the thunk and
  the check both follow it). Research's other functions (malloc, the rule
  base's constructor, the message, Invoke, register_cook) run for real in
  `check_research_026g.py`; a patch's own Unicorn check covers the
  functions it calls.

## How to apply

### The late join (patch 11, patch revision 5)

A player who presses Play goes into a match that is being played, if one
takes players. It is part of the default build since revision 5;
`patch_026g.py --no-late-join` and every `--cave=hash-mismatch` build are
without it and say revision 4.

What the game does differently, from its first connection to a match:

- it keeps twenty histories and twenty statistics records whatever the
  roster, and its input masks (0xED, 0xEE) have three bytes;
- it says so in its match connection: 0xD8's debug byte is `80` (`81` with
  `-debug_hash_mismatch`), where a game without the patch sends `00` or `FF`;
- each roster place has its own ranges of callback and weapon ids, so a
  place added later does not move the ids of the others; animation ids are
  appended in roster and kit order;
- on the server's 0xE4 during play it checks the newcomer's profile, loads
  its kits in the background, installs the player without making it enter,
  and answers (0xE0 with a marker no snapshot upload can have) "installed",
  "failed" or "busy". The profile stays allocated until its players are
  destroyed.

The server waits for every game's "installed", then gives the newcomer a
fresh world and the inputs since, checks that it computes the same, and
lets it enter at a tick all are told. The players already in the match keep
their clocks and histories. A join that fails frees the newcomer's place in
the lobby, keeps its roster place taken, and closes that match to later
joins.

**A game with this patch and one without never play one match**: the worlds
they keep differ. The server tells them apart by the patch revision (5 or
later) and has a queue for each (`docs/notes/matchmaking.md` section 9).

Build the code and check it (nasm, a C compiler for x86 and GNU ld; `CC` and
`LD` name others than `cc` and `ld`):

```sh
python late_join_026g.py --write-bin          # late_join_026g.section.bin and .json
python late_join_026g.py --check-bin          # are they what the sources give here?
python check_late_join_026g.py /path/to/survarium.exe --nasm /path/to/nasm
cc -std=c11 -g -O1 -Wall -Wextra -Werror -pthread -fsanitize=address,undefined \
  late_join_runtime_026g.c test_late_join_runtime_026g.c -o /tmp/srp-late-runtime-test
/tmp/srp-late-runtime-test
```

`patch_026g.py` and `selftest_026g.py` need none of that: they read the
checked-in `.bin`. The self-test (check 15) assembles the 4096 bytes of
assembly again and compares, and checks the build with the late join
against the build without it.

The machine-code checks need Unicorn 2.1.4. They cover native profile
decoding, the query and callback conventions, slot and weapon binding,
animation indexing, preparation, acknowledgement and cleanup, the fixed
capacities, where a slot's grenades and traps are among the saved objects
(patch 14) and the PE file. Resource scheduling, physics, animation and UI
set-up, transport and player destruction are intercepted there. The C tests
cover cancellation, retries, failed loads and concurrent destruction, and
count allocations and references (where AddressSanitizer loops at start,
run the test with `setarch -R`). Neither shows that games agree in a match.

Server tests cover preparation retries, packet loss, stale
acknowledgements, failed catch-up, reconnects, departures, living, dead and
respawned players in the snapshot, the match's end, Team Deathmatch
tickets, duplicate seats, squads entering together, the two queues, session
persistence and growth to twenty players, and a test through the whole
stack covers login, the lobby, catch-up, entry, input relay and results:

```sh
cargo test --locked -p session -p login-server -p lobby-server -p udp-transport -p survarium -p match-server --lib
cargo test --locked -p srp --test v026g_end_to_end
```

**What was run in the game, and what was not** (2026-10-13). The author of
the code ran the exe built by hand before it had a revision (it signed in
as revision `0x80`, with the code 0xE5C bytes lower in the section): joins
of two games into three and four on map 61, a battery mode, with the
starter kit and with other equipment, in step from 53.4 s to 133.3 s with
no resync. The revision 5 exe has the same code at another address; its
build passes the self-test, the machine-code checks and the C tests
(2026-10-14). **Run with the revision 5 exe (2026-10-13, the test server): a match of two games, in step. Not run with it: a join that completes** (the one tried was given up by the server because the games stood on one PC and only the one in front played: patch 12). Never run with either: a join into a Team
Deathmatch, a fight, a death and a respawn after a join, a change of kit,
a roster over four, a match that nobody joins played to its end.

`late_join_026g.py ORIGINAL OUTPUT --test-session 0x1000` builds an exe for
tests only, which passes the login by and connects straight to its
`-server=host:port` as that session. Use a session for each game with the
`late_join_native` example of the match server; for six players at the
start: `cargo run -p match-server --example late_join_native -- 2249 61 starter 6`.
Sessions start at `0x1000`; `join` on its input reserves each further
player.

### Released patch

```
python patch_026g.py <game>/binaries/x86/survarium.exe <game>/binaries/x86/survarium_skhid.exe
    (add --no-late-join for the revision 4 exe, for a server that does not ask for 5)
copy survarium_login_server.crt <game>/resources/ssl/survarium_login_server.crt
```

The source exe is never changed; the output must be another file. Then start
the game **from `game\binaries\x86`** (it finds the certificate by a path
relative to where it is started):

```
survarium_skhid.exe -localization=english -server=<host>:<login port> -minimum_resources_memory_size=768
```

Or let `make_client_026g.py` do all of it, without touching the game folder.
`bugtrap.dll` and `updater_skhid.exe` build reproducibly: the same commit,
toolchain and `SKHID_REVISION` (the release revision their version resource
says; `release/build-zip` sets it) give the same files in any folder, so a release builds them afresh
instead of copying the last download's (`../../client/check-reproducible.sh`
checks it; the downloads listed above were made with `--bugtrap=` naming a
kept DLL). First build the crash reporter with `../../client/bugtrap/build.sh`,
then:

```
python make_client_026g.py <game folder> <new folder> --server=<host>:<port> [--localization=ukrainian] [--check]
python make_client_026g.py <game folder> <new file.zip> --pack-zip --server=<host>:<port> [--localization=ukrainian] [--check]
```

It writes `game/binaries/x86/survarium_skhid.exe`,
`game/binaries/x86/bugtrap.dll` (the Rust crash reporter),
`game/binaries/x86/play_skhid.bat` and `play_skhid.sh` (Windows and Linux launchers),
`game/resources/ssl/survarium_login_server.crt` and
`README.txt` and `LINUX.txt` into the new folder, or a ZIP with `--pack-zip`, to be copied over
the game. With `--no-launcher` these are
the only seven files shipped; source, symbols, tests and development tools are
excluded.

Without that option it adds the launcher (`launcher/README.md`), made from the
game folder's own `survarium_launcher.exe`: `game/binaries/x86/launcher_skhid.exe`,
`updater_skhid.exe` (build it first with `../../client/updater/build.sh`, or pass
`--updater=<path>`), `7zr.exe` (7-Zip's standalone extractor, the copy of
`../../client/updater/extractor/`: the updater unpacks the original game with it when
a player has none; `--extractor=<path>` names another copy, which must be the same file),
`updater_skhid.url`, the ten pages of
`game/binaries/x86/launcher_skhid/`, and `Skhid.bat` and `Skhid.sh` at the top, beside `game/`
(`Skhid.sh` is the file of that name here: on Linux, `bash Skhid.sh` in the game's folder runs
`play_skhid.sh` from there).
`--site=https://<host>` names the server's web
page when it is not `https://<host of --server>`. A player then unpacks the ZIP into
the game's folder (the one that holds `game`) once and double-clicks `Skhid.bat`,
which starts `launcher_skhid.exe`: it looks for a newer ZIP on that site, installs it, brings the
game's patches to the server's `/patches` list, and enables its PLAY button only for a game the
server lets in (`launcher/README.md`, "How an update works"); PLAY starts the game in the
language chosen in it. The same input gives the
same ZIP byte for byte, which the updater relies on. Back up the original `bugtrap.dll` and login certificate before
installing. `--check` does every check and writes nothing. Existing ZIPs are
never overwritten. `--bugtrap=<path>` selects another built DLL; by default
the builder reads `client/bugtrap/target/i686-win7-windows-msvc/release/BugTrap.dll`.

Game content no longer goes into the ZIP: it ships as `resources.db.patch_<n>`
files that the updater fetches from the server's `/patches` list
(`../../client/updater/README.md`, "Patches"). The region options of
earlier releases (`--resource-region`, `--resource-grow`, `--resources-out`)
are gone; the updater only puts back the stock bytes of regions an earlier
release changed in place ("Restoring resources.db" there), fetching originals
from `/resources/<sha256>` when the player kept no copy.

Full overlays embed the supplied `--server` address in the executable, so
players do not need `-server=` at launch. The generated batch file retains
the language and allocator options. To use another server, add
`-server=host:port` to its start command. Login supplies the lobby address;
the lobby supplies the match address. Neither needs embedding.

For the standalone patcher, add `--server=host:port` at patch time to embed
the default. Without that option it retains the original address behavior.
An embedded hostname is limited to 63 ASCII bytes and the port to 1–65535.
`--cert-only` does not support embedding; cert-only overlays still pass the
address through their batch file.

On Linux, use **Wine 11.0+ with modern WoW64** (tested: **11.8 Staging WoW64**)
and Winetricks. Run `bash play_skhid.sh` from `game/binaries/x86`.
See [LINUX.txt](LINUX.txt) for setup and troubleshooting.

`bash play_skhid.sh launcher` starts `launcher_skhid.exe` in the same Wine
prefix instead of the game (not run under Wine by us yet).

The launcher is executable in folder output and has Unix executable permissions
in ZIP metadata. Launcher and packaging tests use synthetic fixtures and mocked
Wine commands, without game files or downloads:

```
python -m unittest discover -s . -p 'test_*.py'
bash -n play_skhid.sh
```

Other builds of the patch: `--cert-only` (only patch 1), `--cave=hash-mismatch`
(patch 4 in another place, see below), `--check`.

## Login menu account hint

The login movie's old registration button is replaced with plain text (released in
`resources.db.patch_12`; its source is `../flash/login_menu`, srp's class `SrpLoginHint`):

- English: "Account created on first sign-in"
- Ukrainian: "Акаунт створюється під час першого входу"
- Russian: "Аккаунт создаётся при первом входе"
- German: "Konto wird beim ersten Login erstellt"
- Polish: "Konto powstaje przy pierwszym logowaniu"

The hint wraps inside the login panel. The old button is hidden and disabled,
removed from keyboard focus, and its browser callback is erased. All five game
languages have a translation. The language comes from the native client's
localization data, so it follows `-localization`.

The released movie keeps its archive entry's size and CRC (zero padding and
four trailer bytes), so the signed file table stays intact. A change of it is
an edit of `../flash/login_menu/src`, built like any movie of the trees
(`tools/actionscript/flash_tree.py build`, [lobby-source.md](../docs/notes/lobby-source.md)).
The bytecode builder that made it (`login_hint_026g.py`) is in git history only.

## Field-of-view slider

Settings → Gameplay, a group of the right column under the HUD group: its heading "FOV:" in the headings' look (`st_fov_option`, with the colon
gamma has, as "Friends:" and "HUD:" are), the stock slider under it, 50 to 100 (`../flash/main_menu/src/SrpFieldOfView.as`; 2026-11-06, the owner: "we can move fov
to game maybe?", for the Graphics tab's Vulkan options' button). Exe patch 83 keeps it: `uro_fov` in the
Settings' call answers `root.srp_slider_option(name, label, degrees, least, most)` and sets whole degrees
within uro_fov's range, user.cfg at once (`rust/patches/src/hud_options.rs`, `vostok::settings::FieldOfView`).
The game's own video option 7 (a "FOV" slider) names "fov" again, no variable's, and the movie binds nothing to it:
one owner of uro_fov in the menu (a video option bound to it would write, at every Apply of the Graphics tab,
the value it read when the window opened). Until then the slider was the Graphics tab's, under gamma,
the game's option 7 bound to it (patch 22 named uro_fov there).

The slider waits for patch 83's answer: with an exe without it, it does not show. A change of it is an edit
of `../flash/main_menu/src` (`tools/actionscript/flash_tree.py build`). The Graphics tab's slider (from
revision 11, `fov_slider_026g.py`'s, in git history only) was checked live in the lobby and in a match;
the Gameplay one in Ruffle only (`../scripts/hud-options/check.sh`) and patch 83's side in Unicorn
(`check_hud_options_026g.py`).

## Silver only: no gold, no exchange

Gold is gone (the owner, 2026-11-08): silver is the one currency. The lobby
movie has no gold balance, no exchange window (`MessageMoneyExchange` and its
stepper were removed; the two symbols the graphics bind keep empty classes)
and no "Get Gold": every price is silver, a premium item's too
(`docs/notes/buy-gold.md`, `docs/notes/exchange.md`). The windows that offered
gold (the skills reset, the reroll, the upgrade, the repair, the rename, the
premium shop) offer silver only. The exe is not changed; the server answers
`47 04` as a request it does not know. The exchange's history (revisions 23 to
53: both ways at 100 silver a gold, Get Gold opening it) is in git.

## Buy for modified items

`InventoryList.showContextMenu` and `LobbyMenu.try_buy_item` (in
`../flash/lobby_menu/src`): right-click on an owned item with modifications lists Buy, which buys a stock one (the rules: `docs/notes/buy-modified.md`). The
menu (`InventoryList.showContextMenu`) listed Buy for an item that is not a
stack only when `!itemData.rank`; an owned item's rank is its number of
modifications (the server's `roll_modifications`), and a dictionary item's
is its rarity. The test becomes `!rank || itemAvailable`, the method's own
"owned and enabled" (`descriptor <= 100 && enabled`). Shop, bundle and bundle-content entries are never `itemAvailable`, so
they keep the old rule. `buyElement` and `try_buy_item` go by the dictionary
id and the shop's
price and seller, so nothing else changes; the server gives a new item
without modifications.

From revision 25 to 2026-11-01 a buy on an owned item was not asked "This
item is already purchased by you. Would you like to buy anyway?":
`LobbyMenu.try_buy_item`, which every buy goes through, asked it only for a
buy from the shop's list (`sourceSlot == 200`), not from an owned item's own
slot (100 or less). From 2026-11-01 it asked for every buy but a bundle's
(`sourceSlot != 300`), camouflages included, except the right-click menu's Buy
on an owned item. Since 2026-11-05 (the owner: buying through the upgrade
window or from the inventory should not ask) every caller says where the buy
was started (`try_buy_item`'s `origin`, a `BuyOrigin`: `SHOP`, `OWN`,
`UPGRADE`) and only a buy started in the shop asks, camouflages included, a
bundle never (`docs/notes/items.md` §6).

## The craft window: Buy and Sell, Continue and Exit

`InventoryList` and `MessageCraftItem` (`../flash/lobby_menu/src/survarium`): right-click on an item in
the craft window's list ("Upgrade") gives Buy and Sell only, and its result offers Continue and Exit (the
rules: `docs/notes/craft-menu.md`). The window locks the inventory list (`lockControls(true)`), and
`InventoryList.showContextMenu` ended at once on a locked list. On a locked list it now calls
`showCraftMenu(activeItem)` first: while a `MessageCraftItem` is on the lobby (`findCraftWindow`) it
shows that menu (Buy by the list's rule; Sell for an owned item, not the item being upgraded or the
donor the window holds, `MessageCraftItem.holds`); otherwise nothing, as before. Buy goes as the
item card's (`buyFromCraftMenu`: the dictionary id, the shop's price, the shop's seller); Sell
through the list's `sellItem`.

After an upgrade the window's result had one button, Ready, which closed it.
`MessageCraftItem.fillCraftedItem` now ends with `offerContinue()` (but for a refused craft): the window's Cancel shows again as Exit (the lobby already closes
the window on it, as on Ready), and the Ready button becomes Continue, side
by side. Continue closes the window as Exit does, puts the main item (as the
inventory has it now) on the item card (`setPrimaryItem`) and sends the
list's `craft_item` event, which opens the Upgrade window for it again with
an empty donor slot; an item with three modifications, or not craftable,
just closes. The words are the game's: `btn_exit_label` and
`btn_go_to_online_store` ("Continue"). Not in a release yet.

While the window is open, hovering an item of its list compares it with the item being upgraded
(`MessageCraftItem.compareWith`), not the equipped one; with no item being upgraded, no compare
card (`LobbyMenu.showItemHint`). Not in a release yet.

## Weapon scope context menu

`InventoryList` (`../flash/lobby_menu/src/survarium/controls/InventoryList.as`: `isUpgradeable`,
`upgradeLabelKey`) adds the existing localized **Optics** action to a compatible weapon's right-click menu, in the inventory or an equipped slot. It
opens the scope editor already shipped in revision 19. Armor still opens its
camouflage/decal editor. See [weapon_scopes.md §5](../docs/notes/weapon_scopes.md#5-weapon-right-click-menu).
This does not change the executable or reserve a release revision.

## Building the lobby movie (from source)

The lobby movie is one file (`flash_movies/lobby_menu.swf`), and a patch that
carries it replaces the players' movie whole. It is built one way only, from
its ActionScript source tree, `../flash/lobby_menu/src`
([lobby-source.md](../docs/notes/lobby-source.md)), with every lobby change as
an edit of it:

- the features written in the game's own classes: silver only (no gold, no
  exchange: the windows that offered gold, `StatusPanel`, `PlayerInfo`), Buy on owned items in `LobbyMenu` and
  `InventoryList` (#215), the craft window's Buy/Sell and Continue in
  `InventoryList` and `MessageCraftItem` (#216), Optics in `InventoryList`
  (#217), the player gear in `LobbyMenu` and `PlayerRanking` (#232);
- the rooms window, new UI, in `../flash/lobby_menu/src/survarium/rooms`, and
  its parts in `TopMenu`, `StatusPanel`, `LobbyMenu`, `BottomPanel` and
  `PlayerRanking`.

```sh
python build_lobby_026g.py <patch_24's lobby_menu.swf> <new lobby_menu.swf>
```

It needs `private/actionscript/tools.json` at the repository root (a JDK and
Apache Flex 4.16.1: `tools/actionscript/README.md`; `--tools` names another).
One Flex run compiles the whole tree, and its code goes into revision 24's
movie in place of all of its code; the graphics, fonts and symbol bindings are
revision 24's. About 5 seconds; the same tree gives the same bytes. Nothing is
patched in bytecode; the steps that did (`rooms_026g.py` to
`player_gear_026g.py` and their Java patchers, on revision 23's movie) are in
git history only.

Tests: `python -m unittest test_build_lobby`. Not in a release yet.

## What each patch does

Addresses are virtual addresses in the original exe. The evidence for each
is in `../docs/notes/client_patch.md`. Every site is compared with the
original bytes first, so another build or an already patched file is refused.

### 1. The code-signing certificate check (needed to start at all)

- **Where:** the function at 0x12A10 becomes `xor eax,eax; ret`.
- **Why:** at start the client takes the survarium.com certificate from its
  own Authenticode signature, asks Windows whether its chain is valid now,
  and only then verifies the signature of the `resources.db` file table with
  that key. The certificate ran out on 2015-06-06, so every unpatched 0.26g
  client stops with "Resources archive is corrupted".
- **What stays:** only the chain check is skipped. The file table's
  signature was still verified until revision 12, which skips that too
  (patch 24).

### 2. The single-instance check

- **Where:** `jz` at 0x124F9 becomes two `nop`s.
- **Why:** so that several clients run on one PC (testing). Not needed to
  play.

### 3. The world-hash interval, 100 ms to 50 ms

- **Where:** one byte at 0x27CC53, the default of
  `-hash_check_interval_in_ms` (`mov dword [esp+18h],64h` at 0x27CC4F).
- **Why:** a stock client reports the hash of its world for every second
  50 ms tick, on a phase of its own. Two clients on opposite phases never
  name the same tick, and the server cannot compare their worlds. With 50
  every client reports every tick (`../docs/MATCH-SYNC-026G.md`). The key on
  the command line still overrides it.

### 4. The snapshot upload (needed for any match)

- **Why:** 0.26g matches are lockstep: every client simulates, the server
  only relays inputs. A client can start or rejoin only from a snapshot of
  the world, and the original server made that snapshot itself. This server
  does not simulate the game, so a client must send its world when the
  server asks. A stock client has no code for that; without the patch the
  world stays frozen and no match starts.
- **Where:** two 5-byte jumps, at 0x27AEA1 (in `on_match_ready`) and at
  0x27A505 (the 0xF6 case of `on_match_packet_received`), into 420 bytes of
  new code that serializes the client's world with the game's own functions
  and sends it in chunks (0xE0).
- **Where the new code goes (default, `--cave=section`):** a new PE section
  `.srp` after `.reloc` (VA 0xA425000, 0x200 bytes of file). The section
  table gets an 8th entry, `SizeOfImage` grows by a page, and the
  Authenticode blob moves 0x200 bytes towards the end of the file, where the
  security directory finds it again. Nothing of the original code is lost.
- **The fallback (`--cave=hash-mismatch`):** the code overwrites
  `game_world_core::on_hash_mismatch` (0x325A60), which only the branch the
  second hook replaces could reach. No header changes. For a client that
  misbehaves with one more section; not needed so far.
- Base relocations that overlap rewritten bytes are made no-ops, and the PE
  checksum is recomputed.

### 5. The login certificate (a file, not an exe patch)

- **What:** `survarium_login_server.crt` goes over
  `game/resources/ssl/survarium_login_server.crt`.
- **Why:** the client accepts a login server only if its certificate
  verifies against that file. The file the 0.26g game ships is of another
  key and ran out in 2015. This one is the game's own name issued again
  (valid to 2037), and its key is in `../certs/`, which the login server
  reads.

### 6. The minimum video-memory check

- **Where:** `jae` at VA 0x106411 (file offset 0xF5811) becomes `jmp`:
  `73 0F` → `EB 0F`.
- **Why:** the client otherwise calls its fatal warning when detected
  video memory is below 320 MiB, which can prevent startup under Wine.
- **What stays:** the function returns the detected video-memory size;
  its existing 320 MiB fallback for an unknown size is unchanged. The
  allocator launch options above still apply.
- Included in both full builds (`section` and `hash-mismatch`).
  `--cert-only` keeps the original check. The self-test verifies the jump
  target, unchanged return paths, and absence of overlapping relocations.

### 7. The patch revision in the sign-in

- **Where:** `mov cl,[9DAE9Eh]` at VA 0x380862 (file offset 0x36FC62) in
  `sign_in_on_connected` becomes `mov byte [esp+27h],REVISION` / `nop`:
  `8A 0D 9E AE 9D 00` → `C6 44 24 27 01 90`. The base relocation under
  it (0x380864) is disabled.
- **Why:** nothing a client sends tells a server which patches its exe
  has. The first login message has a 12-byte version field, `0.26g0` and
  six zero bytes; the patch puts a revision number into byte 7 of it.
  The text stays `0.26g0`, so a server that does not read the byte takes
  the exe as before, and an exe without this patch says 0.
- **What reads it:** the login server writes it in its log
  (`Client version: 0.26g0, patch revision 1`). It refuses nobody for it.
- **The number:** `REVISION` in `patch_026g.py`, the last release's.
  Revision 1 introduced this field; revision 2 adds the optional embedded
  login server; revision 3 the battery rule (patch 9); revision 4 the two
  start-up sites (patch 10). Each release has a number of its own, the live
  exe's + 1 (`patch_026g.py --revision=<n>`, which changes this byte
  alone), and sets `REVISION` to it once it went out (the table above), so
  that the clients before and after it can be told apart.
- It is what the exe says of itself, not proof: a changed exe can say
  anything.
- Full builds only; `--cert-only` keeps the code. The self-test works the
  field out from the instructions that fill it. Tried on 2026-10-12: a
  game with the revision-1 exe signed in to a local server, which logged patch
  revision 1 (a client sending the stock field: 0). Evidence: `../docs/notes/client_patch.md` section 6.

### 8. An embedded default login server (optional)

- **Where:** the missing-server error at VA 0x1EB25B becomes a jump to a
  51-byte stub after the snapshot upload, followed by the address string.
- **When:** only when neither `-server` nor `-replay` was supplied. Explicit
  options keep their original behavior, including invalid-address handling.
- **How:** the stub initializes the parsed `server` key's 512-byte buffer,
  end pointer and string type, then rebuilds the temporary string and repeats
  the original option read from 0x1EAE2D. It uses relative addressing, so rebasing works.
- Both full variants support it. The PE section grows when needed, and the
  existing relocation and checksum handling covers the new hook. Build
  hashes depend on the embedded address.

### 9. The battery rule of modes 2 and 3 (the last battery)

- **Where:** two instructions of the rule's container event handler
  (0x313A50) become `nop`s: `jnz 313B3Bh` at 0x313A62 (6 bytes) and
  `mov byte [ecx+160h],0` at 0x313B34 (7 bytes).
- **Why:** when a team's count reaches the number of batteries, the rule
  stores its completion time and clears a byte that every container asks
  before it lets anybody use it; from then on it counts no event. Neither
  of the two is part of the world a client saves and loads. A client plays
  a tick, and plays it again when an older input of another player arrives
  (a rollback): the battery is in the player's hands again and the count is
  one less again, but the byte stays cleared. So the last battery cannot be
  put in on the replay, and nobody can put a battery in or take one out
  for the rest of the match. Seen on the server: the kept logs to
  2026-10-13 11:00 UTC have 19 matches in which a team had all batteries
  but one: 14 ended on time with that count, 5 by the rule with all of
  them. Two of the five were finished by a player alone (the other had
  left); in three, two or three players were still in, so the last
  battery does go in when no rollback crosses that tick.
- **What stays:** the rule counts every battery put in and taken out, sets
  the completion time when a team has all of them and takes it back when
  not, which is what the 0.21d client's rule does. The containers are never
  closed. The match is ended by the server, 3 s after it reads the full
  count, as before.
- **Run outside the game:** `run_battery_rule.ps1 <exe> [<exe> ...]` (the
  32-bit Windows PowerShell) copies the handler's 238 bytes out of an exe
  and calls them on a made-up rule object: nine puts, the tenth, a
  rollback of the count, the same put again, a take, a put. With a
  revision 2 exe (the players' download until 12:22 UTC of that day) the
  replayed put is not counted
  and the byte stays cleared; with revision 3 it is counted, the byte
  stays set, and a take sets the completion time back to none (run on
  2026-10-13, Windows 11).
- **State:** written on 2026-10-12; patch revision 3. Built and
  self-tested, the handler run outside the game; in the players' download
  since 2026-10-13 12:22 UTC. **A match played to a team's last battery
  with it was not watched yet.** A client without it and a
  client with it stay in step until a team's last battery goes in, and part
  there (the server then starts the fewer of them from the others' world).
  The login server writes each sign-in's revision into its log, so the
  two can be told apart there; it does nothing else with it.
- **Who gets it:** a game started through the launcher takes the new ZIP
  by itself at its next start. A game with a download from before the
  launcher (2026-10-13 09:50 UTC) keeps the old rule until its player
  takes the ZIP once more; on 2026-10-13 most accounts still signed in
  with revision 0, the first download.
- Evidence: `../docs/notes/match.md` section 26,
  `../docs/notes/client_patch.md` section 3.9.

### 10. A start with a Documents folder the game may not write to

- **Where:** two sites, eight bytes, no added code.
  `xor ecx,ecx` / `jmp short 46B12h` at VA 0x46B0B (file offset 0x35F0B)
  in `vfs::mounter::finish` becomes `jmp short 46B36h` / two `nop`s:
  `33 C9 EB 03` → `EB 29 90 90`. The address in `mov eax,9DB51Ch` at VA
  0x851CC7 (file offset 0x8410C7), the initializer of the option
  `-record_journal`, becomes 0x9DBF0C: `B8 1C B5 9D 00` → `B8 0C BF 9D 00`.
  Its base relocation stays: the new value is an address too.
- **Why:** the game keeps its settings, its log and a journal in the folder
  `Survarium` of the player's Documents folder, and no option moves it.
  A game that could not write there stopped two seconds after its start,
  before the sign-in, in one of two ways (two crash reports of one player,
  2026-10-13):
  - The folder is not there and cannot be made. Its mount (`user_data`)
    fails and has no root, the engine still announces it as mounted, and
    the receiver reads a null pointer: access violation at 0xF9A8F.
  - The folder is there, but a file cannot be made in it. The journal
    `Survarium_<Windows user>.journal` is made anew at every start, and
    the failure is a fatal assertion: "failed opening file … with open
    flags:create_always+read_write", a breakpoint at 0x40141.
- **What the patch does:** a mount without a root is not announced, and
  the journal is not recorded unless the game is started with
  `-record_journal=1` (the exe's own option; "1" was its default). Such a
  game starts and can be played. Its settings are not saved. The crash
  reporter of the download (`client/bugtrap`) tells the player so at the
  game's start, with what to do (its README, "Start-up notice").
- **What is lost:** the journal, a recording of a session's input and
  packets that only the same build's `-replay_journal` reads. It was
  overwritten at every start and nothing here uses it.
- **Does not change** what a match computes: revision 4 and revision 3
  play together.
- **Tried** (2026-10-13, Windows 11, the built exe, 28 s each, to the
  sign-in screen): a Documents folder that refuses new folders; a folder
  `Survarium` that refuses new files; an ordinary folder (no journal file
  is made); the same with `-record_journal=1` (the journal is made). No
  crash in any. Before the patch the first two give the two crashes
  above, with the addresses and registers of the reports.
- **Not tried:** a sign-in, a match, and saving the options into a folder
  that refuses the file.
- **What makes a Documents folder like that** is not known from the
  reports. Windows' "Controlled folder access" does it to a program it
  does not know, and so do file rights on a Documents folder moved to
  another drive.
- **State:** patch revision 4; in the players' download since 2026-10-13
  15:36 UTC, together with the crash reporter that has the notice. The
  first sign-in with revision 4 on the players' server was logged three
  minutes later.
- Full builds only; `--cert-only` keeps both. Evidence:
  `../docs/notes/client_patch.md` section 3.10; self-test check 14.

### 12. A game that goes on without the focus (patch revision 5)

A 0.26g game whose window is not the active one stands still. Its logic
thread calls `game::tick` only while `is_application_active` says yes
(`engine_world::logic_tick` 0x1A030; the call at 0x1A0E1, `test al,al`,
`jz 1A113h` at 0x1A0EE). Otherwise it only sends what is queued and keeps
the lobby link up (`docs/notes/transport.md` section 6.1): the link looks
healthy and nothing of the match moves. When the player comes back the
game makes up the time at once (its log says `eating time: N`).

In a match that is a player the others see frozen, who is resynced or
taken for gone on the way back. With the late join it is worse: before a
newcomer enters, every game in the match has to prepare for it and say so,
and a game without the focus says nothing, so one player who looks at
another window holds every join up until the server gives it up after 60 s.

The patch: the `jz` at 0x1A0EE becomes two nops (`74 23` -> `90 90`). The
call and the test stay. It is in the builds of patch revision 5 only
(`RUN_UNFOCUSED` in `patch_026g.py`); `--no-late-join` gives the revision 4
exe as before.

Measured on one PC, 2026-10-14, one game in the lobby of the test server,
windowed 1280x720, 15 s each (`eating time` is the game's own word for a
freeze it makes up):

| | without the patch | with it |
|---|---|---|
| Window not in front | froze: `eating time: 18.4`; 101 % of one core | no freeze; 46 % of one core |
| Window minimised | froze: `eating time: 15.6`; 100 % of one core | no freeze; 146 % of one core |
| Back in front | went on | went on |
| The mouse, window not in front | Windows' cursor shown, free on the whole desktop, also over the game | the same |

**Not run:** a match (so: mouse look and the cursor in a match, a fight
seen by the others while one game is not in front, a join with a game not
in front); exclusive fullscreen (`r_fullscreen on`) with alt-tab; another
graphics card. Known cost: a minimised game uses about half a core more
than before. Sound goes on while the game is not in front.

### 13. Three faults of a match that takes players in (patch revision 6)

Found on 2026-10-13: the first two in the first hour of late joins between real
players, the third on the test server the hour before.

**The scoreboard without the newcomer.** The hud's lists of players are
made once, by 0x234880 when the match is ready: the scoreboard
(`root.list_set_players`), a second list (`root.set_players_list`) and the
enemies' markers (`root.set_enemy_icon_ids`). Their loops end at the count
of players there is then. The late join raises the count and nothing made
the lists again: the players of the match did not see the newcomer in
their scoreboard (the newcomer, whose game made its lists after loading,
saw everybody). Now `install_context` calls `rebuild_lists` when the
newcomer has its place: it builds 0x234880's frame and goes into that
function where the lists are made (0x234E71), and a jump at 0x23572B
(`lists_hook`), right after the third list, takes that run out through the
function's own release of its two arrays and its own return. Nothing else
of 0x234880 runs again: not its subscription, not `root.reset`, not the
hud's items. The rows' numbers come back with the game's own refresh
(0x236D30, every tick the list is shown).

**A game that was given a newcomer died** (access violation at 0x399A8,
reading 0x20). A model that was in the scene was moved
(`scene::update_model` 0x14FED0, the tree's `move` 0x3A1C0) to a place of
numbers that are no numbers (the stack of the dump is full of them). The
tree then finds the box outside itself and grows to hold it
(`update_bounds` 0x396F0): it takes a node for a new root and doubles its
size until that reaches the size it worked out, which for such a box is
infinity, so it never does (`xmm3`, the size, is infinity in the dump).
The scene's trees have 1024 nodes each and nothing behind them (0.26g's
are fixed pools; the engine's older source, `loose_oct_tree.cpp`, asks the
heap), so the list of free nodes ends and the next one is read through
null. A bigger pool would end the same way. Which model, and why its place
was not set, is not known: the other game of that match took the same
newcomer and lived, and so did both games of the join before. The
newcomer's own game loaded the world twice and neither time got as far as
agreeing on a tick with the others (the joins that completed took about a
second for that); the first time it stopped answering 5 s after its world,
50 s before the server gave the join up. Its player reports the same tree
out of nodes while loading (their dump was not seen here). A jump at
0x39E52 in `loose_oct_tree::insert` (`box_hook`), the
one way a box gets into a tree (`move` goes through it too), takes a box
with a not-a-number or an infinity in it as a box of two metres at the
origin; the next move of the object puts it where it is.

**A game whose join the server gave up died** (access violation at
0x15EE8D, writing 0xD1; seen on the test server, in a game that had loaded
a match and was sent back before it was let in). `scene::remove_model`
0x14FF60 looks the model up in the scene's list; when it is not there it
writes "remove_model: model is not inserted now" and goes on all the same,
and erasing the list's end moves what lies after it and adds to a
reference count through it. The tree's erase before that did not fault, so
the model's object was in a tree: LIKELY the model is another scene's,
which holds it. The first instruction of that branch (0x14FF9B) is now a
jump to the function's own end (0x1501F2), where it lets go of the model
it was given (the pointer comes by value, the function counts it down) and
returns: this scene is not touched, the other lets go of the model in its
own time.

Checked: `selftest_026g.py` (682 checks) and `check_revision6_026g.py`,
which runs the three with Unicorn on made-up objects: the three lists made
again for 2 to 20 players, every value released once, registers and stack
as before, no slot of the frame read that nothing wrote, the hud and the
options not written, a hud of another match left alone, the game's own
start of a match unchanged; twenty boxes; a model in the scene, and one not
in it held by two and by one (one count less, the model's end called for
the last and only then). `check_late_join_026g.py` passes as before. Read
in the game's own ActionScript (`hud.swf`, `player_icons.swf`,
`replay_player.swf`, decompiled): each of the three lists replaces what
the movie held (`PlayerList.addPlayersList` sets `_players` and makes both
teams' data anew, keeping which row is the player's own;
`set_players_list` makes a new data provider; `set_enemy_icon_ids` sets
the array), and a row's kills and deaths come with the next
`list_update_player`, which the game sends for every place each tick the
list is shown (0x23B421, `[hud+0BAh]` = `root.show_player_list`). A
newcomer's marker is made by the install itself (`player_ready` 0x1F29D0
calls 0x23BD60, `root.create_player_icon`). **Not run: a game.**

A newcomer has its row from the moment its place is prepared, before it is
let in; one whose join fails keeps the row, as a player of the first
roster who never loaded does.

### 14. A newcomer's grenades and traps (patch revision 7)

Found on 2026-10-14, in the first hour of revision 6: a player with
grenades in its kits was taken into matches being played four times and
never let in, while six players without them were. In revision 5 the games
already in such a match died as well.

**The cause.** The world saves and loads its game objects in the order of
one list (core+0x18914 the first, +0x18918 the last; `0x2D2699` saves into
the state's "game objects" bytes, `0x2D2B74` loads, each object reading as
many bytes as it wrote). The game appends to it: a slot's grenades and
traps when the slot is bound (`grenade_core` 0x303820 and the trap item
0x3067A0, one node for each of a stack, called from the inventory's bind
0x2FC7D0), and then the map's objects (0x285750). So a match's start gives
the slots' objects first and the map's after them. The late join binds a
newcomer's slot when the map's objects are in the list already: the games
in the match had the newcomer's grenades after them, and the newcomer's own
game, which starts with its slot, before them. One game's world was read by
the other's objects in another order, the hashes of the two never agreed,
and the server never let the player in. Seen in the worlds the players'
server kept: six zero bytes at the very end of the game objects after such
a player's place was prepared (six grenades, one byte each untouched),
where a match's start has its grenades in front. An object that reads a
byte not its own can take it for a thrown grenade and read a place of
numbers that are no numbers after it: LIKELY the crashes of patch 13's
second fault. Medkits, oxygen, artefacts and weapons are saved inside
their player's own block and were never part of this.

**The order.** `bind_slot_reserved` (the code every slot is bound through,
at a match's start and later) puts the list aside, lets the game bind the
slot into an empty one, and puts what was aside behind the slot's nodes. A
slot's grenades and traps are in front of everything the list held, so the
list is the same in every game whenever a slot was bound: the newest
slot's first, the map's objects last.

**The states to roll back to.** A game keeps the states of the last half
second and loads one whenever an input for a past tick arrives. Those
saved before a slot was added have no bytes for its objects, and a load
would read every object after them out of step. `lj_native_install` counts
the nodes the slot added and puts as many zero bytes in front of the game
objects of every kept state (core+0x2FBC, item+0xBDF30): a grenade or a
trap nobody has touched saves itself as one zero byte
(`grenade_core::serialize` 0x303DA0, `booby_trap_core::serialize`
0x3052E0). A state with no room for them is left (the buffer is 8192
bytes; the largest seen is 525).

**Revisions.** The order is another one than revisions 5 and 6 keep when
two slots have grenades, so a game of 7 and an older one part as soon as
one of those is thrown: a server asks for 7. While the games of a match
install a new place their worlds differ for a moment (a game with it, a
game not yet); the match server does not judge hash reports then
(`Game::installing`), where it resynced a game that was a second or more
behind the others with a world of the other kind.

Checked: `check_late_join_026g.py`, with Unicorn on made-up objects. The
game's own binders (slot, kit, inventory, grenade item and grenade, trap
item) run on kits with grenades and traps: slots bound at a start, the
map's objects added, slots bound after them, and each time the list is the
newest slot's objects, the older slots', the map's, every link there and
back right and every id of its place's range. The whole install runs for
newcomers with 2, 1, 9 and 2 such objects into a world with two saved
objects and four kept states: each state has the zero bytes in front and
its size, one that fits to the byte too, one with no room is untouched,
and a newcomer with none changes nothing. Seven copies of the code with
one thing broken each all fail that check. The one zero byte is the
game's own word: a grenade and a trap as their constructors leave them,
saved and loaded by their own code there, write that byte and read it
back with no change (a thrown grenade and an armed trap fail it).
`selftest_026g.py`: 682 checks.
The match server's tests: two new ones (games that install a place five
seconds apart are not resynced for it; a game really out of step meanwhile
is resynced afterwards), both failing without the change. **Not run: a
game.** Not known: whether anything else of a grenade or a trap differs
between a slot bound at the start and one bound later (none was found in
the code read); what a join does while a grenade is in the air.

### 15. The game's clock is brought up faster (patch revision 8)

A game's clock is a timer the server sets when it gives the game a world,
and corrects afterwards (0xE7: so many ms behind, or ahead). The game makes
a correction up by running its clock a little faster or slower than real
time, and by how much is a setting of the game itself:
`time_floating_factor`, a console variable (the object at 0xA9D0D0), 0.05
in the original, from 0.001 to 0.5. The `game` constructor copies it into
the timer once (0x1E9848, timer+28h), and `set_offset` (0x1EE9E0) runs the
timer at 1 + it, or 1 - it, for `offset / factor` ms. Nothing else reads
it, and it is not among the settings the game keeps in `user.cfg`.

**Why it matters.** A game that is given a world starts behind the match
clock: the world is its uploader's of half a second ago, the clock is put
350 ms past it, and the wait for the game's answer to the new time and the
way there come on top. The further away the player, the more: about 260 ms
at a round trip of 22 ms, 465 ms at 196 ms, 540 to 690 ms at 300 ms
(players' server, 2026-10-14). The match server drops every input that
comes more than 250 ms after its tick. So while the clock is on its way
the player's keys do nothing: for about a second in the first case, 6 to
9 s in the second, 13 to 15 s in the third, after every start, every late
join and every resync. A player at 196 ms whose link stalled every 10 to
20 s was resynced twelve times in four minutes and was out of step or
without keys for most of them.

**The patch.** The default becomes 0.3: four bytes of data at 0xA799C8
(`cd cc 4c 3d` to `9a 99 99 3e`), no code, no address. 465 ms are made up
in 1.6 s instead of 9.3 s; the clock runs at 130 % of real time for that
long, or at 70 % for a correction backwards. Only in the builds with the
late join (revision 8).

**Revisions.** Nothing that a match computes reads the factor: a game of 8
and a game of 7 play one match, and a server need not ask for 8. The match
server times its corrections by the revision a game signed in with
(`match_msgs::clock`, `docs/notes/match.md` section 29): when it measures
the clock again, and for how long it puts a resync off because a clock is
on its way. A game whose revision it has wrong is brought up all the same,
each correction being the lag that is left (two tests).

Checked: `selftest_026g.py`, 700 checks (check 16: the four bytes; the
console variable's name, its value's address, its least and its most; the
constructor as the value's only reader and the timer field it fills;
`set_offset` and the 0xE7 handler; no relocation under the bytes). Against
the revision 7 exe of the same server the build differs in the revision
byte, those four bytes and two bytes of the PE checksum (compared byte by
byte). The match server's tests: a game of each factor put 400 ms behind is right
after 8 s and after 1.3 s, in one correction, never a ms too far. **Not
run: a game.** Not known: how a game looks and sounds while its clock
runs at 130 % or at 70 % for a second or two; whether anything limits how
many ticks one frame may play (the load after a sync plays seven at once).

### 16. A newcomer's score on the scoreboard (patch revision 9)

The score in a player's row of the scoreboard (Tab) is the game's own
count, not the server's: the statistics handler adds each event's points
to the place's score as it happens (`on_event`, 0x26DB80), and works every
score out again from the statistics whenever the game loads a kept state
(`recalculate_scores` 0x26DC30, from `deserialize` 0x26DB60), which it does
whenever an input for a past tick comes. But it works out only as many
places as the roster had when the match began: `start_match` (0x26D8D0)
copies that count, and the late join raises the roster, not this copy.

**Why it matters.** Seen on the players' server on 2026-10-14: on the
scoreboard of a player who was there from the start, every newcomer had 2.7
to 4 times its score (1088 for 327, 467 for 116), because every rollback
over one of its kills added the kill once more; a newcomer with no kill had
953, a score of an earlier match left in its row. The results screen, the
rewards and the quests are the server's and were right; the starting
players' scores, and every kill and death, were right too.

**The patch.** The count `start_match` keeps becomes 20: `mov al,[edx+577B5h]`
at 0x26D8F3 becomes `mov al,14h` and four nops. Every place's score is then
worked out again at every state loaded, and at a match's start every row
is told a 0. Places with no player have zero statistics, so a 0 that is
never told again. The shared statistics' own count, which rewards and
events read, stays the roster's. Only in the builds with the late join
(revision 9).

**Revisions.** The score is neither in the world nor in its hash: games of
9, 8 and 7 play one match, and a server need not ask for 9.

Checked: `selftest_026g.py`, 733 checks with patch 17's (check 17: the six bytes; how
`start_match` uses the count, and the shared count's copy beside it; the
handler's `deserialize` and `recalculate_scores`; its vtable; every reader
of the count in the code; no branch or address into the six bytes).
`check_late_join_026g.py` runs the game's own `start_match`, `on_event` and
`deserialize` on a made-up handler: a newcomer that scores 13 in a tick has
13, 26 and 52 after 0, 1 and 3 rollbacks in the original and 13 each time
in the build, and every row starts at 0 in the build only. Against the
revision 8 exe of the same server the build differs in the revision byte,
the six bytes, one byte of patch 17 and the PE checksum (compared byte by
byte). **Not run: a game.**

### 17. The team points during the countdown (patch revision 9)

While a match waits or counts down ("Awaiting match start"), the hud shows
the countdown and nothing else of the match: `game_world_ui::update_ui`
(0x23B200) jumps from the countdown to the players' rows (0x23B328), past
the drawing of the team points (0x2364C0), Team Deathmatch's last players
(0x236610) and the respawn time (0x236C30), which play goes through. The
hud's state has the match's own numbers then (the rules fill it every
tick), but the bars kept the last match's until play began: seen on the
players' server, 63 and 53 under the countdown, the end of the match
before. The original game's code (0.21d's is the same), not srp's.

**The patch.** The jump goes to those three drawings instead
(`jmp 0x23b421` at 0x23B328 becomes `jmp 0x23b383`, one byte of its
displacement), which then go on to the rows as play does; the match time
and the start's tip stay out. They draw then what the first refresh of
play would: the start's team points (150 and 150 in Team Deathmatch, 0
and 0 with batteries); the last players, which the hud movie hides while
a team has tickets; and a respawn time of 0 (no player waits for one
during a countdown, as the saved worlds of a real match show), an empty
text. Only in the builds with the late join (revision 9).

**Revisions.** Nothing a match computes: it plays with 7, 8 and 9 alike.

Checked: `selftest_026g.py` check 18 (the five bytes; both paths of the
original around them; the calls from the new target to the rows and no
branch leaving elsewhere; each call's `ret`, so both paths reach the
target as deep; the three hud movie functions; no branch or address
inside the jump). `check_late_join_026g.py` runs the game's own
instructions from the countdown's test to the rows on a made-up hud, the
functions taken at their entry, for modes 4, 3 and 2: in the original the
countdown calls the pregame's two alone, in the build then the three play
calls, with the same arguments; play is the same in both; the stack, the
registers it keeps and the player's count end as they began; given the
original for the build it fails. **Not run: a game.**

### 18. The weapons' random state at a spawn (patch revision 10)

Seen on the players' server on 2026-10-14 (match 0x1ce45cbe): the players
taken in late went out of step six times, each time when another player
respawned into a kit he had used before the newcomer's world was made; the
newcomer's world then differed from the others' in four bytes, that kit's
weapon's random state.

**The cause.** A weapon's random state (boost's `minstd_rand` at
weapon_core+0x5BC: the spread of its shots draws from it) is 1 from the
weapon's constructor, is changed by shots, and is read from a world by the
weapon's deserialize; nothing else sets it, a respawn neither (it keeps the
kit's weapon objects). A place has a player object for each of its three
kits, and a world holds the current kit's alone. So a game that loaded a
world during a match (a newcomer, a game that was resynced) had every other
kit's weapons at 1 where the games that had played them had drawn from
them, and the respawn into such a kit put it out of step at once. The same
in any game after a rollback over a respawn into another kit.

**The patch.** The respawn rule's spawn (0x316DA0, every respawn rule's
slot +0x5C; every spawn of a match, the first too, and every replay of its
tick goes through it) makes the new kit current, initialises its player
(the inventory set up from the profile) and then puts it into the world.
Between the two, the five bytes at 0x316EA0 (`mov edx,[esi]` /
`mov eax,[edx+20h]`) become a call into `weapon_random_hook`
(`late_join_026g.asm`, 118 bytes at offset 3712): each weapon of the
player's inventory (an item whose vtable+0x70 is weapon_core's bind
0x2E30F0, which only the vtables of weapon_core and the client's weapon
hold) gets the state

    h = ((place << 16 | kit << 8 | slot) * 0x85EBCA6B) ^ (time * 0x9E3779B1)   (mod 2^32)
    state = (h ^ h >> 15) mod (2^31 - 2) + 1                                     (1 .. 2^31-2)

from the player's place and kit, the slot and the spawn's time (the
place's respawn time, which is in the world), the same in every game; then
the two instructions. Nothing else is written. Only in the builds with the
late join (revision 10).

**Revisions.** What a match computes changes at a match's first spawn: a
game of 10 and one of 9 or before part there. **A server asks for 10**
(`SRP_MIN_PATCH_REVISION=10`), put
up with the ZIP. The match server needs no change (its clock factor and
the late join take 10). A late-join exe built by hand (`0x80`) is let in
and has not the patch.

Checked: `selftest_026g.py`, 760 checks, 787 with patch 19 (check 19: the five bytes; the
spawn around them and the registers it leaves; the respawn rules' vtables
and tick; the player's initialize and the inventory's 23 slots; the
weapon's constructor, the bind's two vtables and their RTTI names; every
[register+5BCh] of `.text`, five; the hook's code; nothing aims inside the
five bytes). `check_late_join_026g.py` runs the game's own spawn on made-up
objects as two games, one whose new kit's weapons were drawn from and one
whose are at 1, for kit changes, protection times, places and times: the
build gives both the formula's states, after the initialize and before the
player is in the world, and changes nothing else; the game's own
`uniform_01` then draws the same numbers in both; the original leaves them
apart; the original and three copies of the build with one thing broken
fail it. Against the revision 9 exe of the same server the build differs in
the revision byte, the five bytes, the hook's 118 bytes and two bytes of
the PE checksum. **Not run: a game.** Not covered: the single-player
respawn rule (the test mode `loopback_client` alone). Evidence:
`../docs/notes/client_patch.md` section 3.18.

### 19. A first world's kit choices and inputs (patch revision 10)

Reported for the matches after 0x1ce45cbe (2026-10-14): of fifteen late
joins five were out of step from their first tick and never let in; for two
a newcomer's world with one player's next kit changed gave the newcomer's
reported hash byte for byte.

**The cause.** A game's first world of a match (a newcomer's, a game's that
comes back, and every game's at a match's start) gives each place of the
world an entry at the world's time in its input list and in its list of kit
choices (0x2D4AB0 and 0x2D4B50, from deserialize 0x2D0B70). The kit choice
was the kit in use, not the one the world has chosen for the next spawn,
and both entries went at the newest end of their list even when it held
newer entries (relayed between the server's 0xE5 and the world), where
every other writer keeps the lists in time order. Each tick the place's
next kit is the newest entry at or before the tick, looked for from the
newest end: the world's choice, and a choice relayed for later, were both
hidden by the kit in use, and the newcomer respawned that player in
another kit than the other games did.

**The patch.** The two calls (0x2D1050, 0x2D1055) go to `first_load_input`
and `first_load_kit` (`late_join_026g.asm`, offsets 3840 and 3920, 178
bytes): the same input entry, the kit choice being the world's next kit
(place+0xD), each put before the oldest newer entry with the game's own
allocator and insert (0x1FCB60, 0x2D4850); an entry of the same time is
given the value, as before. With no newer entry the lists are what the
game's own left, but for the kit choice's value. Every game that loads the
same world, the starters of a match too, makes the same entries; a kit
chosen before a match's first world is no longer reset to the kit in use
(in every game of revision 10 alike).

Checked: `selftest_026g.py` check 20 (787 checks in all, 814 with patch 20).
`check_late_join_026g.py` runs the game's own block of the first world
with its allocator and insert on made-up lists, then the places' own tick:
in the build the lists are in time order and the next kit after each tick
is what a game that played has (a choice before the world, one relayed for
later, empty lists, an entry of the world's time, every entry newer, the
pool full); in the original the later choice is hidden by the kit in use;
the original and three broken copies fail it. **Not run: a game**, nor the
reported hash reproduction with this exe. Evidence:
`../docs/notes/client_patch.md` section 3.19.

### 20. A weapon's state that a respawn kept (patch revision 10)

Reported by the analysis of the evening matches of 2026-10-14: in 0x47d5fa35
most of the stale-kit desyncs were not of the random state (patch 18): six
games each at two respawns. The saved worlds of those ticks differ in a
second field of the kit's weapon in slot 10, `m_last_tick_time_in_ms`: -1 in
the games that had loaded a world during the match, the time of its last use
in the others (also 0x3c67147b, and 0x4301ed65 where the random states were
equal).

**The cause.** A world holds the current kit's player alone, and a weapon
keeps fields through a death and a respawn that the initialize does not set
again: the time it was last ticked (only the weapon in hand is ticked; the
activation of the weapon put in hand sets it to the spawn's time), the fire
mode and the kind of ammunition the player chose in a kit. Each is in the world's bytes, so a respawn into a kit used
before a game's world parted that game from the others. (The removal of the
player before a spawn already puts each magazine and chambered round back
into the ammunition stack, 0x2E18E0.)

**The patch.** At 0x316E57 in the respawn rule's spawn, the load of the
initialize's call (`8B 16 8B 52 1C`) becomes a call into `kit_state_hook`
(`late_join_026g.asm`, offset 3168, 129 bytes). Before the initialize, each
weapon of the new kit gets -1 as the time of its last tick (the
constructor's) at every spawn, and at a spawn into another kit than the one
before also the constructor's fire mode and ammunition kind, and as a safety
(LIKELY no-ops) the magazine, chamber, aim, recoil and the activation's
other fields (`KIT_STATE_FIELDS` in `patch_026g.py`). The initialize then activates the weapon it puts in hand
as a new one. A respawn into the same kit keeps the fire mode and the
ammunition kind: that kit's player is in every world.

**What changes for a player**, exactly two things, both only after a
respawn into another kit: the kit's fire mode is forgotten (mode 0, the
first of the weapon's own list: always one it has), and the kit's
ammunition kind is forgotten (kind 0, the first of the kit's own matching
ammunition: always one it carries). Edge case: when kind 0's rounds are used
up and another kind has rounds, the weapon spawns empty on kind 0 and goes
to a kind with rounds only at an input with action bit 20h or 40h (PROVEN
that it waits; which keys NOT PROVEN, LIKELY fire or reload). No round is
lost or added and the server's ammunition count does not change. Not proven:
rounds in a magazine of a weapon with no ammunition item would be dropped
(no way to load such a magazine was found).

Checked: `selftest_026g.py` check 21 (814 checks in all).
`check_late_join_026g.py` runs the game's own spawn with both hooks on
made-up objects as two games (a kit's last life left in one, a new weapon
in the other): in the build the new kit's weapons are alike in both at the
initialize and nothing else changes; the original and five broken copies
fail it. **Not run: a game.** Not covered: a player who left and came back
(the place's profile, its storage of stacks and the conditions kept in it,
is in no world while he is away), the other items' own timers (artefacts,
medkits, oxygen tanks: none differed in the saved worlds), a weapon's state
machine and scope. Evidence: `../docs/notes/client_patch.md` section 3.20.

### 21. Team Deathmatch's rule counts a death after its own end (patch revision 11)

Seen live on 2026-10-14/15 (match.md section 28): in two matches a team ran
out of tickets, its players died one by one and one dead player of the team
stayed in the rule's alive mask for the rest of the match, so the match
ran to the time limit though the team was out. The rule's death handler
(`tdm_rule_core` event 1, 0x316090) returns at once when its completion
time (+0x12C) is set; the end test sets it when a team has no tickets and
no player alive; it is not serialized, so a rollback over the death that
completed the rule leaves it set and the replay of that death is skipped
(the same as patch 9 for the batteries).

**The patch.** The `jnz` at 0x3160F1 (six bytes) after `cmp dword
[esi+12Ch],-1` becomes six nops: every death is counted (the ticket, the
alive mask, the respawn flag, the end test, which sets the time again,
later). Nothing else reads the time; the match's end is the server's, which
reads the alive mask.

**Not compatible with 10 or before:** a game of 10 does not count the death
and a game of 11 does, so they part at the first death after a team is out.
A server asks for 11 together with the ZIP.

Checked: `selftest_026g.py` check 22. `run_tdm_rule.ps1` (32-bit PowerShell)
copies the handler, its recount and its end test out of an exe to their own
addresses and plays two teams of two through a team's last death, a rollback
(tickets and masks loaded, not the time) and the replay: the original and
the build without the late join keep the dead player in the mask and count
no later death (what the live matches showed); revision 11 clears the bit
again and counts the later death. **Not run: a game.** Evidence:
`../docs/notes/client_patch.md` section 3.21.

### 22. A field-of-view setting (patch revision 11)

The game already had most of a FOV setting. The video options' constructor
(0x262929) makes option 7, a float slider of snap 1 labelled
`st_fov_option` in all five languages ("FOV", "Sichtfeld", "Поле зрения",
"Поле зору"), and names its console variable "fov". No console variable
has that name, so the option was never shown, and the camera's projection
multiplies the zoom by a fixed 1.134464 (65 degrees, vertical). The console
variable `uro_fov` (65.0, range 0 to 100, saved in `user.cfg`) is read by
nothing. 0.10b had the setting: its projection took `cam_fov` (60 to 70)
times the zoom, which 0.26g made a constant.

**The zoom.** 0x1F3480 sets the camera's zoom at every frame to
`1 + (target − 1) × p`, p being the aim's progress (0 to 1, weapon+5FCh) and
the target a fitted scope's `fov_factor` (0.25 in all three scopes: PSO-1,
Giperon, Leupold, 4×) or else the weapon's `1 / aim_zoom_factor` (iron
sights: 1.1× for 25 weapons, 1.2× for 12, none for 13); 1 when nothing is
aimed.

**The patch.** Only in the builds with the late join (revision 11):
- (until 2026-11-06: option 7 named "uro_fov" for "fov", 0x262930; it is the
  original's dead option again, the Gameplay tab's slider of patch 83 sets
  uro_fov, `../docs/notes/client_patch.md` "The setting's slider");
- uro_fov's registration stores 50 as its least instead of 0 (0x85AE06; at
  0 the projection would divide by tan 0); the most stays 100; the
  relocation of the replaced store's address (0x85AE0D) is disabled;
- the camera's projection (0x242900) jumps at its first instruction to
  `fov_projection` (`late_join_026g.asm`, offset 576, 126 bytes), which
  passes the matrix builder (0xEEE30) the same arguments with the angle
  `65·z + (uro_fov − 65) · max(0, (z − 0.25) / 0.75)` degrees, z the zoom.
  It writes no XMM register but xmm0 and returns as the original. The rest
  of the original's body stays, never reached.

So the angle is `uro_fov` unaimed and nearly it with iron sights (at 100
a 1.1× sight shows about 90 degrees for 91), and through a scope exactly the
original's 65 × 0.25, whatever the setting: no setting magnifies a scope
more than another, and a reticle drawn for the original stays right.
Between them the angle slides with the aim, always falling as the zoom goes
in. At 65 every view is the original's. The projection is slot 0 of the
vtables of every camera (`game_camera`, `lobby_camera`,
`free_fly_camera`, `demo_camera`, `player_input_handler`), so the lobby,
spectator and replay views follow the setting too: at 100 the lobby scene
is wider and the character smaller, at 50 it is cropped. That is left so on
purpose; a player who prefers the stock lobby sets 65. Like every float
console variable of the game (the setter 0xECB70), `uro_fov` takes its
least for a value out of range or not a number: a wrong `uro_fov` in
`user.cfg` gives 50, as a wrong `sensitivity` gives 0.01.

**Revisions.** A camera is not in what a match computes, and the 65-degree
constant has no other reader; two games of 11 at different FOVs played one
match (below). Revision 11 as a whole does not play with 10 (patch 21).

Checked: `selftest_026g.py` check 23 (the two sites in place and the hook in
the original, the build and the build without the late join; the strings
and the console variable they name; `fov_projection`'s read of uro_fov, its
blend and constants, its call, its XMM registers and return; the original's
body after the hook; the five vtables by RTTI; the registration's least and
most; no branch or address inside the sites), the hook with the other
late-join hooks (check 15), and the relocation table (it differs from the
build without the late join in the two disabled entries alone).
`check_fov_026g.py` (Python `unicorn`) runs the original and the revision 11
projection, the real matrix builder included, at five settings, six zooms
(unaimed, iron sights of 1.1 and 1.2, half and nine tenths into a scope, a
full scope) and three viewports: the matrices are the formula's, a full
scope is the original's at every setting, and at 65 every zoom is the
original's. Run in a game (an earlier form of the patch, the zoom
multiplied by uro_fov, with the slider of `fov_slider_026g.py`): the lobby
and a match on a local server with a test bot, the view following the
slider, the value saved to `user.cfg`. Run with this revision's exe
(2026-10-15, a local server built from this source): two games in one
match, one at 100 with an SVD and a PSO-1, one at 50 with an SKS; the scope
and the iron sights as the formula says, and each game started with the
setting saved in its `user.cfg`. **Not run:** Windows; the Giperon and the
Leupold (their `fov_factor` is the PSO-1's); a game of 10 with one of 11.

### 23. A battery under the floor (patch revision 11)

A battery dropped by a dying carrier can end up under the floor or in a
wall, out of reach for the rest of the match. A use casts a 2 m ray from the
eye (`base_player::detect_usable_object`, 0x2D84C0) and stops at the first
wall or floor, so such a battery is never found.

**The patch.** Only in the builds with the late join (revision 11): the ray
is 4 m, the scan goes past walls and floors, and what it finds beyond one
or beyond 2 m is taken only if it is a battery whose centre is below the
player's feet and at most 1 m from them across: down to about 2.3 m,
through more than one floor too. Look down at the battery and press use. A battery in plain view within 2 m is taken as before; one
seen through a wall at one's own height is not, nor is anything else
behind a wall (containers, traps). `../docs/notes/client_patch.md` section
3.23 has the details.

**In Rust** (2026-11-07): the whole of `detect_usable_object` is the Rust
patch `use_scan` (`rust/patches/src/use_scan.rs`; until then
`pickup_below_026g.asm` at `.srp`+800h, three hooks inside the native
scan). One lohook at the function's start (`use.detect`) runs the game's
rule and patch 23's (`use_scan/rule.rs`, no address in it) over the game's
own calls, each the one the native makes, in its order (`vostok/use_scan.rs`:
the world's `ray_query`, each hit's collision group and collision
geometry, `query_objects_by_type` with the cast to usable, `can_use`, a
battery's `get_transform`, the multithreaded allocator's frees), and leaves
by the function's `ret 8`. Patch 85 (a dropped gun) asks the same scan
first: what Use takes in the game now (a battery lying by a body's gun, a
container, a crate, a trap) has no gun prompt and Use goes to the game.

**Revisions.** Every game runs every player's use: a game of 11 takes a
battery an older game leaves, so they part there; a server asks for 11
(patch 21 already made it ask).

Checked: `check_use_scan_026g.py` runs the original and the build's
scan on made-up hits (it says nothing about what the real collision world
returns); with `--against` a build of the asm patch 23 (main 87e5a379f)
answers its 72 scans the same (the answer, the point, the ray, every call
in order, the frees as a set; an across of 0.98 m is caught);
`cargo test` of the rule; `check_engine_026g.py` for its seven
functions (five virtual, their 61 vtables.tsv rows from the RTTI). Run in a game (2026-10-15, two games on Keln): a battery stuck
under the floor was picked up, with a test build that called the base
`get_transform` directly. **Not run:** this revision's exe; Windows.

### 24 to 26. The replay viewer (patch revision 11)

Three fixes for watching a `.sur` replay (`../docs/notes/replay_sur.md`). None
of them changes what a match computes, so they have no revision of their own.
`../docs/notes/client_patch.md` section 3.24 has the details.

- **24. The camera follows its player into another kit.** In first or third
  person, the replay died at the watched player's first respawn into another
  kit. The replay client's "is this place mine" slot now points at a thunk
  in int3 padding (0x242A69) that answers "the place the camera is on", so
  the camera re-attaches as a live game's does.
- **25. A weapon with a full sound queue drops the new sound.** A weapon's
  delayed sounds (64 items) asserted when full, which killed a replay about
  50 s after a seek. A cave in int3 padding (0x835A20) checks for a full
  queue and takes the game's own drop exit. This is in live games too, where
  a full queue also killed the game.
- **26. A replay's seek empties the weapons' sound queues.** After a seek,
  sounds waited behind the old timeline's (late reloads) and the
  re-simulated ones came all at once. At a user seek, `seek_clear`
  (`replay_seek_026g.asm` at `.srp`+A00h) empties every client weapon's queue
  and fx history as the weapon's destructor does. A live game never runs
  the seek.

Checked: `selftest_026g.py` (`test_replay_reattach`, `test_sound_queue`,
`test_seek_clear`); `check_late_join_026g.py` runs the seek's end on made-up
places, kits and weapons. Run in a game (2026-10-15, the replay of a 10-player
match, first person, with seeks back and forth): no crash past the old crash
points, sounds on time after seeks.

### 24. No resources.db signature check (patch revision 12)

The game checks the `resources.db` file table's signature at every start
with the survarium.com certificate's key, so an archive with a new file
stops it ("Resources archive is corrupted"). Patch 24 makes verify_sign's
call of `verify_sign_impl` at 0x135D3 (`E8 D8 F6 FF FF`) `xor eax,eax` and
three nops (`31 C0 90 90 90`): 0 is that function's "good signature", and
the `add esp,10h` after the call still takes its four arguments off. The
game then checks no signature of the archive: what it loads is what the
updater wrote, and the updater checks everything it writes by sha256. What
is signed and the call site: `../docs/notes/client_patch.md` section 0.1.

The first, never-released writing of revision 12 checked the table with
Skhid's key instead (a 413-byte routine at 0xC00 of `.srp`, a signer and the
public key in this folder). Revision 12 as built drops it: anyone can patch a check out of their own exe, so it protected
nothing the updater's sha256 checks do not. Checked: `selftest_026g.py`
check 25 (`test_db_signature`). **Not run: a game.**

### 25. The sign-in reports a stale shell_32.dll in the temporary folder (patch revision 12)

One known cheat, "Survarium 0.26e ESP NAT" (`cmtme.exe`, 2014), is an
external wallhack. It touches the game once: it writes a DLL to
`%TEMP%\shell_32.dll` and loads it into the game. As shipped it does not even
start against 0.26g (wrong window title, wrong offsets), and a player can
patch any client check out, so this is not a defence — it is a first count,
to learn whether the cheat is here at all. The file is a good signal because
the cheat never removes it, so it also marks a PC where the cheat ran before.

Patch 25 sends `sign_in_on_connected`'s last two writes of the version field
(`mov [edi+8],eax` / `mov [edi+0Ch],dl` at 0x380889) to `stale_dll_026g.asm`
at 0xE00 of `.srp`, which asks Windows whether `%TEMP%\shell_32.dll` exists
and does the two writes with the answer in byte 8: `0x80` the check ran,
`0x81` it ran and the file is there. The text still ends at byte 6 and the
revision stays in byte 7, so older servers read the exe unchanged. The login
server logs byte 8 as `cheat_flags` and warns on `0x01`. How it works and
what the flags mean: `../docs/notes/client_patch.md` section 3.25.

Checked: `selftest_026g.py` check 26 (`test_stale_dll`), and
`check_stale_dll_026g.py` runs the check on made-up machines, loaded as is
and rebased (needs Unicorn). **Not run: a game.**

### 27. The game mounts resources.db's patches (patch revision 12)

The players' `resources.db` stays the retail file. New content comes as
small archives of the same format next to it, `resources.db.patch_<n>`,
and the game mounts each of them after `resources.db` at the same place,
`resources`, in the order of n. A patch is numbered by the patch revision
that brought it (a `resources.db.patch_12` would come with revision 12), so
most numbers are missing: not every revision has content. The file system gives a path
to the archive mounted last that has it, so a patch replaces whole files of
`resources.db` and of the patches before it, and adds new files; it cannot
take a file away.

The game mounts its one archive in one place: `query_db` (0xE7D90) formats
`../../resources.db` and calls `query_mount_archive` (0xE7EF0) once, at
0xE7E3E. Patch 27 makes that call go to `mount_patches_026g.asm` at 0x400 of
`.srp`. The code first looks for `..\..\resources.db.patch_<n>` for every n
from 1 to 255, the revision being a byte (the game's own path format with
`.patch_%u` after it; `CreateFileA` asking no access, as patch 25 does), and
notes each one that is there; a missing number is skipped. A number is
missing only when `CreateFileA` fails with file not found (2) or path not
found (3). Any other failure, such as a folder by that name or access denied
(both 5) or a sharing violation (32), stops the game before anything is
mounted with "Cannot open resources.db.patch_<n> (error <e>). Please run the
Skhid launcher again." Then it queues `resources.db` with the game's own
arguments and each patch found, in order, with the same arguments but the
path. The file system runs its tasks in that order.

Every mount calls `on_mounted_resources` (0x147E0) when it is done. That
function keeps the mount in the game's world (engine_world+2D4h) and goes on
with the start-up, so only the last call may get there. Its first six bytes
become a jump into the same code: each start-up mount takes a reference (a
mount nobody holds is unmounted) and is kept at 0x800 of `.srpd`, the late
join's writable page; all but the last then return. The last calls the
game's code with `resources.db`'s mount, which the world keeps (its own
reference) as without patches, and then lets the state's reference of
`resources.db` go. So `resources.db` ends with the one reference it has in
the retail game, the world's, and each patch with one, the state's. A later
call is the game's own.

At quit nothing lets these go, as in the retail game: the game never
releases the world's `resources.db` mount. Its end (`engine_world`'s
destructor, 0x13C10 → 0x13C20 → 0x15C80) lets the shaders' masks go
(engine_world+2C0h), stops its threads and ends in 0x1135E0("", 0) →
TerminateProcess; no code of the exe lets the three mounts at
engine_world+2D4h..2DCh go, and +2D4h is read nowhere after its callback
stores it. The patches stay mounted to the end the same way.
`../docs/notes/client_patch.md` section 0.2 has the addresses.

A patch that is not there but that the exe requires stops the game too, as
one that cannot be opened (patch 49, below).

A file that is there and cannot be mounted stops the game: for
`resources.db` with the game's own "Cannot mount resources", for a patch
with "Cannot mount resources.db.patch_<n>. Please run the Skhid launcher
again." A patch that is there is never skipped. Only the exact name counts:
`.patch_014`, `.patch_0` or `.patch_256` is not a patch. A patch's name must
not end in `.db`: the game's file system keeps one handle, for whichever
path ending in `.db` it opened first, and gives that handle for every path
that ends in `.db`, so such a patch would be read from `resources.db`. What
the game does at a mount, and why later mounts win:
`../docs/notes/client_patch.md` section 0.2.

Checked: `selftest_026g.py` check 27 (`test_mount_patches`), check 15's
layout of `.srp`'s first page, and `check_mount_patches_026g.py`, which runs
both hooks, the code and the game's own `on_mounted_resources` up to its
next step on made-up folders (no patch; 1 to 3; 1 to 4 and 6; 14 and 17
alone; 255 alone; `.patch_1.db`, `_01`, `_0`, `_256`, none of them a patch;
0 to 300, of which 1 to 255 are mounted; 1 and 3 with 2 path not found; a
folder named `resources.db.patch_3`, patch 1 access denied and patch 255 in
use, each stopping before any mount; 14, 17, 20 with the first, the middle
or the last failing to mount; the retail archive failing), loaded as is and
rebased (Unicorn 2.1.4). Each ends with `resources.db` held once and each
patch once, nothing unmounted. In a game: the first writing of patch 27 (the
build numbered 14, `770f7c04…5b09`) mounted patches 14, 15 and 16 in Wine on
2026-10-16 and they worked; its quit was not watched, and this build has not
been run. The updater fetches the patch files the server lists and checks
them (`../../client/updater`), and the play scripts start the game only when
it says they are ready (`updater_skhid.exe ready`).

### 28. The transparent effects scroll their textures as in 0.29 (patch revision 13)

London's sand and fire layers (0.29 content) are forward_default materials
that move their textures over time: `use_uv_scrolling` with
`transparency_map_scroll_u_direction`, `..._v_direction`,
`emissive_map_scroll_u_direction` and `..._v_direction`. 0.26g's forward
effect ignores the switch; its g_stage effect (opaque materials) already has
it. 0.26g has the rest: the shader macro `CONFIG_USE_UV_SCROLLING` (bit 6 of
the shader configuration's byte 0Ch, the same bit g_stage sets), the
constants `uv_scrolling_parameters0/1` and the function that sends them.
0.29's shaders scroll by `scene_time * scene_slomo`; 0.26g binds
`scene_time` only.

Patch 28 adds to `effect_fstage_default_materials::compile` (0x1D8F40) what
0.29's (0x7F2600 of 0.29a3) does for the switch, and nothing else of 0.29:

- a jump at 0x1D90DC (after `use_angle_depended_attenuation` is read, before
  `compile_begin`) sets the bit when the material has `use_uv_scrolling`
  true and `use_sequence` is not on (0.29 ignores the switch for a flipbook);
- a jump at 0x1D94CB (after the uv distortion's constants) sends, when the
  bit is set, `uv_scrolling_parameters0 = (0, 0, tu, tv)` and
  `uv_scrolling_parameters1 = (eu, ev, 0, 0)` with `set_constant<float4>`
  (0x1A8900), the four values read as compile reads its numbers (a float, or
  an integer made a float), a value the material does not have being 0;
- a jump at 0x14990F in `renderer_context`'s constructor (after its last
  `register_constant_binding`, `scene_time`'s) binds `scene_slomo` to a
  constant 1.0 in `.srp`, with the game's own calls for `scene_time`; the
  binding only reads it.

The code is `forward_scroll_026g.asm` at 0xB00 of `.srp` (540 bytes, after
the replay's seek code and before the shell_32.dll check; 0xC00..0xE00 alone,
which revision 12 freed, is 512 bytes). Each jump's two instructions are
done there after the new work, every register is restored, and the image
may be rebased (the delta in ebp, as in the other caves).

What it changes for players depends on the data: the game picks a compiled
shader by a key made of the macros that `shaders/masks` lists for it, and
the retail `forward_base` entry lists no scrolling (all 495 retail
`forward_base.ps` keys have `_` in that slot). With the retail
`resources.db` the bit picks the same shader as before, and a constant the
shader does not have is skipped (`shader_constant_table::apply_bindings`
0x3D7F40). The scrolling shows only with a patch archive that adds the macro
to the masks and the scrolling shaders (the London plan, phase 2). Details:
`../docs/notes/client_patch.md` section 3.26.

Checked: `selftest_026g.py` check 28 (`test_forward_scroll`), check 15's
layout of `.srp`'s first page, and `check_forward_scroll_026g.py` (Unicorn
2.1.4, the image as is and rebased): the original's and the build's whole
compile run on 11 made-up materials (no switch, the switch false, true with
no values, four floats, integers, two values, with `use_sequence` on and
off, with uv distortion, angle fade and soft edges, a constant emissive),
the config, compile_begin and the effect compiler's calls being stubs: the
build makes the original's calls with the original's values, the bit aside,
and the two constants right after the uv distortion's, every register after
both hooks as the original's; the third hook against the game's own binding
of `scene_time` with three made-up name counts. And the byte comparison with
revision 12 (Revision 13 above). **Not run: a game**; no scrolling shader
exists in any `resources.db` yet.

### 29. "Watch replay" (patch revision 14)

The lobby's History offers a replay of a match the player played
(`../docs/notes/replay_watch.md`). The server sends the link
`replay <id> <token> <sha256> <size>` in its `9E` message (not shown), and
the movie's "Download and watch" calls `video_tutorial_link_clicked`. The
original callback (0x28CDA0) opened whatever text was in that buffer with
ShellExecute: a file, a program or a URL of the server's choice.

**The patch.** Only in the builds with the late join (revision 14): a jump at
0x28CDB6 goes to `replay_link` (`replay_link_026g.asm`, 472 bytes at
`.srp`+220h, after the upload code and the default server, 0x21D at the
most, and ending at +3F8h before the patches' mount at +400h; the first page
has no other gap that large). Exactly
`replay ` + 8 + ` ` + 32 + ` ` + 64 lower-case hex digits + ` ` + 1 to 10
digits starts `updater_skhid.exe` from the game exe's own folder (a fixed name;
the link is only its arguments: the command line `updater_skhid.exe <link>`)
with that folder as its working folder, through `CreateProcessW`. Any other
text does nothing. The updater checks the link again, downloads the replay
from its own site, checks it and starts a second game with `-match_replay=`
(`../../client/updater/README.md`). `../docs/notes/client_patch.md` section
3.27 has the details and how the game reads `-match_replay=` (a value in
quotes may hold spaces).

**Revisions.** Nothing a match computes: 11 to 14 play together. The
server sends a replay link only to a game of 14 or more (an older one would
open the text as a file, or ignore it).

Checked: `selftest_026g.py` check 29 (and check 15's layout of `.srp`'s first
page, also with the longest server address); `check_replay_link_026g.py`
runs the original and the build's callback on 16 good links, 83 others and
six refusals of Windows (the original opens all of them; the build starts the
updater for the good ones only); the byte comparison with revision 13
(Revision 14 above). **Not run:** a game.

### 30. One upgrade on several items (the next release's revision)

The owner's rule: one camouflage or decal of the stash is worn by any number
of items at once (later weapon attachments too). The client keeps an item's
upgrades as records of its own and shows one upgrade on several items as it
is: in each item's upgrade window, in a match, once in the stash
(`../docs/notes/items.md` §6). Only taking it off named no item: the upgrade
window's `drop_upgrade_item(item, upgrade)` sent `46 02 <kit> <upgrade>`, and
the answer `83 46 02 <upgrade>` (and `83 46 01`'s `detach_first`) took it off
the first item of the inventory that holds it.

**The patch.** Only in the builds with the late join: four sites call or jump
into `shared_upgrades_026g.asm` (181 bytes at `.srp`+700h, after the patches'
mount and before the pickup code):

- 0x29AAD5 (drop_upgrade_item's handler): `46 02 <kit> <upgrade> <item>`, the
  item the window shows;
- 0x275756 (`83 46 02`): reads `<upgrade> <item>`;
- 0x275589 (`83 46 01`, `detach_first`): the item the answer puts it on
  (it was in another slot of it);
- 0x27534D (`process_detach_upgrade`'s search): the item of that id instead of
  the first that holds the upgrade (0: the original's search).

Nothing is of the upgrade's kind: any upgrade, any slot. No change of
`lobby_menu.swf`. The server's rules are `session::v026g::items_rules`; it
reads only the new `46 02` and sends only the new `83 46 02`, so it needs a
game with this patch (`SRP_MIN_PATCH_REVISION`). Details:
`../docs/notes/client_patch.md` section 3.28.

**Revisions.** Nothing a match computes: a game with it and one without play
together. The release gives it its number.

Checked: `selftest_026g.py` check 31 (and check 15's layout of `.srp`'s
first page); `check_shared_upgrades_026g.py` runs the original's and the
build's drop handler and answers on made-up inventories (the original takes
the upgrade off the first item holding it, the build off the item named,
and no other); the byte comparison with the build of `main` (section 3.28:
197 bytes differ, all of them the patch's). **Not run:** a game.
### 31. A kit chosen before the match starts is played at once (the next release's revision)

The owner's wish (2026-10-17): a player may change kits while the match
loads, not only after a death. The rules the code and the tests follow are
`../docs/notes/kit-change-before-start.md`. In a match the stock game shows
the kits (`root.show_profiles_selector`) on the death screen alone, and a
kit chosen (0xDE, relayed as 0xF3) is only the place's next kit, taken at
the next spawn (`../docs/notes/match.md` section 23). Every game runs the
whole simulation, so a server alone cannot put a player into another kit:
each game must do it, at the same tick.

**The patch.** Only in the builds with the late join; `kit_change_026g.asm`,
319 bytes at `.srp`+EC0h (after the shell_32.dll check, before the late
join), four sites, nine nops at a fifth, and `warmup_look_026g.asm` and
`warmup_limits_026g.asm` at a sixth and a seventh (the camera before the
start, below):

- The respawn rule's tick (0x316820, its first three instructions) first
  goes over the places while the time rule's state is "waiting for players"
  or "countdown" (core+18FE4h is 1 or 2, the state the world holds), up to
  the end of the core's places (their buffer holds 20; past a match's places
  it holds what memory had, an earlier match's players among them). A place
  whose next kit is another than the kit in use, whose player is alive and
  in the world, and whose statistics record is all zero (no shot, hit, kill
  or other event, no damage taken from a player in this match), is
  respawned at once in the new kit by the rule's own two calls, as the tick
  does for a place it respawns: out of the world, then the spawn with a
  respawn point of the player's feet, its yaw and what is left of its
  invulnerability; with none left, one the spawn makes -1 (not
  invulnerable): a protection of 0 would leave the new kit's damage model
  with what an earlier life gave it, which a game that loaded a world since
  does not have (the world holds the kit in use's player alone). Same
  position and facing (the look up or down is level again, as at any
  spawn); no death, no respawn time, no ticket.
- The client's view of that spawn (0x2678E0 at 0x26794D) gave the player
  camera and the input's first person mode: the first test in a game had
  the player in first person and free to move during the countdown. It now
  gives the warmup's camera and input until play begins, by the state the
  network client keeps for the camera (+1F1C60h), exactly as the time
  rule's own change does (0x27B1C8).
- The hud: when the local player becomes alive (0x23B7DA) and at every
  update (0x23B7FC) a live player has the kits shown while the camera is the
  warmup's and hidden once it is play's. A pick (the `profile_changed`
  callback, 0x233F28) hid them and the hud's cursor with them, and the
  switch's own change of the local player showed them again, the cursor
  back at the centre: the second game test's "clicking on a new set resets
  cursor". The hiding after a pick is nine nops: the kits stay shown, the
  cursor where it is, for another pick, before the start and on the death
  screen alike, until the player becomes alive (in play they are then
  hidden as before).
- The camera before the start (2026-10-18, the owner: "once we added the
  ability to switch kits at start up, it is no longer possible to move the
  camera around"; of the first fix, which let every move turn it: "it
  should rotate when the left button is pressed, just like in the lobby",
  "and scrolling for putting the camera further, just like in the lobby").
  The kits show the hud's cursor, and the game world's input handler, which
  gets a mouse move before the player's input handler (priority 7 before
  10), moves the cursor and keeps the move while the cursor is shown
  (0x25D0E0), so the warmup camera, which the player's handler turns and
  moves by the wheel, stood still. The first ten bytes of 0x25D0E0 jump to
  `warmup_look_026g.asm` (184 bytes at `.srp`+D40h, after the weapon
  preview's draw; 12 bytes of state at `.srpd`+E20h). While the kits are
  shown and the player's handler is in the warmup mode (3, before play), as
  the lobby's character view (`lobby_camera`, 0x283D30):
  - plain moves are the cursor's; a left press that moves no more than 8
    units (|x| + |y|) is a click, the selector's;
  - past that, and at once with the lobby camera's own buttons (right, 5th,
    7th), it is a drag: the moves go to the player's handler and turn the
    camera, the cursor stays (the hud's pointer off the screen meanwhile,
    so a drag begun on a kit picks nothing) and is put back where it was at
    the first move after the release, as the lobby does;
  - the wheel goes on to the player's handler alone (no turn): nearer
    forward, further back, 0.12 a notch, the lobby's step.
  `warmup_limits_026g.asm` (101 bytes at `.srp`+A98h, after the seek code)
  has the end of a drag and, called from the warmup mode's store of the
  pitch (0x24225E, eight bytes), the lobby camera's limits: the pitch within
  60 degrees, the distance within 1.5 and 4.6 (the stock warmup had none,
  and starts at 1.0: it now starts at 1.5). On the death screen the cursor
  takes every move as before. The warmup mode gives the player's input no
  turn: nothing a match computes changes, and a game with it and one
  without play together.
  A button held from before the start is not the player's until it is let
  go (the owner, 2026-10-19: "players will shoot immediately once match
  starts"). The input world tells its handlers of a button still held at
  every frame (kind 2, 0x3B9D90); the game world's input handler takes the
  mouse's keys while the cursor is shown (0x25CF60), and the player's handler
  queues every kind it is told, which the play step takes as the weapon's
  action whatever the kind (0x23FF50, 0x242460): a drag still held when the
  kits went at the start fired the weapon at the next frame and once more at
  its release (and aimed with the right button; a pick held through a
  respawn did the same).
  The first ten bytes of 0x25CF60 jump to `warmup_keys_026g.asm` (88 bytes
  at `.srp`+9A8h, after the suicide key, up to the seek code; the buttons
  marked in a dword at `.srpd`+E30h): a button that goes down while the
  cursor is shown is marked; while the cursor is not shown a marked button's
  kind 2 and its release are taken there, and the release unmarks it. The
  owner, on the first writing, which gave the release on: "when mouse is
  released one shot still happens". The player's input only misses the held
  states and the release of a press it never had: nothing a match computes
  changes.

**Order.** In a tick of a build with the late join the respawn rule runs
first (tick list 0, its key from the core's counter), then the places, which
take the tick's kit choices (list 0, the late join's keys (place+1) << 16),
then the time rule (list 1): a choice is played at the next tick, a switch
at the latest in the tick whose time rule begins play, before that rule's
change, never after it; on the death screen a choice for the respawn's own
tick is the life after's. The camera follows the state of
the newest tick first played, which a rollback does not set back. The kit
choice a switch acts on is the one in the place's list at its tick, the
same relayed entries in every game. `../docs/notes/client_patch.md` section
3.29 has the proof.

**Revisions.** What a match computes changes from the first kit chosen by a
live player before the start: a game with this patch and one without part
there, so the release that brings it asks for its own revision
(`SRP_MIN_PATCH_REVISION`). The match server counts such choices for the
results (`PlayerResult::kit_changes`, a life more each for what the match is
taken to have used of a player's stacks, `session::v026g::autobuy`), and its
own bots pick a kit only once play has begun. Replays recorded before it in
which a live player chose a kit before the start play differently in a game
with it: the change is the `next` row of
`../crates/session/src/v026g/simulation_changes.tsv`, so they are refused from
the release that ships it on.

Checked: `selftest_026g.py` check 32 (and check 15's layout of `.srp`'s
first page); `check_kit_change_026g.py` runs the original's and the build's
tick start with the places' own tick and the game's own spawn, the view of a
kit change, the hud's update, the pick and a death screen's respawn on
made-up objects: the cases of the rules,
the ticks around the start (a choice for the last tick before play, for the
tick play begins in, for the first tick of play), the camera before play, in
play and after a rollback over the start, a late joiner, a match of six
places with an earlier match's entries past them, a new kit whose damage
model an earlier life left protected; the original never changes a live
player's kit; five broken builds fail it. **Run in a game
(2026-10-17):** the first build: the kit changed, the player was then in
first person and free to move during the countdown; with the view's fix
(d8acf39): the camera and the movement lock right, a click on a kit reset
the cursor; 3417c05 (two games): a switch before the start in place with
the warmup's camera, the cursor kept, re-picks, re-picks on the death
screen, the start's release. The camera before the start (check 32's two
sites, the lobby camera's buttons, step and limits;
`check_kit_change_026g.py`'s mouse moves through the input world's loop
and the warmup mode's step: plain moves, a click with a little movement, a
left drag, a right drag, the wheel, the wheel in a drag, the death screen,
a replay's cursor, no local player; the mouse's buttons through the
input world's loop and the game world's handler: a left or right drag held
into the start, a pick held through a respawn, a click let go before the
start, a press in play; nine broken builds fail it): its
first form, every move turning the camera, was run by the owner on
2026-10-18 ("not very good, it should rotate when the left button is
pressed"); **the drag, the wheel and the limits are not run in a game**.
**Not run in a game: this build**, which adds
the end of the places and the protection when none is left (a switch of a
place of the match plays as in 3417c05; with no protection left the new kit
is not invulnerable in either, and in the tick its protection ends it now
is not).

### 32. A weapon's upgrade window shows the weapon (the next release's revision)

The owner's decision (2026-10-17): a weapon's upgrade window (its one slot,
the scope, titled "Оптика" by the scope movie) shows the weapon in the
lobby's 3D view, so that the attached scope can be seen. The stock lobby
has one 3D subject, the character (`survarium::lobby_character`, lobby
ui+694h), which shows one kit of the account's profile and holds that
kit's weapon 1 (kit slot 7), or its weapon 2 (slot 10) when it has none.
The movie's `start_item_upgrade(item)` (0x29A870 -> 0x265DE0) aims the
lobby camera at the character's chest (look-at height 1.3, 0.8 for the
kit's legs armour, side angle 0.37, distance 1.5, the closest the camera
allows) and `exit_item_upgrade` (0x29A890 -> 0x265F10) aims it back. While
the window is open, the character's update gives its player the window's
mode (+F49Ch) at every frame, and the player is drawn for the decal
editor: in its bind pose (a T-pose) with its weapon put 10 m under it.
None of the movie's 112 callbacks shows an item on its own or moves the
camera.

**The patch.** Only in the builds with the late join;
`weapon_preview_026g.asm`, 431 bytes at `.srp`+2E50h (after the late join's
code, before `.srpd`), its state (three dwords) at 0xE00 of `.srpd`, and
`weapon_draw_026g.asm`, 32 bytes at `.srp`+D20h (after the forward effect's
scrolling, before the shell_32.dll check); seven sites:

- 0x265E73 (start_item_upgrade's call of the camera's setter 0x2845C0):
  when a kit of the account's profile holds the window's item as its
  weapon 1, or as its weapon 2 without a weapon 1, the character shows
  that kit (`setup_profile` 0x284B10 with that kit) and the camera looks at
  the weapon: height 1.05, distance 0.85, the side angle the original's
  (since patch 45, which both ends now jump to instead of the setter, the
  camera flies to the workbench and the weapon is drawn there: patch 45).
  A weapon no kit holds (only in the stash, or a weapon 2 behind a weapon
  1; 2026-10-21) is shown the same way in the hands of the kit chosen in
  the lobby, from a scratch copy of the account's profile in which it is
  that kit's weapon 1 (`weapon_stash_026g.asm` at `.srpw`+360h, which patch
  32's two finds call: client_patch.md section 3.30); the account's profile
  is never written. Armour gets the original's camera; armour the chosen kit
  does not wear is shown worn, from the same kind of scratch (patch 32's
  start, exit and setup_profile sites go through the stash code's entries
  first);
- 0x265F44 (exit_item_upgrade's call of the setter): after a weapon, the
  character shows the kit chosen in the lobby again (`setup_profile` with
  the profile's +4632h); the camera gets the original's values, a distance
  under 1.5 kept from a weapon's window raised to 1.5;
- 0x284B10 (`setup_profile`'s first six bytes, a jump): while a weapon is
  shown and the character has its upgrade window open (+D515h, which only
  the lobby's own character ever has), every call gets the weapon's kit,
  so the refresh that follows an attach or a detach (the server sends the
  inventory and the account) keeps the weapon in the hands;
- 0x284081, 0x28428C, 0x284316 (`movss xmmN,[0xA7AFA4]`, the camera tick's
  least distance 1.5 at the wheel, the step and the last clamp): a call and
  three nops; the least is 0.6 (0.3 since patch 45 round 5d) from a weapon's window until the camera is
  back at 1.49 after the close, 1.5 otherwise. The three base relocations
  of that address are disabled; the code finds its own address;
- 0x284A33 (the character's update, `mov [ecx+0F49Ch],al`: the player's
  decal pose): a call and a nop; 0 while the window shows its weapon, so
  the player keeps its lobby idle animation and the weapon in its hands;
  the window's mode otherwise.

The window's kit change is the 3D view's alone: the kit the lobby plays a
match with (+4632h) is not touched. No change of `lobby_menu.swf`.
`../docs/notes/client_patch.md` section 3.30 has the reading of the code.

**Revisions.** The lobby only, nothing a match computes: a game with it and
one without play together. The release gives it its number.

Checked: `selftest_026g.py` check 33 (and the late join's layout of
`.srp` and its relocation table); `check_weapon_preview_026g.py` runs the
original's and the build's start and close handlers (with the camera's own
setter, the decals, the scene and the input stubbed), `setup_profile`'s
entry, the tick's three clamp blocks and the decal pose's write on a
made-up lobby, as is and rebased: for armour, legs armour, a stash weapon, a weapon 2 behind a
weapon 1, a bad item index or kit, the build's camera, character, globals
and calls are the original's, at the start and the close; for a held
weapon (in the chosen kit or another, weapon 1 or a lone weapon 2) it
shows that kit and looks at it, and the close shows the chosen kit; the
clamps give the original's results and flags in 1,500 cases; the decal
pose is the window's mode as in the original, and 0 while a weapon is
shown. `check_weapon_stash_026g.py` runs the stash code with the game's
own profile and record ctors and copies: a stash weapon, a weapon 2 behind
a weapon 1 and one in no kit are the chosen kit's weapon 1 in the scratch
(a deep copy of the record, a new revision each time, allocated once),
the account's profile and inventory unwritten, and armour, an unknown
item, a profile with no inventory behind it, a full table and a failed
allocation give no kit. The byte comparison with the build of `main` 5fba57f: 514 bytes
differ, all of them the patch's (section 3.30). **Run in a game (2026-10-17):** the first
build (without the draw): the camera moved in, the character showed the
weapon's kit, but in a T-pose with no weapon (the decal pose). **Not run:**
this build; the camera's two values are first guesses.

### 33. The order of the archive reads is a strict weak ordering (the next release's revision)

A defect of the stock engine that patch 27 (the game mounts
`resources.db.patch_<n>` over `resources.db`) brought out: a lobby of the
scope round 4d crashed (2026-10-17, access violation at 0x10BCCE reading
0xA4) once its kit weapons' sounds were read from two archives at once.

**The stock code.** `hdd_manager::grab_sorted_queries` (0x10BE80) takes the
pending read queries into an array on its stack and sorts them with
std::sort (introsort 0x10BF70, final insertion sort 0x10C060), whose
partition and insertion (0x10C0B0, 0x10C280) run unguarded: they need the
predicate to be a strict weak ordering. The predicate,
`hdd_manager_sorting_predicate` (0x10BCC0; 0.21d's 0xCD023, the same code):
a null iterator first; the same parent (+158h): the priority (+2B8h)
descending; two reads of one archive (the same physical path): the file
offset; anything else: the parent, or the query's own address. Reads of one
archive go by offset, reads of two archives by address: three reads make a
cycle, and the sort then reads past the front of its array (the round 4d
dump: the 0 before a 58-slot buffer taken as a query). With `resources.db`
alone this could not happen between archives.

**The patch.** Only in the builds with the late join. The predicate's 0x1A0
bytes are `read_order_026g.asm` in place (the same entry and convention;
the body holds no address, nothing outside aims inside it, only the sort's
helpers call it): a strict lexicographic order, a null iterator first
(nulls by address), then the priority descending, reads from an archive
before others, then for archive reads the archive's path and the file
offset, for others the parent or the query. Reads of one archive keep
their order (within a priority, in file order); a batch of one archive
whose queries of different priorities share a parent sorts as before.
`../docs/notes/client_patch.md` section 3.31.

**Revisions.** Only the order in which files are read: nothing a match
computes, a game with it and one without play together. The release gives
it its number.

Checked: `selftest_026g.py` check 34; `check_read_order_026g.py` runs the
original's and the build's predicate with the game's own
`get_physical_path`, `strcmp_s` and `get_file_offs`, and the game's own
sort, as is and rebased: on random sets over two and three archives, loose
files and null iterators the build's predicate is a strict weak ordering
(the original's is not on most of them); on 400 random batches of 17 to 58
reads the sort with it compares only the array's queries and leaves them
sorted, where the original's leaves its array (on a fixed batch of 26 reads
of two archives it reads the 0 before the array, as in the dump); on
single-archive and loose-only sets in which different priorities share a
parent, the two answer alike for every pair. The byte comparison with the
build of `main` 8511083: 397 bytes differ, the PE checksum and the
predicate's. **Not run:** a game.

### 34. The commit_suicide key kills one's own player (the next release's revision)

The owner's wish (2026-10-17): a player can kill their own character in a
match, to respawn and pick another kit quickly. The game has the feature
but for one line: `commit_suicide` is game action 1Fh with the default key
K (action table entry 0xA74D20), a key bound to it reaches the input
handler's event buffer, and the match simulation kills a player whose input
has bit 24 (`base_player::process_quick_slots`, 0x2D8BC1: a hit on the
player's own head bone "brain", the attacker the player). The input builder
(`process_first_person_mode`, 0x241250) never asks for action 1Fh, so no
input ever had bit 24.

**The patch.** Only in the builds with the late join; the Rust patch
`suicide_key` (the framework's example, `../docs/notes/rust_patches.md`), a
`call` hook in the arena's block `patches.code` (until the framework, 40
bytes of NASM at `.srp`+980h). One site: the builder's last action call
(spot, `call 0x242460` at 0x242196) becomes a call of the generated entry,
which hands `suicide_key::on_spot` the call's typed arguments and an
`Original` of `action_present`. The handler asks `action_present` for action
1Fh and sets bit 24 when the key went down (one input, as spot), then does
the spot query the site wanted (`orig.call`) and returns its answer. K while
alive in a match: the player dies at the input's tick, in every game.

**Why the games agree.** Bit 24 is an ordinary input bit: the player's game
applies it at once, the server relays it as it relays every input, every
game applies it through the same code at the same tick. A game without the
patch cannot send it but plays it from the others, so no minimum revision is
needed for a match to agree, and the server must not take the bit out (the
sender has applied it already). What a match computes does not change: no
row in `simulation_changes.tsv`. The server's bots never send it.

**What it counts.** The game counts a kill whose killer is the victim as a
death and a suicide, no one's kill (`on_player_killed`, 0x2F4F38); Team
Deathmatch charges it a ticket; the score weighs a suicide 0; the respawn
time is a death's. No server rule is added. `../docs/notes/client_patch.md`
section 3.32 has the proof.

Checked: `selftest_026g.py` check 35 (and check 15's layout of `.srp`'s
first page); `cargo test` and `cargo clippy --all-targets` in `rust/` (i686);
`check_suicide_key_026g.py` runs the original's and the
build's last action call with the game's own event buffer code on made-up
handlers: K down sets bit 24 and nothing else, K repeated, released or held
from before does not, spot answers as in the original, nothing else of the
handler changes; the original never sets the bit. **Not run in a game**:
that the hit always kills (a helmet could take some of it), the respawn and
the kit pick after it.

### 35. A baked camouflage texture missing from the baker's list is baked again (the next release's revision)

A player's game stopped on 2026-10-17 while it loaded Rudnya with the fatal
assertion "Saved baked texture
user_data/streamed_textures/edge_researcher_03_legs_s3d.dds not found"
(check_loaded_baked_texture, 0x131B40; the dump's exe is revision 16). A
stock 0.26g defect: the code is the original's in every revision.

The camouflage/decal baker draws an item's textures with its camouflage
and decals once and saves them under user_data/streamed_textures; it keeps
a list of what it saved (path, CRC of the bytes, size, age, "a load is under
way"). A translate query that finds the path in the list loads the file;
check_loaded_baked_texture compares the CRC of the bytes loaded with the
first record of that path: the same, the file is used; another, the record
is **erased** and the query is made again (`result_requery`, "Texture %s is
corrupted!"), which bakes it anew. A record is made when the bake is done,
before the resources manager has written the file (the save is
asynchronous), and two characters wearing the same item bake one path twice
(two records). When more loads of a path are under way than it has records
and the bytes read are not the ones baked, the load after the last erase
finds no record and the game stopped. In the dump: no record of
`edge_researcher_03_legs_s3` (d, n, s), the list six records shorter than it
had been (as six erases leave it: two of each), and "Texture ..._s3s.dds is corrupted!" logged in the same
millisecond (`../docs/notes/client_patch.md` section 3.33).

**The patch.** Only in the builds with the late join: the `jnz` at 0x131C1E,
after the test of the assertion's "ignore" flag, becomes a `jmp` (one byte,
75h -> EBh) to the function's own end for that flag: `return false`. The
load is then made again like the one before it: the look-up finds nothing,
so the texture is baked. Nothing a match computes; a game with it and one
without play together.

Checked: `selftest_026g.py` check 36; `check_baked_texture_026g.py` runs the
original's and the build's look-up, push and check on a list where two
characters baked one texture and three loads read other bytes: the original
erases two records and stops at the assertion on the third load, the build
makes all three again and bakes it; a good load and one load of other bytes
give the original's result in both. Against the build of `main` without it,
the default build differs in the byte at file 0x12101E and the PE checksum
and in nothing else (compared byte by byte); `--no-late-join` gives the
revision 4 exe byte for byte as before. **Not run:** a game.

### 36. The memory arena "ai" is 4 MiB (the next release's revision)

A player's game stopped on 2026-10-17 at 14:24 UTC in a 20-player match
with the fatal "not enough memory for arena [ai]"
(`memory_doug_lea_allocator.cpp`, line 53; out_of_memory_with_crash
0xE4D70, `int3` at 0xE4EA5), on the cooker thread: weapon_core_cook's
query_weapon_states → the weapon's state machine (0x2DF780) →
ai::fsm::append_transition (0x38C970); three live dumps were read. A stock 0.26g limit that Skhid reaches: a match has one player object
for each usable kit, three a player since the kits of #148, and every
player object's weapons build their state machines in this arena.

The engine's memory arenas are registered by static initializers, each
with its size as a constant. The one of "ai" (0x1E8CCB:
`mov dword [esp+18h],40000h`, the name at 0x9FA4E0, then the registration
0xE21C0 into the arenas' list at [0xA808FC]) asks for 256 KiB; no option
or setting is read there, so nothing outside the exe can change it. The
arena (0xA967D8, a doug_lea allocator) is used by ai::fsm alone: 0x28 bytes
a transition, pop_state, generalized_action (`../docs/notes/client_patch.md`
section 3.34).

**The patch.** Only in the builds with the late join: the immediate becomes
400000h (4 MiB; one byte, file 0x1D80D1, 04h -> 40h). The other arenas
reserve about 1835 MiB together and the exe is large-address-aware, so the
3.75 MiB more is nothing to the address space. Nothing a match computes;
a game with it and one without play together.

Checked: `selftest_026g.py` check 37. Against the build of `main` 838cce1
without it, the default build (also with `--server=skhid.link:1234`, with
and without `--revision=`) differs in the byte at file 0x1D80D1 and the PE
checksum and in nothing else (compared byte by byte); `--no-late-join`,
`--cave=hash-mismatch` and `--cert-only` give the same exes as before.
**Not run:** a game.

### 37. Memory defaults match the launcher

(The multithreaded arena's half of this patch, at 0xE100F, was replaced by patch 129: the arena is
at least 1024 MiB, with or without `-mt_allocator`. The resource floor below is still this patch.)

When memory options are absent, the full client now defaults to a 768 MiB
multithreaded arena and a 768 MiB resource-memory floor, matching the existing
launcher and play scripts. Retail defaults were 376 MiB and 336 MiB respectively.
Explicit options retain their original behavior. The resource floor covers both
managed and unmanaged pools; the ordinary 2:1 split gives 512 MiB managed.

The 2026-10-18 map-loading report stopped while allocating 22,369,685 bytes for
`resources/textures/sky/cgskies_cloud.dds`. The dump reports 173,033,088 bytes
allocated out of 295,436,288 managed bytes. Native allocation requires one
contiguous block and does not compact the arena, so total free space alone does
not guarantee success. That pool is below the launcher's usual managed budget.
The reporter does not know the player's launch method, and the dump lacks the
heap and logs. This change closes the low-default launch path; it is not proof
that every cause of the reported intermittent failure has been eliminated.

Only two numeric immediates change: the default-only increment at 0xE100F
(16 MiB plus 752 MiB), and the resource floor at 0xE9985 (768 MiB before native
unit conversion). No allocator, eviction, timeout, texture, or simulation code
changes. Certificate-only and legacy builds without the late join retain the
retail defaults. No release revision is assigned here.

Validation: `check_memory_defaults_026g.py` executes the native default and
explicit/invalid option branches and a representative fragmented allocation.
The latter uses the report's byte counts with a synthetic block layout, since
the real blocks are absent from the dump. `selftest_026g.py` checks exact mutation
ranges and relocation safety. An isolated Wine/DXVK Mamayev Kurgan rain load
without either memory option rendered and produced verified server checkpoints;
the log reports 512 MiB managed and 256 MiB unmanaged.

Native 20-player comparisons on the same map found two budget failures:

| Startup settings | Retail defaults | Patched defaults |
| --- | --- | --- |
| Both memory options absent, no diagnostic cap | 3/3 failed with `not enough memory for arena [global mt allocator]` | 3/3 loaded and produced verified checkpoints |
| `-mt_allocator=768 -max_resources_size=422`, resource floor absent | 1/10 failed at `0x1068F7`, the reported managed-arena timeout site | 10/10 loaded and produced verified checkpoints |

The constrained failure requested 57,082,932 bytes for
`resources/projects/v021d_level_06/vegetation/suh_middle.veg`, with
134,967,552 allocated out of 295,043,072 managed bytes. It reproduces the
reported failure mechanism, not the exact sky-texture query or player history.
The native maximum-resource option clamps to at least the configured resource
floor, so the patched trials have 512 MiB managed instead of 281.375 MiB.
One uncapped retail run raised an access violation while reporting the
multithreaded-arena exhaustion; its log and dump comment preserve that failure.

The comparison exes differ only in the four bytes of the two default immediates.
Both use the existing 4 MiB AI arena patch and bypass disabled network-statistics
counters because the diagnostic direct connection omits the lobby profile.
They use the local ported-map resource archive and camera instrumentation, not
the player's unknown installation. The resource floor is a startup request;
the submitted pool size alone cannot establish the launch arguments. The native
single-region fallback at `0xE8891` through `0xE8BF3` can replace requested
256/512 MiB pools with smaller pools when address space is fragmented. The
instruction test supplies one synthetic 443,154,432-byte available region:
the native calculation and allocation calls produce 147,718,144 unmanaged and
295,436,288 managed bytes, exactly the reported managed size, despite a 768 MiB
request. This behavior is unchanged by the default patch; the test does not
establish that the player's address space had this layout.

With the smaller pool and one player, ten fresh-process loads and ten calls to
the native resource reload routine did not reproduce the failure. Memory use
settled after the second reload. That stress test does not exercise full
lobby-to-match transitions. The exact reported intermittent crash remains open.

### 38. Spotting: no cooldown, a battery's body spots it (the next release's revision)

The owner's decision (2026-10-19), on "battery marking sometimes works,
sometimes doesn't": no spot cooldown at all, and a hit on a battery's body
counts. The rules: `../docs/notes/spotting.md`.

A spot is input bit 28, set by the input builder when the spot key goes
down (0x2421A6; holding the key sends no more). Every game applies it in
`base_player::apply_new_input` (0x2D86E0): with the spotter's
`m_time_left_to_try_spot_in_ms` (+34Ch) at 0 it casts a 100 m ray from the
eye and walks the hits in order, spotting the first spottable one (group
800h) whose owner gives a player (an enemy: 10 s marked) or a geometry
whose first spottable says yes (a battery's use box
`!victory_item_collision!`); a hit that is walkable after a hittable one
ends the walk. Two faults made a spot of a battery fail:

- the wait was set to 3000 ms at every press, hit or miss (0x2D87C1): a
  miss, or a ray that met the battery's body, blocked the next press for
  3 s;
- the battery's rigid body (group 0A18h, 0x301CF5) is spottable and
  hittable, but its owner (victory_item_core+58h, vtable 0xA13EEC) returns
  neither a player nor a geometry: it spotted nothing, kept its hittable
  bit in the walk's mask, and the floor behind it then ended the walk
  before the use box.

**The patch.** Only in the builds with the late join. The wait's immediate
at 0x2D87C1 becomes 0 (two bytes): the field is 0 at every tick, so every
press spots, and every snapshot saves 0 (the countdown 0x2DAB44 keeps 0, a
spawn sets 0). And 0x2D8A0F, where the walk asks the owner for a geometry,
jumps to `spot_026g.asm` (50 bytes at `.srp`+7C0h, after the shared
upgrades' code, before the pickup code): for a battery's body owner it asks
the battery's spottable interface (+54h, vtable 0xA13EE0) and spots it as
the use box's path does (can_be_spotted_by 0x301CA0, set_spotted 0x301C40:
the spotter's team bit into +1A4h, event 4 to the match statistics) and
ends the walk; any other owner goes on as before.

**Pressing again.** A spot of a battery one's team already sees sets no
new bit and raises the statistics' event again, as the stock game did after
its 3 s; only the battery's first spotter gets the spot award, once. A spot
of an enemy marks them for 10 s again. A press is one ray; the identical
input of the next tick (the key held, the mouse still) is not applied again.

**Revisions.** It changes what a match computes (every game runs every
player's spot): a game with it and one without part at the first spot
(the wait, +34Ch, is saved with the player), so a server asks for the release's
revision, and `../crates/session/src/v026g/simulation_changes.tsv` has its
`next` row. `../docs/notes/client_patch.md` section 3.35 has the proof.

Checked: `selftest_026g.py` check 38 (and check 15's layout of `.srp`'s
first page); `check_spot_026g.py` runs the original's and the build's input
apply on made-up hits with the game's own spottable code: a miss and the
next press, one press held, a press every tick, a battery's body alone, the
body before the floor and the use box, the use box alone and first, the
other team's bit, a battery already spotted, a wall before it, an enemy, a
teammate (alone and before an enemy), a wall before an enemy, a spottable
hit with no owner behind it; the wait is 0 after every tick in the build.
Against the build of `main` 7cb1e45, the default build differs in the two
bytes at file 0x2C7BC7, the five of the hook at file 0x2C7E0F, the 50 bytes
of code at file 0xC4B1C0 and the PE checksum, and in nothing else (compared
byte by byte). **Not run in a game.**

### 39. The Tab list shows the places in the match, none whose player left (the next release's revision)

Seen since the rooms (#203): a player who left a running match (Leave
match) and came back into it as a new place had two rows in the others'
scoreboard, the old one grey. The owner (2026-10-19): "once they leave their
previous entry should be removed". The rules: `../docs/notes/tab-list-left-places.md`.

Every game of a match knows which places are in it: the server's 0xE9 and
0xEA (a leave by the player's own word, or a link silent for
`SRP_MATCH_CLIENT_TIMEOUT_SECONDS`) set and clear kd_stats_rule's `online`
of the place at the same tick in every game, the worlds carry it, and the
hud's state has it for every place. The stock hud made the scoreboard's
rows once for every roster place (and again after a late join's install)
and only greyed a place that was not in; no row was ever taken out.

**The patch.** Only in the builds with the late join; its code is in
`late_join_026g.asm` (`late_join_026g.section.bin` differs in its first
4096 bytes, the assembly; the C part is the same), three sites:

- the hud's initialize_match puts a place's row into the scoreboard only
  when the place is in the match (0x23537A calls `tab_row`, which appends
  it);
- while the list is shown, a change of the places in the match makes the
  three lists again, as a late join's install does (0x23B421 calls
  `tab_shown`; `lists_hook` ends that run too);
- the rows' refresh leaves the places not in the match alone (0x23B447..
  0x23B476 rewritten in place): no greying call for a place that left,
  which, by name, also greyed a player back under the same name.

A place that left has no row; one not in yet (loading at the start, a
newcomer being prepared) has none until it enters; a lost player is listed
until the server gives its place up and listed again if it comes back to
it. No row is greyed: a lost player greyed for the rooms' 90 s needs the
server to tell the games which places are away (`../docs/notes/client_patch.md`
section 3.36). The replay bar, the enemies' markers and the results screen
keep every place. Nothing a match computes: a game with it and one without
play together, and no server rule asks for it.

Checked: `selftest_026g.py` check 39; `check_tab_list_026g.py` runs the
build's and the original's rows with the game's own list code on made-up
huds (places leaving, entering, a late joiner, nobody, the list hidden, a
list made by the install, a player back under the same name);
`check_revision6_026g.py` and `check_late_join_026g.py` pass with it.
**Not run in a game.**

### 40. A model's LOD and screen factor at a wider field of view are the original's (the next release's revision)

The report: "with a high FOV some models disappear" (patch 22's setting).
The note with the rules and the table: `../docs/notes/fov_lod.md`.

Not the frustum: every culling frustum of the game is made from the
camera's real projection. The game measures a model by its size on the
screen, through the projection, with thresholds tuned at the stock 65
degrees (a zoom only narrows it):

- `static_render_model_instance::select_lod` (0x16EEC0): a static model
  whose `lod_switching.type` is 1 is drawn not at all (LOD 3) when half its
  box's larger NDC extent is under `param2`. In the retail `resources.db`,
  281 static models with LODs are of that type; 20 have a `param2` one sees
  (church_interior_details on Rudnya 0.2, Cologne's floor-vegetation corners
  0.15 and hoist 0.1, Mamayev Kurgan's base_a_mod_ 0.15 and base_b 0.07, a
  ladder 0.05, the Kamaz trucks 0.025, transformers 0.015, the UAZs, a
  flagpole and a ladder 0.01). On London (patch 14) 110 models are type 1
  with `param0` 0.2 to 0.9: at a wide angle a piece of a big room under
  that share of the screen falls to its LOD1, which leaves out whole
  surfaces (floors, bars, walls), the owner's "a third of the screen"; the
  correction of select_lod fixes it (`docs/notes/fov_lod.md`). Whether LOD 3
  really draws nothing is doubtful (a model there usually falls back to
  LOD 2 or 1, ibid.);
- `render_surface_instance::calculate_screen_factor` (0x158D80): the share
  of the screen times 100, for the shader LOD and the small casters dropped
  from the sun's and spot lights' shadows and rain occlusion.

A wider angle makes a model smaller on the screen: it lost detail and went
out sooner, at tan 32.5 / tan(F/2) of the distance (0.76 at 80, 0.53 at
100; a Kamaz side-on at 16:9 about 130 m at 65, 70 m at 100).

**The patch.** Only in the builds with the late join. `fov_projection`
(late_join_026g.asm) also keeps k = max(1, tan(A/2) / tan(65 z/2)) at +E50h
of the late join's state page, A the angle it passes (bit for bit patch
22's) and 65 z the stock angle at the same zoom: 1.0 at 65 and less and
through a full scope, 1.317 at 80, 1.871 at 100 unaimed. select_lod's
`mulss xmm0,[0x927A44]` (0.5, 0x16F2A4, 8 bytes) and the screen factor's
`fmul dword [0xA710D0]` (100, 0x1590AF, 6 bytes) become a call and nops to
`fov_lod_scale` and `fov_screen_scale` (late join +6E0h, +700h): the original
instruction, then the same by k when k is not 0 (the zeroed page before the
first projection), `ret`. The two relocations under them are disabled. At
65 and less every value is the original's bit for bit (x 1.0). At a wide
angle the game draws within a distance what 65 draws there, in a wider
view; k also reaches the shadow passes' LOD, a little more shadow detail.

**Revisions.** Rendering only: nothing a match computes reads a LOD or a
screen factor (collision and hits use the physics shapes), so it needs no
row in `../crates/session/src/v026g/simulation_changes.tsv`, and games with
and without it play together. `../docs/notes/client_patch.md` section 3.37
has the proof.

Checked: `selftest_026g.py` check 40 (and checks 15 and 23);
`check_fov_lod_026g.py` runs the projection at 50 to 100 and five zooms (k
is exactly 1 at 65 and less and through a scope), the game's select_lod on
a Kamaz-sized box (out at 113.7 m at 65 in the original, and at every
setting in the build; with k zeroed, 87.4 m at 80 and 62.5 m at 100) and
its screen factor (the original's at 20, 60 and 150 m). Against the build of
`main` bee9dc3, the default build differs in the two sites (file 0x15E6A4,
0x1484AF), two relocation entries (0xB71E47, 0xB728AD), fov_projection's
code (0xC4BC60..0xC4BCED), the two scales (0xC4C0E0..0xC4C115) and the PE
checksum, and in nothing else (compared byte by byte). **Not run in a game.**

### 43. The player's own row in the Tab list is gold (the next release's revision)

The owner (2026-10-20): "when pressing tab in game, your own character
should be gold", "just like when you kill or get killed in a kill list top
right". The rule: `../docs/notes/tab-list-left-places.md`.

The colour is already in the hud movie (`flash_movies/hud.swf`, revision
22's): the kill list (`CombatLogItem`) colours a name by its team code, 2
being the player's own (`game_world_ui::on_player_killed` 0x237C70 sends 2
when the place is the player's), with `Colors.PLAYER`; the Tab list's rows
(`PlayerListRenderer.updateData`) give the row marked `isLocal` the same
`Colors.PLAYER`. It is `Colors.YELLOW`, 0xF1E545, the lobby's colour for
the player too. `PlayerList` marks the row whose name is the one
`root.set_local_player` gave. In a match only `game_world::on_activate`
(0x1F9340) gives it, the name made as the rows' names are ("[clan] name",
or the name), and only while the journal records (the journal's flag
0xA8394C and its use, 1). Patch 14 (revision 4) turned the journal off by
default, so since then no hud has known its player's name and no row was
gold.

**The patch.** In every full build (with and without the late join, and
`--cave=hash-mismatch`), as patch 14's default is, 0x1F9478 `test eax,eax`
(`85 C0`) becomes `jmp short 0x1F9487` (`EB 0D`): past the journal's two
tests, the name is given whatever the journal does. Two bytes; no address
and no base relocation under them, nothing jumps into them. The movie is
not changed.

Rendering only: the hud's lists (and its chat, which keeps the name too).
Nothing a match computes: no row in
`../crates/session/src/v026g/simulation_changes.tsv`; games with and
without it play together. `../docs/notes/client_patch.md` section 3.40.

Checked: `selftest_026g.py` check 43; `check_tab_self_026g.py` runs
on_activate's end (0x1F9395..0x1F949D) with the game's own network client
(vtable 0x9FFB14: +20h, lobby_client), string code and memcpy on a
made-up world, for the builds with and without the late join and over
on_hash_mismatch: with the journal
off the original gives no name and the build "[clan] name" (or the name);
recording, both the same; played back, the build alone; when the network client's +20h
says no, neither. Against the build of `main` b97133d (revision 25's exe,
`12006dd9…a8b4`) the default build differs in file 0x1E8878..0x1E8879 and
the PE checksum, and in nothing else (compared byte by byte). **Not run in
a game.**

### 44. A weapon wears the camouflage its item holds (the next release's revision)

The owner's request: the armour camouflages on the regular weapons, painted where the premium
weapons are painted. The rules, the data and the masks: `../docs/notes/weapon-camo.md`,
`../scripts/weapon-camo/`.

weapon_cook::on_weapon_subresources_ready (0x24ACF0), when the weapon's model is loaded and before
it is given to the weapon (0x243A30), calls `weapon_ready` (`.srpw`+0) in place of `lea
eax,[esp+20h] / push eax` (0x24B0D2, five bytes, no address and no relocation under them). With a
non-empty `data/camouflage` text in the weapon's merged config (a camouflage upgrade puts it
there when the config has a `data` table), an `object/model` text and a model with a render
instance, it asks for one class-6Ch bake for each of the instance's surfaces (at most 8), as the
lobby's decal editor (0x2641A0) asks for one: a camouflage_decal_cook_parameters from the game's
allocator with the camouflage's diffuse, `_nmap` and `_smap` names, the identity as the
camouflage's matrix, the render instance (one reference more), the surface's index, the scale
(`data/camo_scale`, else 4.0), and the baked files' name `srpw1_<camouflage>_<model>_<surface>`
('/' and '\\' as '_'); its callback lets go of the reference and frees the parameters. The baker
refuses a surface whose diffuse has no `_aoac` texture (it stays plain). Every register and the
flags are kept, eax as the original's `lea` leaves it. Since patch 47 the camouflage's lookup and
its three names are patch 47's lookup and fill (`-DDECALS`), which add the weapon's decals: a
weapon with decals and no camouflage is baked too.

`.srpw` is a section of its own (code, three pages: patch 44's 739 bytes, the stash's and patch
45's on the first, patch 47's on the second and patch 48's on the third; raw bytes after `.srpc`'s,
the Authenticode blob moved behind them, SizeOfImage and SizeOfCode grown), at 0xA42A000, the page
after `.srpc` (patch 46's death camera, the page after `.srpd`): `.srp`'s two pages are full. It is
the last section. In the builds with the late join only.

Rendering only: no row in `../crates/session/src/v026g/simulation_changes.tsv`; games with and
without it play together (without it the weapon is plain). `../docs/notes/client_patch.md`
section 3.41.

Checked: `selftest_026g.py` check 44 (the function's frame and the site in the original, the
call, the code's calls into the game, nothing aiming inside the site, no relocation, `.srpw`'s
section entry and the headers); `check_weapon_camo_026g.py` (Unicorn, the cases in its header).
Run in a game on 2026-10-20 (a local server, an AK-12 and an Uzi with the City camouflage in
slot 2): both baked and the lobby's character showed them. **Not run in a match.**

### 45. A weapon's upgrade window shows the weapon above the workbench (the next release's revision)

The owner's request (2026-10-21): "instead of focusing on players the camera would flow to the
working table and the 3d model of the weapon would have been there", "that would allow you to
rotate it", "with weapon in the air". Patch 32 still decides which weapon is shown and has the
lobby's character hold it (a kit's weapon, or a stash weapon in the chosen kit's hands from a
scratch profile), with its scope and camouflage; patch 45 draws that held weapon on its own above
the workbench by the lobby's right wall and flies the camera there.

`weapon_table_026g.asm` at `.srpw`+800h (+6B0h since round 5d; 812 bytes in round 1, 1 488 in round 2 below; after the stash code), its state (40 bytes, then 48h)
at `.srpd`+F00h; it begins with four `jmp near` (its entries; a fifth, `left`, in round 2; a sixth, `ray`, in round 5c):

- **camera** (+0): patch 32's start_cam and exit_cam end with `jmp camera` instead of `jmp
  2845C0h` (the lobby camera's setter; `-DCAMERA` of `weapon_preview_026g.asm`, which without it
  assembles to the old bytes). With patch 32's mode 1 (a weapon shown) the table goes on: the
  camera's point is to fly to BENCH (4.25, 1.30, 3.05), its yaw to -pi/2 (the nearest turn of it
  to the camera's yaw: it looks along +x, at the wall), its pitch to -0.25 (a little above); the
  height 1.30 and the distance 1.2 go to the setter in place of patch 32's 1.05 and 0.85; the
  camera's yaw and pitch are kept (once, not again for a window over a weapon's) and the weapon's
  turn starts at 0. Mode 0 with the table on (the close, or an armour's window over a weapon's)
  puts it off: the point flies back to the origin, the yaw and pitch to the ones kept. Then the
  setter, every register but xmm0 and xmm1 the caller's.
- **tick** (+5): the camera's tick, `movss xmm0,[ebx+0C8h]` at 0x284020 (8 bytes, every tick
  passes it; ebx = camera+4, the frame's time at [esp+10h]) is `call tick` and 3 nops. While the
  table is on, the drag's moves of the frame (+CCh yaw, +D0h pitch) turn the weapon (its spin, and
  its tilt within +-1.2) and are taken from the camera. While the camera is the table's, its point
  x and z, yaw and pitch move 5 * dt of the way to their targets each tick (the height, the
  distance and the side angle by the camera's own steps); off and back within 0.001 of each, they
  are set to them and the camera is the stock one again. Every register but xmm0 (the original's
  load) kept.
- **cull** (+0Ah): player::mt_draw's `cmp byte [ebp-1],0 / jne 247592h` at 0x247576 (6 bytes) is
  `call cull` and a nop: the table's player takes the draw path even when its model was not seen
  last frame (the camera looks away from it), so that its held item is drawn.
- **matrix** (+0Fh): mt_draw's `cmp byte [ebx+0F49Ch],0 / je 2477C2h` at 0x24778F (9 bytes) is
  `call matrix` and 4 nops: for the table's player the held item is drawn (weapon::draw, its
  vtable+48h, which places the model, its locators and the scope) with the bench's transform,
  built in the decal pose's buffer [ebp-0E8h]: rotY(spin) * rotZ(tilt), the point 0.18 m along -z
  of the model (a rifle's middle; its grip is at the origin) at BENCH. Else the decal pose's or the
  hand's, as the original.

The table's player is the one mt_draw draws for lobby_character's update (its return address
0x284A83), whose character (the update's ebx, which mt_draw's prologue keeps at [ebp-0F0h]) has it
at +D2B8h and its upgrade window open (+D515h, only the lobby's own character), while the table is
on. Every other player, every other time: the original's code, byte for byte. No base relocation
is under a site and no branch aims inside one; the code holds no absolute address (its state by
call/pop, the return address it looks for from it).

Round 2 (2026-10-21, after the owner's game run; 1 488 bytes, state 48h bytes):

- **The middle.** The weapon turns about its middle and the middle is at BENCH: the model's box
  (render_model+108h, rotY(π) of the mesh, as the engine turns it) through bone 1's (`offset`)
  inverse bind (+140h of the skeleton render model) and its pose of the frame (mt_draw's
  [ebp−0Ch]), found from the held weapon (+1810h → +108h, vtable 0x9F0F24 checked → +62B4h).
  A default ±1 box, a missing piece or not a weapon: (0, 0, 0.2) of the model. The first round
  put (0, 0, −0.18) there, which was the stock's end (the box's frame is not the mesh's).
- **The weapon first.** The camera holds at the window's start (its three steps' speeds 0) until
  the weapon is drawn at the bench, which waits for the character's new player (+D2C4h −1,
  +D2C8h 0; the held item stays in the hand until then), at most 2 s; then it flies.
- **The left button** (`left`, +14h; 0x283D81 `test byte [eax+0Ch],52h / setne al`, 7 bytes, is
  `call left` and 2 nops): at the bench a left-button drag turns the weapon too, when its press
  is off the movie (the lobby menu's is_mouse_over_ui, asked once at the press's first move):
  a press on a control or on an item being dragged (a camouflage, a scope) does not, until it is
  let go. Buttons 52h as before.
- **The light** is data, not this patch: `../scripts/content-edit/lobby-scene-2.json` (it replaced lobby-bench-light-1).

Round 3 (2026-10-21; 1 798 bytes, state 90h bytes):

- **The middle, for every weapon.** The box's centre is taken with its x 0 (a box holds parts
  parked beside the gun: the VSS's, SR-3M's and Val's second magazine 28 cm, the SKS's stripper
  clip 35 cm), and the 26 weapons that leaves more than 1 cm off (a default box: the AK-74U, Vepr,
  TOZ-34, IZh-27, AKMN, A-545, TOZ-122 and their premiums; the Magnum, RPD, Mosin, Winchester,
  MP-153, Remington 700, Benelli, Saiga, 9A-91, Valmet, SOK-94) have their middle in a table by
  items_dict id: round 3b (after the crash on the VSS: round 3 read weapon+25Ch, not a pointer for a
  held weapon) takes it from the record the character's player was built from, inside the
  character (its current copy +D2B4h, kit +D2B5h, slot 7 or 10, the record's +4), every index
  checked, else the box's way. Round 3c: the table holds the middles in the bind space and they go
  through bone 1's pose of the frame as the box's (the Vepr's `preview` animation puts its body
  7 cm from where its 3rd-view idle does). Every weapon of the data is within
  1 cm (it was up to 16.5 cm): client_patch.md section 3.42 has the table.
- **No empty bench on a refresh.** Once the bench showed a weapon, a rebuild (a scope or a
  camouflage put on) keeps the current player's weapon there until the new player's comes. The
  new weapon still shows plain until its camouflage bake lands (the old player is let go when the
  new one comes in, on_player_ready: not reachable).
- **The interface for other patches:** the transform the weapon is drawn with at the bench (16
  floats, rows, v * M) at `.srpd`+F50h (the state's +50h, `WEAPON_TABLE_MATRIX_OFFSET`), a u32
  stamp +1 each frame at +48h (`WEAPON_TABLE_STAMP_OFFSET`); valid while the state's +0 and +28h
  are 1.

What it leaves alone: what patch 32 shows and builds (the kit, the scratch profile, the
character's pose and the decal pose, the least distance 0.6 while a weapon's window is open), the
wheel (it moves the camera nearer or further, 0.6 to 4.6), armour windows, the squad panels, a
match. Rendering and the lobby only: no row in
`../crates/session/src/v026g/simulation_changes.tsv`. `../docs/notes/client_patch.md` section 3.42.

Checked: `selftest_026g.py` check 45 (the four sites in the original and the build, nothing
aiming inside them, no relocation; the tick's frame and adds, the camera's point written by its
constructor alone, the character at the origin, mt_draw's caller and prologue and its decal
buffer; the code's branches) and check 33 (patch 32's two jumps to it); `check_weapon_table_026g.py`
(Unicorn, the cases in its header: 32 checks, as is and rebased) and `check_weapon_preview_026g.py`
(patch 32's cases with patch 45's camera). Round 1 (e93fb6f0) run in a game by the owner (the camera at the
bench, the weapon there); **round 2 not run in a game**.

Round 4 (2026-10-23, the owner on main + #312: "i can press attachments and then quickly esc, what
happens is that the weapon I was trying to attach too, is now in my hands, even though in inventory
I have a different weapon"): a window closed while its player is still being made left that player
in the hands. setup_profile's early return (what is asked is the shown copy) does not look at the
player being made; patch 32's scratch keeps the chosen kit's id with a revision of its own, so the
close's call for the account's kit returned early and the window's player came after it. On live
too (revisions 28 and 29 have the same code), for a weapon no kit holds and for armour the chosen
kit does not wear. Fixed by patch 48's `early` (below): the close waits for that player and the
account's is made right after it. `../docs/notes/client_patch.md` section 3.42, round 4.

Round 5 (2026-10-23, the owner: "yes, let's", to the camera flying at once instead of waiting about
200 ms for the window's player): no hold. The camera flies to the bench from the window's first
tick; the weapon appears above the bench when its player is made (until then the held item takes
the decal pose's transform, 10 m under the floor: neither in the hand nor at the bench). A refresh
keeps the bench's weapon as before; the close, the drag and the left button (with the table on)
are unchanged. Section 3.42, round 5.

Round 5b (2026-10-23, the owner: "the camera jumps a bit at the end of the transition"): the
camera's own side angle, height and distance steps moved at the setter's constant speeds and
stopped dead at 0.9 s while the point, yaw and pitch still glided, which then snapped 2 mm onto
their targets at 1.6 s. Now a flight sets those steps' speeds to 5 × the way left each tick, so all
seven ease out together, and the point, yaw and pitch land exactly (a least speed of 0.01 a second)
instead of snapping (round 5c moves the three itself). The bench and the wheel are unchanged; the close flies back the same way.
1 994 bytes. `check_weapon_table_026g.py` 42 checks (every tick's move no more than the one
before, no sudden stop, no snap, there and back). Section 3.42, round 5b.

Round 5c (2026-10-23, the owner: "same jump", and a per-frame camera log from a debug build): the
jump was the camera's own collision ray (0x284309 -> 0x2844D0). It runs from the camera's point back
to the eye and, at a hit, cuts the distance, which is then clamped to 0.6. The frame the window's
player was made, something it brings near the bench came into the ray's way: the distance went
1.34 -> 0.60 and the eye 0.80 m in one frame, then rested at 1.09 instead of 1.2. Now, while the
camera is the table's (flying there, at the bench, flying back), no ray is cast (a sixth entry, `ray`,
+19h). The flight also moves the camera's own side angle, height and distance itself, so the bench's
distance is exactly 1.2. 1 870 bytes. `check_weapon_table_026g.py` 44 checks (a ray hit at 0.6
from READY on changes nothing). Section 3.42, round 5c.

Round 5d (2026-10-23, the owner: "a bit too far", "off center to the right", "front and center",
"maybe we should not have fov affect weapon preview in lobby under bench?", "just like we did with
scopes").
- **Field of view.** The bench's view is at the original's 65 degrees whatever uro_fov: the camera's
  zoom (camera+94h, which patch 22's projection takes, as a scope's) glides over the flight to
  (uro_fov + 130) / (4 uro_fov − 65) and back to 1 at the close.
- **Centre.** The weapon's middle sits at the middle of the screen's part left of the upgrade window
  (0.270 of the width at 16:9 and 16:10, 0.329 at 21:9, from the lobby movie's layout), by a look-at
  point beside BENCH; the side angle is 0.
- **Distance.** As near as keeps its ends on the screen at any spin, for the weapon's own length
  (its render box): a rifle about 63% of that part at 16:9, a pistol at the 0.3 m least (patch 32's
  NEAR_MIN, was 0.6).
- **Ultra-wide screens.** Also as far as keeps the tilted ends 2% from the top and the bottom (it binds
  beyond 16:10).
- **A window over a weapon's.** The old weapon drawn through the rebuild is not measured; the new one
  is framed for itself.
- **Room.** 2 266 bytes, now at `.srpw`+6B0h. `check_weapon_table_026g.py` 48 checks. Section 3.42,
  round 5d.

Round 5e (2026-10-23, the owner: "a bit too much to the left"; the wheel "is not focused on the center
of the weapon").
- **Place.** The weapon's middle sits FRAME_SHIFT (365 lobby-movie units) left of the screen's middle:
  0.310 of the width at 16:9 and 16:10, 0.359 at 21:9, a little right of the middle of the part between
  the Squad strip and the upgrade window. Its ends stay 2% clear of the screen's edge, the window's edge,
  the top and the bottom at any spin.
- **Wheel.** The camera looks at the weapon's middle and a side angle places it, so the wheel zooms into
  the weapon: its middle stays put. The wheel's range is 0.3 m to max(1.25 d, d + 0.5 m) of the framed
  distance d; the close flies back from wherever it left the camera.
- **The review's fix.** While the flight eases, the wheel's far clamp leaves the distance to the glide:
  no snap when the weapon gets framed.
- **Room.** 2 382 bytes. `check_weapon_table_026g.py` 52 checks. Section 3.42, round 5e.

Round 6 (2026-10-29, the lobby scenes: each scene the player picks has its own table;
`../docs/notes/lobby-scenes.md`).
- **The spot from the scene.** The weapon's spot and the camera's view of it are no longer in the
  code. When a weapon's window opens, `camera` reads them from the lobby's project config:
  `srp_lobby/weapon` (a float3) and `srp_lobby/weapon_view` (radians). The path is the lobby menu, its
  project (+114h), `[[project+17Ch]+108h]`, as the engine reads `camera/position`. Each key goes
  through has_key 0xF6C00 before child 0xF6B10, and each type is checked. The classic lobby's values
  (4.25, 1.30, 3.05) and -pi/2 are data now: the content edit `lobby-scenes-1.json`.
- **No spot.** Without a full and well-typed `srp_lobby` the window is treated as an armour's: the
  stock camera, the weapon in the hands.
- **Any wall.** The bench's transform is turned about the vertical by R = weapon_view + pi/2 (rows
  rotY(spin) rotZ(tilt) rotY(R)), so the weapon lies across the camera's view. R 0 (the classic)
  skips the turn: its transform is bit for bit 5e's.
- **State.** 0xBC bytes: +A0h the spot (x, y, z, 1), +B0h the view, +B4h/+B8h cos R and sin R.
  They are written before TABLE is set.
- **Room.** 2 779 bytes, past `.srpw`'s first page. Patch 47's code moves to `.srpw`+1200h and its
  state to `.srpd`+FC0h.
- **Checked:** `check_weapon_table_026g.py` 56 checks. Its lobby menu has a project with a real
  binary config. New cases: four other spots and views, and eight refusals.

Round 7 (2026-10-30, the lobby audit; `../docs/notes/lobby-scenes.md`, "The camera flight").
- **The flight in Rust.** The camera's flight there and back is the Rust patch `weapon_flight`
  (`rust/patches/src/weapon_flight.rs`, one lohook at `lobby_camera.view`, 0x2840F2: the tick
  with its yaw and pitch final): the eye on a straight line between the two eyes (over a
  character's head where it comes within 0.9 m of one), the yaw, pitch,
  distance, side angle and zoom with it, one smoothstep over 0.45 s; back, to the stock camera's
  eye (the wheel's target cut by the camera's own ray there, `engine_lobby_camera_ray`), a tick
  held with no ray, then the stock camera from there. Until round 6 the NASM's glide went back
  to the wheel's target with no ray, out of a small room, and the stock ray snapped it back
  (up to 3.1 m).
- **The interface.** Patch 45's state: the NASM writes TABLE, the targets and REQUEST (+2Ch, +1 a
  request; EASE until round 6); the flight writes FLIGHT (+4: 0 stock, 1 moving, 2 resting; FLY
  until round 6), which the NASM's camera, tick and ray read. The NASM's glide is gone (2 515
  bytes); its ray clamps the wheel's target only.
- **Checked:** `cargo test` (the flight's machine), `check_weapon_table_026g.py` 29 checks with the
  game's tick (as is only: the flight is the arena's), the close in a room with no snap;
  selftest checks 45 and `test_weapon_flight`.
- **The captures' sweep** (`camera_sweep.rs`, diagnostics builds only, `--ragdoll-trace`, with
  `srp_camera_sweep.txt` in the user data folder): the camera at a script's poses, the squad's
  characters the account's own (`squad`), the scene without anyone in it (`empty`: the lobby's
  three characters drop their players, 0x284AA0), for static captures of every lobby.

### 46. On the death screen a drag turns the camera; the wheel does nothing (the next release's revision)

The owner (2026-10-21): "when player dies they can still move camera
around, with holding a mouse button? Scroll shouldn't work though". The
rules: `../docs/notes/death-camera.md`.

**The death screen today.** A death leaves the player's input handler in
the first person mode (`player::on_player_death` 0x1F28E0 changes no mode;
only the network clients' spectate and replay paths switch to 4 or 2):
`player::update_camera` (0x1F3480) gives the view's update (0x2422B0) the
eyes' transform (player+44Ch), which modes 0 and 4 copy as they are, so
the camera is the corpse's eyes and does not move. The hud shows the kits
and the cursor (0x2383B0) until the respawn (8 s in every mode's options),
so the game world's input handler (priority 7) takes every mouse move and
the wheel and answers 1: the player's handler (priority 10) gets none, and
in the first person mode it would not use the wheel anyway (only the
third-person and warmup steps add it to the distance). The game builds and
sends the local player's input only while it is alive (game_world::tick,
`cmp byte [eax+33Ch],0` at 0x1F7883): a dead player sends nothing.

**The patch** (builds with the late join; `death_camera_026g.asm`, 860
bytes, in a section of its own, `.srpc`, on the page after the late join's
state page, 0xA429000; its state, a yaw and a pitch, 8 bytes at the late
join's state page +E60h):

- `WARMUP_LOOK_HOOK`'s jump (the first ten bytes of the game world's
  on_mouse_move, 0x25D0E0) goes to `death_look` (`.srpc`+0) instead of
  patch 31's warmup_look. While the kits are shown (hud+2DFh), the player's
  handler is in the first person mode (+358h = 0) and the local player
  (game+53ACh, +4) is dead (+33Ch = 0): a mouse button held is a drag as
  patch 31's (its state and its drag_end: the lobby's buttons, right, 5th
  and 7th, at once; the left one past 8 units, so a click stays the kits';
  the hud's pointer off the screen during it, back where it began at the
  release). A drag's move turns the yaw by x and the pitch by y at the
  player's look's step (the option sensitivity, 0xA74AEC, x 0.1 x pi/180; the
  pitch also x height / width x 0.9549 (since patch 112 16:9's 0.5625 at
  every window while "Lock vertical sensitivity" is ticked; unticked the
  yaw's step at the vertical sensitivity), and inverted by
  mouse_invertion, 0x1F1D9D3), within 70 degrees to either side, 60 up and
  40 down (relative to the head, below), and is taken (1):
  neither the cursor's nor the player's handler's. The wheel in a drag is
  taken with it and changes nothing. Without a button the move, the wheel's
  too, goes on through warmup_look to the original: the cursor's. Every
  other move goes on to warmup_look as before.
- update_camera's `call 0x2422B0` (0x1F364B, `DEATH_VIEW_HOOK`) calls
  `death_view` (`.srpc`+200h): while that player is dead, the handler in
  the first person mode, the kits shown and the turn not 0, it runs the
  update's own third-person step (0x2422DE, after the update's own start)
  on the identity with the turn's yaw and pitch and the distance 0 (the
  handler's warmup yaw, pitch and distance, +34Ch, +350h, +354h, borrowed
  and put back): the turn L, the yaw about up and then the pitch about the
  turned right. The view is then L times the eyes' transform (each of L's
  rows x E.i + y E.j + z E.k), at the eyes' place: the look turned about the
  head's own up and right. A live player gets the original update and the
  turn is cleared.

**Relative to the head** (the owner, after the first game test,
2026-10-21: "the way the angle is set should be different for different
death poses, I think, sometimes you can see inside of models", then "Can
we maybe still orbit around eyes but take a pose into consideration?").
The first build turned the view about the world's up: with the head rolled
or lying on its side that swung the look into the corpse's own torso. The
eyes' transform (player+44Ch) is the head's own, every frame: base_player's
character head transform, the head bone's matrix times the body's
(`animation::calculated_head_matrix`, 0.10b; the use ray of
detect_usable_object starts at its place +47Ch along its forward +46Ch).
Turning by it and keeping the turn within 70 degrees either side, 60 up
and 40 down of the face's forward keeps the look in front of the face
whatever the pose, with no table of poses (the numbers to be tuned in the
game). The head itself is not drawn for the local player in the first
person mode (mt_draw gives set_head_visibility 0x1F37A0 a 0 every frame,
dead too: models 0, 5, 6, 7 hidden), so the eyes never see the face from
inside; the eye point is not moved. Not an orbit (the owner's call).

Camera only: the moves of a drag never reach the player's input handler,
and a dead player sends no input anyway. Nothing a match computes: no row
in `../crates/session/src/v026g/simulation_changes.tsv`; games with and
without it play together, replays are unchanged. A death before the start
(the warmup mode) keeps patch 31's orbit and wheel; spectators and replays
(another mode, no kits) are the original's. `../docs/notes/client_patch.md`
section 3.43.

Checked: `selftest_026g.py` check 46 (the site, the view's update and its
step, the mode a death leaves, the input a dead player does not send, the
handler's world, game, client and local player, the window's size and the
look's step as the game has them; the code's exits, calls, absolute
operands and returns) and the layout checks of check 15 (ten sections,
`.srpc` last); check 32 sees the mouse move's jump go to death_look.
`check_death_camera_026g.py` (Unicorn) runs mouse moves through the input
world's loop (the death screen's moves, wheel, click, right and left drags,
an inverted and a slow mouse, the pitch's and the yaw's limits; a live player, a
spectator, third person, no local player, a death before the start, the
kits hidden) and the view's update on a made-up rolled and tilted eye
transform (the build's view bit for bit the original's step on the identity
times the eyes' rows, float32 in the build's order; the look in the head's
frame the turn's alone; the warmup fields put back; a live player's turn
cleared): 122 runs, eight broken builds fail. `check_late_join_026g.py`, `check_kit_change_026g.py`
and `check_weapon_preview_026g.py` now find `.srp` and `.srpd` by their
names (`.srpc` is the last section); every `check_*_026g.py` passes.
Against the build of `main` ecb9916 the default build differs in the
headers, the two sites and the new section's 1 024 bytes (the certificate
moved behind them). **Not run in a game.**

### 47. A weapon wears a decal, placed on it above the workbench (the next release's revision)

The owner's request (2026-10-21): "implement and allow put two decals on weapons"; then the server gives a weapon one decal slot (0): the client keeps three upgrade records an item, and a scoped weapon holds its scope and camouflage besides (the owner, 2026-10-21: "let's remove one decal instead for now"); the code bakes up to two, as written. The rules and
the data: `../docs/notes/weapon-camo.md`, `../scripts/content-edit/weapon-decals-1.json` (every
weapon config with `data` gets `parameters/decal_available` and an empty `data/decals`, as
armour's).

`weapon_decals_026g.asm` at `.srpw`+1200h (3145 bytes, after patch 45's code; +1000h, the section's
second page, until patch 45's round 6), its state (12 bytes, the left button's press) at `.srpd`+FC0h
(+FA0h until then); it begins with seven `jmp near`, its entries:

- **lookup** (+0) and **fill** (+5), called by patch 44 (`weapon_camo_026g.asm`, -DDECALS): what a
  weapon's bake paints. lookup: the camouflage (`data/camouflage`, as patch 44 read it) and the
  decals (`data/decals`, where a decal upgrade appends {texture, low, high}, the record's u64
  put in by the game's config merge); patch 44 bakes when there is either. fill: the
  camouflage's layer of the parameters, and for each of the first two decals the layer P +
  i*270h: the texture, `_nmap`, `_fmap`, `_smap` and the float4x4 the editor unpacks from the u64
  (0x263D00), its point 1.0 lower (a weapon's y -1..1 in the u64's 0..2) and its two sides halved
  (a weapon's decals are half a character's); `_<texture>_<high><low>` after the baked files'
  name. A decal never placed (u64 0) goes on the left of the receiver, the second on the right.
- **pick** (+0Ah): the decal preview's `call 0x264960` at 0x265999 (5 bytes): for the weapon above
  the workbench (patch 45's table on, patch 32's weapon shown, the edit's host the window's item)
  the mouse's ray in the weapon model's space (the inverse of bone 1's skinning of the frame, read
  from the weapon's render instance, times the transform patch 45 publishes at `.srpd`+F50h; none
  before patch 45 has drawn the weapon at the bench) gives the point of
  it nearest the weapon's long axis and its direction reversed as the normal (the decal looks
  along the ray; its projector and occlusion find the surface); a ray behind the eye, more than
  0.25 m off the axis or beyond its ends misses. Else the original's pick on the character.
- **decode** (+0Fh): the preview's `call 0x263D00` at 0x2659B2 (5 bytes): the weapon's decal at
  the weapon's transform, so the preview's box is drawn on the weapon at the bench.
- **bake** (+14h): the editor's bake, 0x2641A0 (`push ebp / mov ebp,esp / and esp,-8`, 6 bytes)
  is `jmp bake` and a nop. For the weapon: the click picks, packs the u64 (0x264A10), accepts it
  (0x263950: the record, `46 05`) and tells the movie (`root.exit_decal_edit`); the start's and
  the cancel's bakes do nothing (the weapon keeps its textures). The server sends the account
  after a weapon's `46 05`, the lobby builds the weapon again and patch 44 bakes the new place.
- **start** (+19h): the start's `call 0x264CF0` at 0x263938 (5 bytes): on a weapon the decal's
  index in `data/decals` counts the records of slots 0 and 1 (a weapon's camouflage is in slot
  2); a record of another slot ends the edit with nothing queried.
- **click** (+1Eh): the character's mouse key handler, 0x265C20 (6 bytes, `jmp click` and a nop),
  which places the edit's decal on the left button's release: in a weapon's edit only when the
  cursor is within 5 pixels of where the button went down. A drag (patch 45's left button turns
  the weapon) places nothing; a release with no press seen (the movie's "change") neither.

Everything else, armour's decals, the squad panels and a match: the original's code. No base
relocation is under a site and no branch aims inside one; the code holds absolute addresses (the
exe is never rebased). The movie: a weapon's upgrade window shows one decal slot when it has
`decal_available` (`UpgradeWindow.as`, `WEAPON_DECALS`). Rendering and the lobby only: no row in
`../crates/session/src/v026g/simulation_changes.tsv`; a game without the patch shows the weapon
without its decals. `../docs/notes/client_patch.md` section 3.44.

Checked: `selftest_026g.py` check 47 (the sites in the original and the build, their callers,
nothing aiming inside, no relocation, the code's calls and branches, `.srpw`'s two pages) and
check 44; `check_weapon_decals_026g.py` (Unicorn, the cases in its header, with patch 45's own
transform: 202 checks) and
`check_weapon_camo_026g.py` (the camouflage alone as before). **Not run in a game**: the preview on
a weapon, the place the click gives and the bake of a decal on a weapon are read and emulated
only.

### 48. An upgrade dropped on a weapon in its window is shown at once (the next release's revision)

The owner's request (2026-10-21): "can equipments in general be more responsive? I want the scope
to be equipped and see the results a few frames after? Or camo?", and "the server can be in the
background". Where the time went (`../docs/notes/client_patch.md` section 3.45): the lobby client
sends its messages only at its own tick, every ~505 ms (the packet logs: a request leaves 0 to
505 ms after the drop, 250 ms on average); the server answers in 2-3 ms (locally; a network's
round trip live); then the character's player is made again; a camouflage's first bake is ~40 ms
of GPU and compression (the game's own log lines). Now the drop itself changes what the character
is built from; what is sent is not changed and the server's account stays the truth.

`upgrade_preview_026g.asm` at `.srpw`+2000h (1638 bytes, the section's third page), its state
(A0h bytes: the window's item, the queue of changes, the revision last given and its upgrades,
the last query's time and kit) at `.srpd`+D60h, after the patches' mount's and before patch 32's; seven `jmp near`, its entries:

- **relocate** (+0): relocate_upgrade_item's handler (0x29A8B0), `mov edx,[esi+4620h]` at
  0x29A9B2 (6 bytes, `call` and a nop), before it writes `46 01`; and **drop** (+5):
  drop_upgrade_item's (0x29A9D0), `mov ecx,[esi+4620h]` at 0x29AACF, before patch 30's `46 02`.
  For the window's weapon (patch 32's mode 1, its item the host, the lobby character's window
  open): the change is queued ({upgrade, the record's dict id, slot}, or "take off"), a log line
  is written ("srp: upgrade preview: item <host>, upgrade <id>, slot <n>") and setup_profile is
  called at once. A scope's record holds the variant made for the weapon, as the server's
  (`ScopeFits`): the first category-22 entry of the items dictionary whose config's
  `srp_scope_fit` names the scope and the weapon (by name or dict id); none: nothing is queued.
- **account** (+0Ah): the 0x87 reader's last call (`call 0x270320` at 0x270313): each account
  answers the oldest queued change (srp pushes the account after every `46 01` / `46 02`, a
  refusal too).
- **find** (+0Fh): patch 32's two finds (`weapon_preview_026g.asm`, -DFIND) call it; it calls the
  stash's find_kit_or_show, then, for the item a change was queued on and shown from the account's
  profile, builds the character from the stash's scratch: the account's profile, the queue
  applied to the weapon's record in order as the server applies it (a put leaves an identical
  record as it is, else takes the upgrade and what is in its slot off and adds it when there is
  room; a take-off takes it off), and a kit revision of its own (`0x80000000 | n`) that stays the
  same while the weapon's upgrades are the same, so the account confirming the change makes
  nothing again (setup_profile's early return).
- **again** (+14h): setup_profile past its early return, `cmp dword [ebx+0D2C4h],-1` at 0x284B89
  (7 bytes): for the lobby's own character, a player being made of the same kit's id and revision
  is what is asked: it returns (a call waiting for that player is dropped: this one is newer);
  with no player being made the query's start time and kit are kept.
- **made** (+19h): on_player_ready, `cmp byte [ebx+0D2C9h],0` at 0x284F7F (7 bytes): for the lobby's
  own character a log line "srp: lobby character's player made in <n> ms" (the lobby's clock,
  [menu+9Ch]+584Ch, since the query), then the original's comparison.
- **early** (+1Eh, 2026-10-23): setup_profile's early return, `cmp byte [ebx+0D2C8h],0` at
  0x284B7C (7 bytes, its last test before `jz 284CF8h`): for the lobby's own character it returns
  early only while no player is being made (+D2C4h -1); otherwise the call waits for that player
  (+D2C8h) and on_player_ready makes it next, as the original does for another kit. A call for
  what is being made still returns at `again`. Patch 45's round 4: an upgrade window closed while
  patch 32's scratch's player was being made showed the window's weapon afterwards.

The log lines go through the game's own function (`{0x9DAF4C | 1, 0, [0x1F1D614]}`, as every
game line's caller builds it; round 2: the first build passed an empty one and nothing was
written).

A refusal: the account comes back without the change, the weapon's upgrades differ from those
shown, the character is made again from the account's. Armour windows, the squad panels and a
match: the original's code. No base relocation is under a site and no branch aims inside one;
the code holds absolute addresses (the exe is never rebased). The lobby only: no row in
`../crates/session/src/v026g/simulation_changes.tsv`.

Checked: `selftest_026g.py` check 48 (the sites in the original and the build, the handlers'
registers, the 0x87 reader's last call, setup_profile's ways to 0x284B89, the code's calls and
branches, `.srpw`'s three pages); `check_upgrade_preview_026g.py` (Unicorn, the cases in its
header: the game's own setup_profile, profile copy and config lookups; the quick close through the
movie's two handlers and the game's own on_player_ready, with and without `early`). **Not run in
a game.**

### 49. The game stops at its start without a patch its release requires (the next release's revision)

The crashes of 2026-10-21 (`../docs/notes/required-patches.md`): players whose updater could not
download patch_26 and patch_27 started the revision 27 exe without the launcher; patch 27 skips a
missing number, so the game ran on older content than the server's and crashed in matches (an
unknown item id in a profile, other animation ids in a world snapshot). Now each release's exe
knows the patches of its release and does not start without them.

In `mount_patches_026g.asm` (patch 27), the probe's missing case (`CreateFileA` fails with 2, file
not found, or 3, path not found) tests n in a bitmap of 32 bytes (`needed`, bit n of byte n/8,
`bt [edx+needed],ecx`): a clear bit is a number skipped, as before; a set bit goes on to the same
fatal call as a file that cannot be opened, before anything is mounted: "Cannot open
resources.db.patch_<n> (error 2). Please run the Skhid launcher again." (error 3 for a missing
folder on the way), the lowest such n. The code grows from 707 to 759 bytes at `.srp`+400h (before
patch 30's +700h); nothing else changes and no new site.

The bitmap sits right after the "Cannot open" text, before the state's address at the .bin's end.
The checked-in .bin holds zeros: a build without the option requires no patch (local and test
builds). `patch_026g.py --required-patches=12,14,17,...` (and `make_client_026g.py`, passing it on)
writes it into the build's copy; only those bytes and the PE checksum differ. `release/build-zip`
passes the numbers of every row of the offered-patch list (`release/patches.tsv`; `--test`:
`patches-test.tsv`), which `release/release.sh` gives it with `--patches` after `upload-patch`
added the release's new patch: one command, and each release's exe requires exactly what its
server offers. `build-zip` reads both exes back (`lib.sh`'s `exe_required_patches`, which finds the
text and reads the 32 bytes after it); `release.sh` and `release/deploy` refuse a ZIP whose exe
requires other patches than the server data offers, or none (an exe from before patch 49).

With the launcher nothing changes: its updater fetches the listed patches before PLAY is on. A
game started some other way without a patch now stops at once with the message, before any
match. A patch is never removed from a player's folder by this; one the list no longer offers is
simply not required by the next release's exe (and still mounted when it is there). The lobby
only (start-up): no row in `../crates/session/src/v026g/simulation_changes.tsv`.

Checked: `selftest_026g.py` check 49 (the default build's bitmap zeros; the `bt` and its way to the
fatal; against the build without the option only the bitmap and the checksum differ; the list
read back by `patch_026g.py` and by `release/lib.sh`; bad lists refused) and check 27 (the texts,
the ways out); `check_mount_patches_026g.py` (Unicorn, as is and rebased: a build requiring 1,
12, 14, 17 and 255 on folders with all of them, with more, with a required one missing, path not
found or a folder by its name; revision 27's list without 26 and 27, the case of 2026-10-21,
stops at patch 26); `release/test.sh` (the stand-in build-zip given the list, the new patch
required after `--resources`, a ZIP that requires fewer refused before the swap, `deploy`'s
refusals). **Not run in a game.**

### 52. A key in the lobby no longer kills the game when the focus is on a dead object (the next release's revision)

The owner's local game died on 2026-10-22 at 04:44 in the lobby, about a second after an upgrade
(the second of two upgrades five seconds apart): `ACCESS_VIOLATION` reading 0x14 at
0x77200B. The crash report's own frames (0xFFFFFBD8, 0x786310, 0) are not the stack; the dump's
stack is: lobby_menu's key handler (0x26CB80) invokes `root.has_active_window` (0x26CBF9, through
0x511600 and the AS3 VM), `LobbyMenu.has_active_window` reads `stage.focus is DropdownMenu`, and
the Stage focus getter's thunk (0x7750F0, member pointer 0xAFF180, ThunkInfo "focus" at
0x8F1E40) calls the getter 0x771FA0 (return address 0x775125 on the stack).

The getter is Scaleform's `Instances::fl_display::Stage::focusGet`: it locks MovieImpl's focused
InteractiveObject (the weak pointer of `FocusGroups[FocusGroupIndexes[0]]`), takes its
AvmInteractiveObj and that one's ActionScript object (`GetAS3Obj`: the raw pointer, else the
collectible one) and reads the object's traits type for an InteractiveObject, without testing
the object. Scaleform's own `FocusManager.getFocus` tests it (`if (ido && ...)`), and so do the
key dispatch (`MovieRoot::OnKeyDown`) and the focus events (`HasAS3Obj`). The display object
outlives its ActionScript object when the collector frees that object (`~DisplayObject` calls
`AvmDisplayObj::ReleaseAS3Obj`), and AS3's `removeChild` leaves the focus where it is (only an
unload resets it, `MovieImpl::ResetFocusForChar`): a focused object that was taken off the
stage, and then collected while something still held its display object, is such a one. The dump
holds the stacks only, so which object it was is not in it; neither patch 52 nor the lobby needs
to know. At the crash esi (the focused object) was 0x0D4386A0, eax and ecx 0.

**The patch.** In every full build (with and without the late join, and
`--cave=hash-mismatch`), the 46 bytes from 0x771FED (`test eax,eax` after the interface call) to
0x77201B (the result's assignment) become the same reads with both tests: no AvmInteractiveObj,
or no ActionScript object (both pointers 0, the collectible one with only its bit 0), goes to
0x772031, the null result a focused object of another type already got. `stage.focus` is then
null, as it is in Flash for an object no longer on the stage, and the key goes on as with no
focus (the key dispatch already skipped such an object). ecx is the only register used, as
before; at 0x77201B eax is the object, as before. Every jump of the old bytes stayed inside them,
nothing else aims inside, no base relocation is under them. The lobby and the hud only: no row
in `../crates/session/src/v026g/simulation_changes.tsv`.

Checked: `selftest_026g.py` check 52 (the getter before and after, its member pointer and
ThunkInfo, the key handler's invoke, no relocation and no branch inside; cert-only keeps it);
`check_stage_focus_026g.py` (Unicorn: focused InteractiveObjects by either pointer given by
both; another type and no focus null in both; no ActionScript object, and an interface that
gives none, fault in the original at 0x77200B and 0x771FF8 and give null in the build; every
full build the same). **Not run in a game.**

The 2026-10-27 PIN-entry crash exposes the same missing receiver in a second path:
`AvmTextField::OnCharEvent` (0x79D360) calls `EventDispatcher::WillTrigger` (0x74A860)
with no AS3 object, reaching the null read at 0x74A800. The Rust `text_input` module
extends patch 52 with a lohook at 0x79D369. Its borrowed `DetachedTextInput` handle
recognizes the raw/collectible receiver exactly as the native code does; only a
missing receiver can set BL to false and leave through the native epilogue at
0x79D4AD. The editor then ignores this stale character. Attached fields retain the
original dispatch and insertion behavior, including tagged collectible pointers.

`RoomsWindow` owns focus transfer across its redraw: clear focus before removing
an input, then restore the text selection only when the replacement view has an
input. Joining a room and hiding the window release focus. `RoomsCreate` no longer
tries to restore focus independently after its redraw callback.

Checked: `check_text_input_026g.py` reproduces the original null read and verifies
normal and detached events, Cyrillic/digit/control characters, both receiver fields,
tagged pointers, registers and stack. The Rooms Ruffle harness verifies focus is
released before removal, restored with its selection after redraw, and released on
hide. Native in-game retesting is still required.

### 53. Research, mode 5 (the next release's revision)

Research as 0.27d shipped it (`../docs/notes/research_mode.md`: the rules, the data, the inventory
of the builds; the owner, 2026-10-22: "will go into the next one", not revision 28). 0.26g has none
of 0.27d's capture code and its rule and world-object layouts drifted (the rule base grew 78h, the
world objects were reworked), so the rule is implemented in Rust on 0.26g's interfaces, with 0.27d's
disassembly as the spec, and the units are not world objects: the rule tests the players'
positions against the units' shapes, which the mode config carries (`capture_zones`).

- **The rule** (`research_arena`, `Rule`): game_match_rule_base (0x313900, type 5) and
  0.26g's twenty slots: 7 events (`(type, data, time)`, as the world's dispatcher 0x2D1EA0 passes
  them; a kill, `{killer, victim}`: "kill invader"; the first build took `(type, time, data)`, fixed
  after #297's review), 8 and 9 a roster
  place entered or left, 11 the world (its tick a delegate at the end of the world's second tick
  list, numbered by the world's counter, as timelimit_rule_core's 0x318050), 12 the blackboard, 15
  and 16 the state (21 + 96 bytes a unit, little-endian, `rr_write`'s layout; 15 writes it field
  by field through the game's writers, as tdm does, so the hash mismatch dump names each field;
  16 takes the reader and a dword it does not read and pops both, `ret 8`, as the game's rules
  do: the first build's `ret 4` crashed both clients at their first synchronization,
  `../docs/notes/client_patch.md` §3.50), 17 the hud's team points, 18 completed (the end time);
  the rest the game's (0x266710 the delete, 0xF1900.. the resource's, 0x8FE10 the priority,
  0x316540 the link resolver). Its vtables carry game_match_rule_base's RTTI locators.
- **The cook**: a client rule cook as tdm_rule_core's (0x2F41E0, 0x2063F0: class at +8, the thread
  at +14h, 8 at +18h, the game at +20h), class 0x1E0 (`RESEARCH_CLASS`; register_cook's table has
  0x206 places and nothing registers 0x1E0; a taken class is left alone and Research is not
  registered), its create reading `mode.parameters` through the game's own look-ups (0xF6B10,
  0xF6C00). The mode configs list it: `rules [93, 91, 480, 94]`. Its vtable carries an RTTI
  locator of its own, `research_rule_cook`: a translate_query_cook, shaped as tdm_rule_core_cook's
  hierarchy (its base class descriptors reused), not tdm_rule_cook's (the first build's).
- **The tick** (`research-rule`): only in play (the time rule's state 3, world+18FE4h);
  the players of the places entered, alive (+33Ch) and in the world (+33Fh), their team (+6B8h ->
  +45ECh), feet (+43Ch, the player's transform) and capsule (half its height from the character
  controller, `[[+494h]]` +1D0h/+1F0h, as get_transform 0x7F5C0 takes it; 0.38 m round): inside a
  unit when the capsule standing on the feet overlaps one of its shapes (the first build took the
  feet for the capsule's centre: a floor up to 0.9 m over a unit took it); the units' capture (0.27d's 0x324070), the
  teams' points (0x31EEC0), the end 3 s after 100; the shares of a capture (0.27d's water-fill,
  0x31E920) counted in the statistics' events 6 and 18 (hundredths), a kill invader in event 10;
  the statistics' tail: `m_team_points` the scores, `m_winner_team` the leader.
- **The hud**: `cmp dword [edi+20h],4 / jne 0x23B3FC` at 0x23B3DE of game_world_ui::update_ui
  becomes a `call` of the hud stub: in mode 5 `research_update_hud` sends `root.research_update` (numbers
  only) at each change and every 60 frames, and the units' messages through the hud's
  parameterised message (0x2375C0); then the compare and its branch. `cmp eax,4 / jne 0x23B36A` at
  0x23B351 (the match's first message by mode) becomes a `jmp` of the welcome stub: mode 5's is
  `st_start_match_welcome_message_5`.
- **The section** `.srpx` (0xA42D000, after `.srpw`'s three pages) is the arena of the later
  patches (the twelfth and last section table entry; "The arena" above, client_patch.md 0.3).
  Research has its first two blocks: `research.code` (the 256 bytes of `research_026g.asm`,
  four `jmp near`: research_register, research_hud, research_welcome and research_icons;
  then the Rust code and its constants) and `research.state` (`rn_state`: the cook, the hud's
  cache; zeroed), linked from source (addresses recorded in `arena_026g.lock.json`). The next blocks are patch
  54's, the engine library's and patch 58's.
- **The register hook**: `mov eax,ebx / call 0x1EC170` at 0x1E9B4C of the game's constructor (after
  its own client rule cooks) becomes a `call` of the register stub, which registers the cook once
  and does the two instructions.

The Research-only Rust port comes from `feat/rust-port` commit `195bad01`; other patches
keep their current-main implementations. The port retains the rule layout, wire format and
integer simulation. `arena_rt` supplies the no_std memory intrinsics; NASM aligns every
Rust entry to 16 bytes and preserves the game's calling convention.

The additional icons hook wraps `update_player_icons` at 0x23B207 and sends projected
world markers from Rust. The top HUD has no capture caption; its occupied point grows by 34/24.
World markers grow from 24 to 34 px. Both displays use the retail crossed-swords artwork with a two-second
elapsed-time pulse. World markers have alpha 0.55 and fill alpha 0.12, under player icons.
Their parent fades out in 200 ms and back in 300 ms while the viewed player aims (iron
sights or scope); the separate top HUD stays visible. See `research_mode.md` for the
camera checks and payload. This presentation change does not change the simulation.

A simulation change for mode 5 only: modes 2 to 4 compute what they did; no exe without the patch
loads a mode-5 config (the class has no cook there), so a server offers Research only to games
with it. No base relocation is under a site and no branch aims inside one.

Checks: `cargo test -p research-rule` (the rules: captures by 1 and 2 players, contested, the fall
back, draining and taking, the owner refilling, the water-fill, the points and the end, kill
invader, a place that left, the wire and its refusals); `check_research_026g.py` (Unicorn: the
constructor's hook registers the cook once and goes on as the original, a taken class; the cook
makes the rule from a mode config with the game's look-ups; set_game_world_core links the tick;
a capture at 20 s and its event, nothing outside play, the points in the statistics' tail, kill
invader, serialize (rr_write's bytes through the game's writers; with the hash mismatch dump on,
the 88 fields by name) and deserialize (two arguments, 8 bytes popped), the game's own serialize
and deserialize loops over a world's rules with esp as before, the hud's points (the alive counts
left), completed; update_ui in mode 5 and 4, the messages; the first message by mode);
`selftest_026g.py` check 53 (also: every slot of ours pops what tdm_rule_core's and
tdm_rule_cook's in its place pop; the cook's RTTI hierarchy). The first local test (2026-10-22)
crashed at the first synchronization (slot 16, fixed) on a scrambled level (pack 1's vectors,
fixed in pack 2). **Not yet run in a game since.**
### 54. The artefacts: Spring's aim time and no slow from bullets, Lifebone's heal, one press for every artefact of a kind (the next release's revision)

The owner's decision (2026-10-22): the three combat artefacts buffed, best one of each, a stack
of one kind still adding up. The rules: `../docs/notes/artefacts.md`. The numbers are in the
configs (`../scripts/content-edit/artefacts.tsv`: Spring's passive speed, the two new
`aim_time_mod` keys and `active/stopping_power_mod`, Lifebone's regeneration delay); the patch
does what no config of 0.26g could.

- **Spring's aim time.** Its loader 0x33ACF0 reads a fixed set of keys into five character
  modifier nodes (passive: movement speed, max_carried_weight, 0 in retail; active: movement
  speed, stamina regeneration and spending) and the duration; none is aim time. At 0x33AE13 (its
  active's third node) `artefacts_026g.asm` makes the passive's second node aim_time (49) with
  `passive/aim_time_mod`, and fills a sixth node at +3B8h, aim_time with `active/aim_time_mod`
  (and a seventh at +3C8h, below). The two are after a Spring's own fields: its two sizes
  (artefact<artefact_spring_core> 0x3B8 at 0x25E7A8, the bare core 0x398 at 0x2EEA74) become
  0x3D8. The active's link (0x311BC0) and unlink (0x311DA0) take them in and out with the
  three others, so they are linked
  exactly while they are (a deserialized active Spring links it through 0x311BC0 too). The
  getter (0x2D5AE0) sums a list's nodes: a stack of Springs adds up.
- **No slow from bullets while a Spring is active.** A hit's stopping power slows its target:
  the player's stagger (0x2DBCD0, every tick) sums the stopping power of the hits of the last
  0.5 s and makes -min(sum, 0.5) the player's own movement_speed node (+2F4h); nothing scaled
  it. At 0x2DBD89 the sum becomes M42(sum), the getter on list 42 (artefact_synergy, read by no
  code of 0.26g and set by no config): times max(0, 1 + its nodes). A Spring's active has a
  seventh node there, +3C8h with `active/stopping_power_mod` (-1), linked and unlinked as the
  aim node; the object is 0x3D8 bytes. A hit's stamina damage is not changed.
- **Lifebone's heal.** Its enable_active (0x312250) removed the hand and leg injuries only. After
  them (0x3122AA, where the time is still its argument) the pools `pain`, `body` and `head` are
  found by name (0x2E58B0) and filled with increase_health (0x32A420, Hmax - H), as a medkit
  heals a pool (0x33D9C3).
- **One press, every artefact of a kind.** artefact_base::action (0x313300) activated the
  artefact pressed only, when it was in its passive with no cooldown. At its entry a key-down
  now activates the pressed one and every other artefact of the same class (vtable) and kind 1
  in the inventory's artefact slots (13..18) that is ready, each as action does (enable_active,
  its effect 0x313350, the time, state 4), and returns: a press of one cooling or active still
  fires the ready ones. A key-up goes on in action. Different kinds keep their keys. Ready
  includes patch 109's cap (2026-11-07, its entry `artefact.fire`): a copy past the first 4 of
  its kind in the containers is not fired and stays ready, so a press fires at most 4.
- **Onyx: a stack is a longer burst, not a stronger one** (the owner, 2026-10-25; its numbers,
  -75 % for 3 s, in `artefacts.tsv`). Its enable_active (0x3128C0) sets the active's time left
  (+F14h, ms) from `active/duration`; the press above, when the kind's enable_active is Onyx's,
  first counts the ready ones it fires (n) and adds (n - 1) x 1000 ms to each fired one's +F14h,
  so they are active 3 s + (n - 1) s and end and cool down together. Each Onyx protects its
  parts with its own hit callback (0x312BF0, one per protected part in the pool's list): in its
  passive (state 3) damage and piercing x `passive/*_mul`, else damage - min(absorb_total,
  damage - damage x `active/damage_mul`) and piercing x `active/armor_piercing_mul`; n active
  Onyx multiplied a hit n times. At 0x312C03, its state test, an active Onyx that is not the
  first active Onyx (its class) of the slots 13..18 returns (`ret 0Ch`) and leaves the hit as it
  is: n active Onyx reduce it once. The passive and an Onyx in no slot go on as before.
- **Onyx's pop-up.** Onyx's client update (0x25F640, slot 0 of its wrapper's vtable 0xA06118,
  ebx = the Onyx +F20h) shows the activation hint (+2F0h, a localization key, fixed text) with
  set_parameterized_message 0x2375C0 (size 21h, shift 14h, 3000 ms) when it sees the state turn
  4 for the local player's Onyx: once for each Onyx, all the same text. At 0x25F9DD (`mov ecx,
  [ebx-0C30h] / push 0BB8h`) the time on screen becomes the active's time left (+F14h), and of
  the Onyx fired by one press (state 4, the same activation time +2A0h) only the first of the
  slots shows it. The hud movie's set_parameterized_message writes a text's `%s` as that time in
  seconds (rounded up); all language hints say "75% for %s s" (`localization/*.tsv`). Not `{s}`:
  the game's localizer (0x1FEF90) makes `{NAME}` the key bound to action NAME (`{USE}`) and any
  other name nothing (the owner's test of `{s}`: "for  s.").

The code is `artefacts_026g.asm` in the arena (`.srpx`, "The arena" above): its block
`artefacts.code`, a code page after Research's two (0xA431000 today, as linked): eight `jmp near`
entries, global symbols the hooks jump to (artefacts_loader ... artefacts_popup), 1088 bytes in
all. No base relocation is under a site and nothing aims inside one. Only in the builds with the
late join.

**Call shapes.** The eight sites are declared in `INSTALLS`: Spring's active link 0x311BC0, its
unlink 0x311DA0 and artefact_base::action 0x313300 are ENTRY (their first bytes jump to ours;
ours goes on in the function or, for a key-down, returns `ret 8` itself: each pops what the
function does, 0, 4 and 8, checked by the self-test against the original and `abi.tsv`); the
loader's 0x33AE13, Lifebone's 0x3122AA and the stagger's 0x2DBD89 are JUMP (from the middle of
the game's code, back at the same stack depth). The patch hands the game no pointer: no
callback, no vtable. Each hook's arguments were read against the game's callers: the loader's
registers (0x33AE0B), the link's eax (0x311946, 0x311A4F), the unlink's pushed Spring
(0x311973), Lifebone's time at [esp+14h] with its four pushes, action's (pressed, time) as
inventory::action 0x2FCAA0 pushes them for its vtable +2Ch call, the effect 0x313350's (the
artefact, the time), increase_health's (time, amount) as the medkit at 0x33D9C3, the getter's
(B, k) as 0x3118D0 passes them, and the stagger's sum at [esp+14h] (its argument slot) on both
ways to 0x2DBD89.

**Revisions.** It changes what a match computes (every game runs every press and every
modifier), so a server asks for the release's revision, and
`../crates/session/src/v026g/simulation_changes.tsv` has its `next` row. The exe reads the two
`aim_time_mod` keys: it goes with the edit list's resources.db patch (patch 49 makes the release's
exe require it). `../docs/notes/client_patch.md` section 3.51 has the proof.

Checked: `selftest_026g.py` check 54 (the six sites and the two sizes before and after, no
relocation or branch inside, the code, its exits and calls, its texts), the arena's layout
(`artefacts.code` at 0xA431000) and the call shapes (the three ENTRY sites' `ret`);
`check_artefacts_026g.py` (Unicorn, the game's own code: the loader with the config lookup on
a binary config of the release's numbers, both Springs' links with the getter: aim time x0.95
passive, x0.85 active, x0.7 for two active Springs, speed x1.05 / x1.15 / x1.3; Lifebone's
enable_active with made-up pools: pain, body and head full, a limb, a full pool and a missing
one left alone; an
inventory of two Springs, an Onyx, a Spring cooling and one active: one press fires the ready
Springs alone, the Onyx alone, a key-up nothing, a cooling Spring's key the ready one; the stagger with made-up hits: -0.45, -0.5 at
most, an old hit out, and 0 with an active Spring in the build; through the game's own callers, a
slot's key through inventory::action 0x2FCAA0 fires both Springs, whose own enable_active
0x3118D0 links their nodes (aim x0.7, speed x1.3) and whose disable_active 0x311970 unlinks one's;
Onyx with the game's enable_active, countdown 0x3127E0, end test 0x312A50 and hit callback:
1, 2 and 3 Onyx pressed once are active 3, 4 and 5 s (the original: the pressed one, 3 s), a hit
of damage 1 and piercing 0.5 is 0.4 and 0.2 through all their callbacks (the original 0.4^n),
after 3 s one has ended and two or three have 1 or 2 s left; a cooling Onyx beside a ready one:
the ready one alone, 3 s, x0.95 x0.25; two Onyx active from two presses: x0.25 once; the end of
Onyx's client update from 0x25F9DD with set_parameterized_message stubbed: the hint once, 3000,
4000 and 5000 ms on screen (the original: once for each Onyx, 3000 ms), and once for each of two
presses; 80 / 79 checks). The edited retail Spring config was read by
the build's loader too (aim -0.05 / -0.1, stopping power -1). Against the build of `main`
5e03028, the default build differs in the eight sites (0x25E7A9, 0x2EEA75, 0x2DBD89,
0x311BC0, 0x311DA0, 0x3122AA, 0x313300, 0x33AE13), `.srpx` one page longer (the artefacts'
page at 0xA431000, its first four pages as they were) and the headers that follow (compared
section by section). **Not run in a game.**

### 58. The keys 1, 2 and 3 pick a kit; a match chat option (the next release's revision)

A player's asks (2026-10-21, board T29): "before respawn, add key shortcuts for change loadouts
1 2 3. i sometimes missclick", "option to turn off in-game chat". The rules:
`../docs/notes/match-hud-options.md`.

**The keys.** A key in a match reaches the game world's `on_keyboard_action` (0x25CD00,
slot 0 of its input vtable 0x9FC588; the input world, the DirectInput key, the kind 1 down, 2
up, 3 held) before the player's handler. The hud movie gets no key there (only the chat's
handler hands keys to it, while the chat is typed in), so the movie cannot see 1, 2 and 3 by
itself. The kits are shown by the hud (`root.show_profiles_selector`, 0x2383B0) together with
the cursor (`show_cursor` 0x23A930, the hud's +2DCh, the handler's +2E0h): on the death screen
and before the start (patch 31). After its Tab test the function asks the kind at 0x25CD41
(`cmp dword [ebp+10h],1 / jnz near 0x25CF0D`, edi = the handler); the patch jumps there to
`match_keys_keyboard`, which for a key 1, 2 or 3 (DIK 2..4) going down while the cursor is
shown, and was at the input tick before (below), invokes the hud movie's `root.select_kit_key(0, 1 or 2)` (an unsigned int, as the hud's
other calls on `[[[hud+8]+108h]+4]`), then goes on at 0x25CD4B (or 0x25CF0D for any other
kind) as the original: the key is not taken, the player's handler gets it as before (a dead
player sends no input, 0x1F7883). The movie (`HUD.select_kit_key`) does what a click on that
kit's tab does (`ProfileSelector.pickByKey`: `tabPage.selectedIndex`, whose change calls
`profile_changed` as a click's does), and nothing when the kits are not shown, the chat is
being typed in (between `focus_chat(true)` and `focus_chat(false)`, or the focus on its input), the kit cannot be played (`enabled`, as its greyed tab) or is the one chosen.

**A key pressed before the kits came picks nothing** (2026-10-25; the owner: "I often press 3
mid-fight to heal with the medkit, I die, and I swap to my 3rd kit"). Every `game::tick` runs
the input world's tick (0x3B7BD0) before the game world's update, which shows the kits when the
player dies. The input tick polls the keyboard (DirectInput's buffered events since the poll
before), calls every input handler's slot +14h, then hands the keys out: each event a kind 1
(down) or 2 (up), then a kind 3 for every key held. So the first tick on the death screen
handed out, with the cursor shown, a 3 pressed in play in the frame of the death, and the kit
was picked and sent (0xDE) at once. The patch points the game world's handler's slot +14h
(0x9FC59C, `ret 4` at 0x75AF0) at `match_keys_poll`, which after each poll sets the kits'
screen's state `kit_keys` from the cursor: CLOSED (not shown), OPENING (shown, not at the poll
before: a key of this poll may have gone down before the kits came), OPEN (shown at both).
`match_keys_keyboard` picks only in OPEN. A key held from play into the kits makes no kind 1, and
one pressed in their frame is OPENING's: either picks nothing until it is let go and pressed
again on the kits. And, the owner's rule after that: a player still pressing 3 to heal just
after the death presses on the kits before seeing them, so the keys pick nothing for
`KIT_KEYS_DELAY`, 300 ms, after the kits appear. OPENING carries the time of the poll that
first saw the kits (`kit_keys_since`, GetTickCount through its import 0x870090), and a poll
moves it to OPEN only 300 ms or more after it (unsigned, over the wrap); CLOSED clears it. A
key pressed in those 300 ms and held into OPEN picks nothing until let go and pressed again.

**The chat option.** The gameplay options' option 4 (`options_item_bool` named
"g_private_messages_in_game", 0xA064FC; the checkbox `_no_in_batle` of `main_menu.swf`,
labelled "Disable private messages at the time of battle") names no console variable: the
options' constructor finds no command of that name (0x260E90), the timeline greys the checkbox,
and nothing reads it. The game world's constructor calls the hud's at 0x1F6845 (`game::game`
makes the game world before it queries the configs, so before `startup.cfg` and `user.cfg` are
run and before the options look their variables up); the patch calls `match_keys_hud` there,
which calls the hud's constructor and then registers, once, a `cc_bool` of that name on a byte
of its own, saved in `user.cfg` (`cc_value<bool>` 0x1DDB0 through the engine library, then the
cc_bool vtable 0x9E7C4C, as the hud registers its own `show_quests_progress` at 0x2339C9). So
the option is live and kept. The main menu enables its checkbox (where its timeline makes it,
`settingsTabs_15`, and in `Settings.create_content`);
`localization/*.tsv` labels it "Hide chat in match" in all language tables (Chat im Kampf
ausblenden, Ukryj czat na czas rozgrywki, Скрыть чат во время боя, Приховати чат під час бою).
A received line is given to `chat_handler::add_message` (0x208AD0, stdcall: the handler, the
channel, the body, the name, the tag; `ret 14h`) by `process_incoming_text_message` at
0x26B916; the patch calls `match_keys_chat` there, which with the option on, in a match (the
handler's +0Dh, `set_mode`'s flag) and on a channel the hud shows (3 clan, 5 match, 6 and 7 the
teams, 8 squad, 9) returns as add_message does without it; any other line goes to add_message.
Kept: the server's lines (0, 2), the general and private channels (1, 4: in a match
add_message gives them to the lobby's movie), the player's own lines (the local echo, another
call), and every line in the lobby.

All four in the arena's blocks `match_keys.code` and `match_keys.state`
(`match_keys_026g.asm`), with the late join. No address and no base relocation under the sites
(0x25CD41 ten bytes, 0x1F6845 and 0x26B916 a `call` each); nothing aims inside them. The slot
0x9FC59C is a vtable's pointer in `.rdata`, under its base relocation as before.

Rendering and input only: a kit choice goes as a click's; no row in
`../crates/session/src/v026g/simulation_changes.tsv`. `../docs/notes/client_patch.md` section
3.53.

Checked: `selftest_026g.py` check 58 (the three sites before and after, the code around them,
the option's name used once, the block's code, exits, calls and data, the movies' and the
manifest's parts, the input tick's order and the keyboard's kinds, the slot) and the call shapes
(0x1F6845 and 0x26B916 CALLEE, `ret 4` and `ret 14h` as their models; 0x9FC59C SLOT, `ret 4`;
0x25CD41 JUMP); `check_match_keys_026g.py` (Unicorn): the poll from each state, the kits shown
and not, GetTickCount a stub (CLOSED, OPENING, OPEN at 300 ms and not at 299, over the wrap
too; `ret 4`, ebx esi edi ebp kept); the keys from 0x25CD41 for
the keys 1 to 5 and Enter, each kind, with and without the cursor and the movie, in each state:
Invoke with `root.select_kit_key` and the kit exactly for 1, 2 and 3 going down in OPEN with the
cursor and the movie, and every case at the original's 0x25CD4B or 0x25CF0D with its registers,
frame and stack; the input ticks, the game's own tick (0x3B7BD0) and keyboard process
(0x3B9AF0) over a made-up input world, for the keys 1 to 4, GetTickCount from 1000 and from
before its wrap: pressed 100 and 250 ms after the kits appear (none), 350 ms after (a pick),
pressed at 100 ms and held into OPEN (none until let go and pressed again), held from play into
the kits, pressed in the frame they came, in play, a respawn then a death: a kit only for a
press handed out 300 ms or more after the kits came, the game's handler given the
same keys and kinds as by the original exe (which picks nothing: main's build, run the same way,
picks kit 3 for a 3 pressed or tapped in the frame of the death); the variable: the hud's constructor called with the game world and the hud, its
registers back, one `cc_bool` "g_private_messages_in_game" at the list's head over two calls,
found by the game's lookup (0xEB7C0) and set and cleared by the game's execute (0xEB980) as a
`user.cfg` line; the chat: every channel with the option off and on, in and out of a match,
dropped exactly for 3 and 5 to 9 on, in a match, else given to add_message with the site's
arguments, the stack and ebx, esi, edi, ebp as before. **Not run in a game.**

### 62. One key hides the interface, in a match and in the lobby (the next release's revision)

A player's ask through the owner (2026-10-23): a key that hides the whole interface, in the lobby
too, for screenshots. The rules: `../docs/notes/hide-ui-key.md`.

**The key.** The game's own action `toggle_replay_ui` (19h; the action table's entry 0xA74CA8,
kinds 5, default F11), which in the original shows and hides the replay viewer's controls and
does nothing in a match or the lobby. No new action: the action table (0xA74AF0..0xA74FA0, its
end the key names' start, read by 15 sites) is a fixed table. The game has no screenshot key (no
such action, command or string): players take them with Steam (F12), Windows or other tools; F11
is none of theirs.

**The controls options' row** (the owner, 2026-10-24: "there is no in settings option to switch
it"). The options' list of keys is a table of its own, 28h static rows of 68h bytes at 0xA9DB28
(the action, its label's localization key, the group: 2 game, 0 lobby, 1 chat; the type; two
buffer_strings, the key being set and the key kept, each three pointers into its own 32-byte
buffer), read by six functions of game_options, each counting 28h rows: 0x201150 (a key's
conflicts), 0x201248 (the conflict message), 0x2013BF (a key set: kept in the row of its action),
0x201A19 (the rows sent to the movie, `root.set_keybindings`), 0x201D05 (the keys read from the
binder, `root.set_keybind`) and 0x201F2B (accept: `bind <action> <key>` or `unbind <action>`
through the console for each row whose key changed). The movie only names an action
(`start_bind_key`), so a row for 19h cannot be the movie's alone: a key set for an action without
a row is kept nowhere (0x2013A0) and never applied. The patch links the table again, its 40 rows
and a 41st (19h, `st_hide_interface_action`, group 2, type 1), as `controls` in the arena's block
`hide_ui.data`, and points the six functions at it (their immediates) with counts of 29h. Saving
needs nothing more: the binder's own `bind` command saves every bound action of its 48h bindings
by name (0x207450) and reads them back (`bind_key` 0x207DC0). The label is content edit
`localization/*.tsv` ("Hide interface", Скрыть интерфейс, Сховати інтерфейс, Oberfläche
ausblenden, Ukryj interfejs). The default stays the action table's F11 (the options' reset sets
the binder's defaults).

**The match.** `on_keyboard_action` (0x25CD00) asks the binder for kind 1 (0x25CD5F) and switches
on actions 11h..19h (0x25CE1C, the table 0x25CF30); case 19h at 0x25CED1 (`mov esi,[esp+10h] /
cmp dword [esi+18h],0 / jz 0x25CF0D`) does nothing without the replay's movie (the handler's
+18h, the hud's +14h). The patch jumps there to `hide_ui_match`: with the replay's movie it goes
on at 0x25CEDB as before; without it, the hud movie ([[hud+8]+108h]+4, the hud the handler's
+4) is asked `root.interface_hidden`, and it and the player icons' movie ([[hud+10h]+108h]+4)
get `root.hide_interface(not that)`: each hides its root, which is its whole interface (the hud's
fps line, which the game put on the stage, is in the root's `_stageLayer`), the hud, the
crosshair, the minimap, the kits, the markers over heads, and goes on running, so what
happens meanwhile (kill feed, messages) plays out unseen. The kits before the start and after a
death too; the start, a death and a respawn keep the state (the owner, 2026-10-24). A new match
starts shown: the movies are the last match's, and `initialize_match` (0x234880) invokes the hud
movie's `root.reset` at 0x234AE8 (ebp the hud), which shows the hud; the patch calls
`hide_ui_reset` there, which invokes it and gives the hud and the icons
`root.hide_interface(false)`. The hud's
screen effects stay: their container (`add_effect`'s blood, drips, the grenade's flash) is on the
stage below the root, placed as the root is scaled, so hiding the root leaves it. The key is not taken (0x25CF0D, al 0), as in the original. Not the game
world's own `show_ui` (0x1F95F0, slot 8 of its vtable 0x9FC4F4, its state +0A0h; the activation
shows the ui with it at 0x1F8476, the deactivation hides it at 0x1F96D1), which the first build
used: the movies it hides (+204h and +20Ch) stand still, and when shown they played everything
that came meanwhile at once (the owner's test of 64ce3ce7).

**The lobby.** `lobby_menu::on_keyboard_action` (0x26CB80) asks the binder for the lobby's
actions (kind 2) at 0x26CCD2 (eax the binder, edi 2, esi the key); the F keys never reach the
lobby movie (its DIK-to-Flash key map, made at 0x1FDF20, has none). The patch calls
`hide_ui_lobby` there: the lobby's own lookup first, as before; when the key has none, the binder
is asked again for kind 1 (its answer's toggle in a place of ours); `toggle_replay_ui` invokes
the lobby movie's `root.toggle_interface` and the key is taken (the return goes to 0x26CCAE, al
1, as after a key the movie typed: the movie never gets it). The movie hides every shown child of
its stage, so no click lands on the interface and the camera turns and zooms (`get_mouse_over`
false); the same key, any key the movie gets, Esc (`has_active_window` true,
`close_active_window`), a match found, its loading or the results show it again. While the movie
answers `root.interface_hidden` true, any other key returns to 0x26CDD2, which hands it to the
movie and returns 0: no action of the game acts on it (the window keys C, I, S, F, O, and F6,
which presses Play at 0x26CDA2). The movie shows the interface and takes that key's press and
its release (`LobbyMenu.onHiddenKey`, `onHiddenKeyUp`: a message hidden with the interface acts on
Enter's release). F keys other than F11 (the movie never gets them) do nothing then.

All three in the arena's block `hide_ui.code`, the controls in `hide_ui.data`
(`hide_ui_026g.asm`), with the late join; `get_binded_action` and Invoke through the engine
library (`engine_binded_action`, `engine_movie_invoke`); no state of its own (the movies keep it). No base relocation under the sites (0x25CED1 ten bytes, 0x26CCD2 and
0x234AE8 a `call` each); nothing aims inside them (the switch's table aims at 0x25CED1, the
jump's own first byte).

Rendering and input only; no row in `../crates/session/src/v026g/simulation_changes.tsv`.
`../docs/notes/client_patch.md` section 3.54.

Checked: `selftest_026g.py` check 62 (the action's entry, the switch and its case, the kits'
call, the lobby's call and its taken path, the block's code, exits, calls and data; the movies'
interface their root: in each movie's own sources no layer is added to the stage but through the
root's `_stageLayer` (the lobby's every former stage layer listed; the hud's one stage child
beside the root the screen effects), the right-click menu closed only when open, the lobby's
`enable_element` moving the corner's exit and settings buttons into `_overlay`'s `_corner`
(above every modal, inside the root; the exe's lobby state promoted them to Scaleform's topmost
level with it, drawn whatever their parents), the item cards in `_cards`, the tutorial's private test
functions gone; the movies' functions) and the call shapes (0x26CCD2 and 0x234AE8
CALLEE, `ret 4` and `ret 10h` as their models; 0x25CED1 JUMP); `check_hide_ui_026g.py` (Unicorn): the case 19h with and
without the replay's movie, the hud movie and the icons', the interface hidden or shown (16
cases): `root.hide_interface(not hidden)` to the hud and the icons exactly without the replay's
movie with the hud's, every case at the original's 0x25CEDB or 0x25CF0D with its esp, ebp and
stack; the lobby's call with the game's own
get_binded_action on a binder of the action table's descriptions (F11, C, F, W, H, F6, Enter,
with and without the movie, the interface hidden or shown, 28 cases): `root.toggle_interface`
and the key taken exactly for F11 with the movie; the interface hidden, every other key to
0x26CDD2; shown, back with the original's answer and toggle; the options' controls with the
game's own game_options functions (GFx objects, the strings' look-up, buffer_string's printf and
the CRT's sscanf_s stubbed): the list sent to the movie is the original's 40 rows and a 41st
(19h, its label, group 2, type 1), the keys read from the binder the original's 40 and (19h, kF11),
a key kF9 set for 19h kept in the 41st row (the original keeps it in none), the accept runs
`bind toggle_replay_ui kF9` (the original's nothing), the binder's own bind binds it, its save
writes `bind toggle_replay_ui kF9`, and that line read back binds it again. The lobby movie builds from source
(`tools/actionscript/flash_tree.py`). **Not run in a game.**

### 65. Blackout: a battery match with no interface, the players' markers on Tab (a prototype; the next release's revision)

The owner's ask (2026-10-24), its rules: `docs/notes/blackout.md`; client_patch.md section 3.58.
The Rust module `patches::blackout` (`rust/patches/src/blackout.rs`; its hooks, names only,
`blackout.toml`; `../docs/notes/rust_patches.md`), six `lohook`s at sites of its own, so no
other patch is touched and patch 63 (team damage, 0x1F0F30) and Research's markers (0x23B207)
land beside it unchanged:

- **The flag.** srp's block after the static info's squads ("SRPX", tagged entries; tag 2, one
  byte 1: Blackout on battery maps), read from the reader's cursor after each call of
  match_options::deserialize: `static_info.options_read` (0x27B92D: ESI the options, the reader
  at EBP-2Ch) and `replay.options_read` (0x1F0971: the options at [esp+40h], the reader at
  esp+14h). The options' +57870h holds "SRBO", else zeros; the patch keeps the match's flag.
- **The interface.** `player_icons.update_start` (0x23AA10, each frame): the hud movie
  `root.force_interface_hidden(flag)` (its root hidden but for the pre-match countdown and, while
  Tab is held, its Tab view: HUD.as) and the player icons `root.force_interface_hidden(flag)`
  (forced: the objects' markers hidden, the teammates' kept as in a plain match), each told at a
  change of the movie or the state, again while a movie does not take it. Forced, the movies'
  `hide_interface` does nothing (patch 62's F11, a match's reset).
- **Tab.** `keyboard.tab_state` (0x25CD2C, BL Tab down): Tab's state; in a Blackout match BL
  cleared, so the key goes on as with Tab up (the game's Tab test at 0x25CD39; patch 58's kit keys
  at 0x25CD41). `input_builder.end` (0x2421B0, EBX the handler): while Tab is held the input's
  actions mask (+33Ch) blocks acts immediately (Use is none: a search held through Tab goes on) and
  movement after 200 ms, and an act held through Tab stays out until let go.
  The input is filtered before it is applied and sent: every game plays what was sent.
- **Teammate names.** `player_names.visibility` (0x1F2F10) keeps the native Alt/crosshair
  decision, except that Tab in Blackout shows names for the local team (teams 0 and 1).
  This runs after the native life, visibility and projection checks. Enemies, spectators
  and other modes keep their native decision. Releasing Tab uses that decision again;
  each match options read resets the held key. The native cache marks changed names dirty.
- **Nothing a match computes:** no `simulation_changes.tsv` row.

**Checked:** `selftest_026g.py` check 65 (the sites' bytes, the entries the hooks jump to, the
crate's block and state); `check_blackout_026g.py` (Unicorn, from the game's code before each
site, the original beside): the options read on nine blocks for both reads; the feed over
sixteen frames; Tab and the input; names across modes, teams, native decisions and cached
visibility, including a new match resetting Tab. `cargo test` in `rust/`; `../scripts/blackout-kits/check.sh`
(Ruffle: the hud's Tab view and kits, the player icons' states). **Not run in a game.**

### 67. The kill feed shows a kill through a wall, by a ricochet and by pain (the next release's revision)

The owner (2026-10-25): "when a character is killed from pain, the character should get <weapon>
<white painkiller> icon"; "when through a wall: <weapon> <wall shot> icon (available in last
patches)"; then yes to retail's ricochet icon. The rules: `../docs/notes/kill-feed-icons.md`;
the evidence: `client_patch.md` 3.55.

**The flags.** A death makes a kill event in every game's simulation:
`base_player::generate_killed_event` (0x2DA170, the victim in esi, the killing bullet or 0 in
ebp) writes the victim, the killer, the weapon's icon and the cause icon (+4: 1 headshot, 3 no
killer) into an event node whose first 12 bytes its allocation zeroes. The patch
(`kill_feed_event`, called at 0x2DA3D1 in place of `mov [eax+4],bl / mov edx,[esi+120h]`)
writes the cause icon and, in the node's +5, the flags. For a bullet, someone else the
killer (the cause icon not 3) and no melee hit, retail 0.69d0's rules for its `ricochet_icon` and
`penetration_icon` (its exe: the hit carries the bullet's segments and reflections,
generate_killed_event sets ricochet for any reflection, else penetration for segments, neither
for a melee hit or no other killer; any pierced object or player counts; the melee flag is
generate_killed_event's own, `[esp+14h]`): 4 when the bullet
reflected before this hit (its reflections, +88h, not 0), else 1 when it went through something
(its trajectory's segments, +84h, which each pierce and each reflection adds one to, not 0).
And 2 when the victim's pain pool is empty and its body and head pools are not (the damage
model's parts by name, 0x2E58B0, health +0A0h; one hit emptying pain and body or head together
is a death by damage). All are the simulation's own state, alike in every game; nothing else is
written.

**To the movie.** The hud's event handler (0x23B5F0) pushes the node's byte +4 as
`on_player_killed`'s fifth argument; at 0x23B6F0 it pushes the word (`KILL_FEED_ARGUMENT`), and
on_player_killed reads the first byte alone for extra_icon (0x2380EA). `kill_feed_row`, called at
0x23814F (after extra_icon, before mastery_icon), sets the description's `penetration_icon`,
`ricochet_icon` and `pain_icon` (GFx ints, 0 or 1; 0.69d0's names for the first two) the way the
function sets its own members, then does the site's two moves.

**The movie.** `CombatLogItem` shows `CombatLogWallIcon` (0.69d0's `icon_penetration.png`),
`CombatLogRicochetIcon` (0.69d0's `icon_ricochet.png`) and `CombatLogPainIcon` (a white
lightning bolt for pain shock, drawn) after the weapon and before the cause icon, for a kill or a
suicide row, in 0.69d0's order; the build adds the three bitmaps (`../flash/hud/bitmaps/`, made
by `../scripts/kill-feed-icons/make_icons.py`).

**Checked:** `selftest_026g.py` check 67 (the sites' original code, the hooks, no relocation under
them and no branch into them, the code the link's, its one jump into the game the part lookup;
the movie's row); `check_kill_feed_026g.py` (Unicorn): the event from 0x2DA3AD to its queueing
in 26 cases (a bullet death, a pain death, one hit emptying pain and body or pain and head
together (damage, not pain), a headshot through pain, pain all but empty, a toxic death, through
a wall, a ricochet, pierces and a ricochet (the ricochet only), a ricochet then a pierce, a
ricochet to pain, a ricochet with no killer (none), melee hits with a straight bullet, with
segments and with a reflection (none) and by pain (pain only), through another player,
a wall with no killer, a headshot through a wall, a wall and pain, no bullet, no damage model,
the parts in another order, no head pool) with the game's own part lookup: the node the
original's but +5; the handler's five arguments; the row's members (penetration_icon,
ricochet_icon, pain_icon, then mastery_icon) against the original's mastery_icon alone, with the
registers and the frame as the original's. **Tested in game by the owner, 2026-10-25** ("works nicely"); the explicit melee test (the event's melee flag) came after that test and is checked with Unicorn only.

### 70. The baked camouflage and decal textures are kept, and baked or loaded once while asked for again (the next release's revision)

Players' short freezes in matches at a steady frame rate (2026-10-25, revision 33). The rules:
`../docs/notes/baked-textures.md`; the evidence: `client_patch.md` 3.57.

**Why.** The camouflage/decal baker draws an item's camouflage and decals into textures and
saves them as `user_data/streamed_textures/<name>.dds`; its list of what it saved lets a later
query load the file. Stock, the game deletes the folder at its start and its exit (`game::game`
0x1E9455 and `game::exit` 0x1EC832, SHFileOperationA FO_DELETE), and the list lives in memory
only. So every game bakes every item it sees, in a match when a player with an unseen kit comes
in: 5 to 30 ms each (bake, compress, CRC, save), a burst of 6 to 50 back to back. A texture goes
into the list only when its bake is done, so the player objects of one player (one for each of
three kits, often wearing the same items) bake the same textures 2 or 3 times: the owner's first
match baked 115 textures into 62 files. Revision 32's starter kit (#400: three kits wearing the
same items for every new account) makes each newcomer cost three times the same bakes.

**The patch** (with the late join; the Rust patch bake_cache, `rust/patches/src/bake_cache.rs`
and `.toml`, over `vostok::baker`, `::files` and `::crc32`; seven lohooks):
- `game::game` before its delete (`lea eax,[esp+470h] / call 0x1E8DA0`, the lohook at 0x1E944E,
  `on_game_start`; it goes on past the call, 0x1E945A). The folder is
  deleted only when its stamp `srp_baked_textures.txt` is missing or names another bake format
  (`BAKE_FORMAT`) or other game data: a fingerprint of the files of `resources.db`'s patches that a
  bake reads (models, material instances, textures, shaders, camouflage and decal items), from each
  patch's file table (`bake_cache/inputs.rs`); then it is made again and stamped. So a release keeps
  it unless its patch has such a file. Revision 54's stamp counts as format 1's. Otherwise each
  whole DDS in it (DXT1 or DXT5, the file as long as
  its header says) goes back into the baker's list with CRC 0, at most 512 files and 896 MB (512 MB until 2026-11-09: half a 20-player
  folder was deleted at every start); the
  rest are deleted. `game::exit`'s delete (0x1EC832) is a 5-byte nop (`BAKE_CACHE_KEEP`).
- check_loaded_baked_texture's CRC compare (0x131C90, `on_check_crc`): a record of CRC 0
  takes the CRC of the bytes loaded, so a kept file is used; then stock rules apply.
- translate_query before its look-up (`lea edx,[esp+1B8h]` before `call 0x131ED0`, 0x12D4C5,
  `on_lookup`). The first
  query of a path owns it. A later query of the path while it is under way waits: it leaves
  translate_query by its own end (0x12D42D), with no look-up, no bake and no load, and holds a
  reference to its model instance.
- When the owner's texture is made, process_compression (0x12F194, a bake) and
  on_baked_texture_loaded (0x12DDB9, a load), after their `call 0x420789` (`push eax / mov
  eax,108h`; `on_baked` / `on_loaded`). Each waiting query gets the texture in
  its surface's slot, a resource as on_baked_texture_loaded makes one, and finish_query with
  success. This happens only if the texture is in the owner's slot.
- finish_query's entry (0xF0D00, `on_finish`; its exit is the function's own `ret 10h` at
  0xF0E6C). An owner that ends any other way
  (an error, a requery, a texture not in its slot) has its waiting queries made again
  (translate_query) after its own finish. Any other query goes straight to the function. An owner
  still listed after 10 s (GetTickCount) is taken over by the next query of its path, its waiting
  queries with it.
- A texture loaded from the folder is held for the game (one reference more; at most 256 of them and
  256 MB of their files, the least used let go). A later query of its path (a respawn rebuilds the
  player's model: the owner's log read one player's armour again at every respawn) gets it at once in
  `on_lookup`, with no file read, no CRC and no new texture, and leaves translate_query
  finished.
- The game's CRC-32 step (0x42050: a byte a step through the table 0x1C210 makes, polynomial
  04C11DB7h reflected), which every bake runs over its whole DDS (process_compression 0x12F340) and
  every load over the file read (0x3C0FF0), 1.4 to 5.6 MB each (its start 0x42050, `on_crc_step`;
  its exit the function's own `ret 4` at 0x4207E): the same value, 64 bytes a step folded with
  PCLMULQDQ where the processor has it (CPUID), else eight bytes a step (slice by 8, tables from
  EDB88320h, made at build time); the registers as the
  original leaves them. Measured on the owner's game, the CRC was most of a bake burst's stall
  (16:37:47, 0.4 s); here 0.48 instructions a byte folded, 3.25 sliced, against 11.
- The stamp is written only when no `.dds` is left after the delete (one held open by another game
  is not deleted): otherwise the next start deletes the folder again.

Textures only; games with and without it play together. Sites (lohooks, JUMP): 0x1E944E,
0x131C90, 0x12D4C5, 0x12F194, 0x12DDB9, 0xF0D00, 0x42050; no base relocation under them, no branch
into them (each span branch-free: main's lohooks replay it).

**Checked:** `selftest_026g.py` check 70 (the sites' original code, the generated lohooks at
their sites and exits, the nop, no relocation or branch inside). `check_bake_cache_026g.py`
(Unicorn) runs every
site from the game's code on the game's own static baker:
- the folder, with made-up patches of the game data (their file tables): kept and back in the
  list, a file cut short and the files past 512 deleted; kept by a release whose patch has only
  item stats, deleted and stamped anew for a patch with a texture, a decal item or one that cannot
  be read, another format, a stamp of other bytes, no stamp, or no folder; revision 54's stamp kept
  and stamped again (deleted with a texture in patch 55), revision 53's deleted; no `resources.db`:
  deleted, not stamped; the fingerprint the same as the check's own (Python) of the tables; the
  original deletes it every time;
- the game's own check_loaded: a kept file's first load is used and takes the CRC read;
- the look-up: the original bakes twice; the build makes a second and third query wait, and lets
  another path, another type and a 17th query go on;
- the texture made by a bake and by a load: given to the waiting queries;
- an owner's error, and a texture not in its slot: the waiting queries made again;
- an owner listed for over 10 s: taken over by the next query, which serves the waiting one;
- a held texture: a respawn's query gets it at once with no look-up; past 256 MB the least used
  goes;
- a folder whose delete leaves a file: not stamped;
- the CRC-32 step against the original on 32 buffers of 0 to 10 MB from unaligned starts with
  random CRCs, on a processor without PCLMULQDQ (Penryn) and one with it (Westmere): the CRC and
  every register the same; 3.25 and 0.48 instructions a byte against 11;
- any other query's finish goes straight through.
**Not run:** a game.

### 121. The camouflage/decal bake does not wait for the GPU

A player's report (2026-11-05): with `streamed_textures` emptied, every player joining a room or
a match makes a hitch while their items bake. Stock, a bake reads its 2048x2048 target back
(blocking), uploads those 16 MB again as the compressor's level 0, and reads each of its 10
compressed levels back, blocking each time (`client_patch.md` 3.57, "The bake's GPU round
trips"). The rules: `../docs/notes/baked-textures.md`, "Bakes off the GPU's clock".

The first version (2026-11-05..07: 8 in flight, 8 queued, past them the stock bake, 2 finished a
dispatch) gave nothing in a match load (2026-11-08: 19 bots in live players' gear, ~800 bakes in
9-12 s): frames over 100 ms 7.2 s against main's 7.7 s, the longest ~1.3 s in both. 474 of the
768 bakes found no place and took the stock bake, which then waited behind the GPU work. The
second (2026-11-08) put a bake with no place off before its draw and made its query again
(finish_query's requery) once one was free; in the same load those requeries piled up and the game
died with "out of memory in arena 'resources helper allocator'" (2026-11-09).
The third (c00b9fe6c, the design below without its drain) completed no bake in a pass between bursts:
the bakes' thread is not the one that ticks frames and dispatches only while its callbacks come,
so a burst's last bakes stayed on the GPU until the next burst, up to ~65 s (2026-11-09).

**The patch** (the Rust patch `bake_async`, `rust/patches/src/bake_async.rs` and `.toml`, over
`vostok::bake_gpu`; nine lohooks of `sites_026g.tsv`):
- `bake.issue` 0x12EEA7: a place on the GPU, one of 8. With all 8 taken, the oldest bake is
  completed first, on the spot (read back, waiting only when the GPU has not done it, and its tail
  run), and the new bake takes its place: no bake waits for a place, is put off, or is asked for
  again. Level 0 a GPU copy of the target (copy2D into a DEFAULT texture), the compressor's levels
  drawn and copied to stagings (`compress_mip.readback` 0x11F754 keeps each staging, no map, no
  copy: the cursor counts places), the last copy flushed (ID3D11DeviceContext::Flush, as
  compress_mip flushes before each copy), the job's frame read once, process_compression left by
  its epilogue 0x12F561, its query not finished. No DDS memory is held. A bake whose compressor does
  not fit its DDS, or whose level 0 finds no texture, fails its query (logged, never tried again).
  The stock bake only after game::exit, for another thread's bake while the places are one
  thread's (logged), or for a frame this does not read (logged).
- `resources.dispatched` 0x1024FA, each dispatch: a pass on the places' thread (one completion at a
  time). The oldest bakes whose last staging maps with the no-wait byte get their DDS (mt_allocate
  then), are read back into it, and process_compression is called again: `bake.resume` 0x12EC57
  jumps to its tail 0x12F00B: the texture made from the DDS, its slot, patch 70's sharing, the
  resource, the save, the CRC, the record, finish and the "camouflage/decal cook" line.
  `bake.save` 0x12F2D8 gives the save a descriptor of the game's that owns that DDS (no copy);
  with no memory for one, EAX 0 (no save, logged; the game's make_save_descriptor would write
  through 0). At most 64 saves, 128 MB, wait to be written (`save.freed` 0x10A050, the descriptor's
  destructor, takes each off; 2026-11-09: MT at 99.3% after 458 bakes in 11 s, a crash); past
  either, the texture is used and not saved, its DDS freed at once (logged). `bake.free` 0x12F3F7
  keeps the DDS from the render allocator's free. Done bakes
  complete while the frame's 6 ms last (one at least; a bake completed for its place counts too);
  the rest in the next frames, oldest first. A bake the GPU has had 1 s is read back waiting
  (logged); one whose levels cannot be mapped, or that finds no MT memory for its DDS, fails its
  query (logged). That is on the thread that ticks frames. A thread that ticks none (or any,
  before the first frame) may not dispatch again for minutes: its pass completes all its bakes
  before the dispatch ends, waiting where the GPU has not done them (logged, a line a second, with
  both threads; the frame thread's dispatches log whose the bakes are).
- `engine.tick` 0x13CA0 (engine_world::tick's start, once a frame): a new frame's budget, and
  its thread the one that ticks frames. A frame with no tick for 100 ms (apc's wait loops dispatch
  without one) counts as ended.
- `game.exit_start` 0x1EC730 (game::exit's start, before every branch): every bake on the GPU
  completed waiting, on its thread (now, or at its next pass); later bakes are the stock ones.
- Types: the job and its stagings are one value (`Issued`), completed once (`complete(self)`); the
  job's query is finished exactly once (by the tail, or failed when the job is dropped); the size is
  stored once, in the job. Every value holding the engine's references or the immediate context's
  work is neither `Send` nor `Sync`: the shared state keeps it in a `vostok::ThreadBound`, which only
  its own thread reaches (dropped on another, it is leaked untouched). `compress` refuses unless no
  capture is under way, a completion unless none is. One transition for a bake on the GPU (`step`),
  one for a completion's tail (`Resuming::on`); the places change only through the table's methods.
- `engine_026g.py`'s call kind `com` calls ID3D11DeviceContext::Unmap and Flush.
- `vostok::log` writes a line at a time (the hooks log from several threads; lines came out cut
  and mixed in the 2026-11-08 run), up to 512 bytes.

Textures only; games with and without it play together. No base relocation under a site, no
branch into one (scanned in the original: rel8/rel32 jumps and calls).

**Checked:** `check_bake_async_026g.py` (Unicorn) runs process_compression from its entry, the
compressor's functions, map2D, the dispatch's end, engine_world::tick's start, game::exit from its
start, finish_query and the tail for real on a made-up D3D11 context: the original maps 1 + the
levels, blocking; the build issues with no Map and no MT memory (backend::reset once before
make_mips, a Flush after the last copy), polls busy with one no-wait Map, then allocates the DDS,
reads the levels into it, makes the texture from it, fills the slot and patch 70's waiter's,
finishes both, the save descriptor owning that DDS; late after 1 s; the device lost, no MT memory,
no save descriptor; a DDS too small and no level 0 (failed, no stock bake, not tried again); 8 on
the GPU, a 9th completing the oldest first on the spot (read back waiting, patch 70's waiter given
its texture) and issued in its place, a 10th completing the done oldest without waiting, the rest
at a dispatch, oldest first; another thread's bake the stock one while the places are this
thread's, then issued on its own thread once they are free; the bakes of a thread that ticks no
frames untouched by the frame thread's dispatch (logged) and all completed at its own dispatch's
end, waiting for the ones the GPU had not done (this case fails on c00b9fe6c); 64 saves not written
yet, then the 65th bake used and not saved, its DDS freed (fails on 712dc41e5), and a place freed
by the game's ~save_descriptor for the next; one completion a frame past the
budget, the frame ended after 100 ms with no tick; a done bake behind a busy one; game::exit
through 0x1051E0's failing branch (every bake on the GPU completed), then the stock bake.
`cargo test` covers the transitions, the places in the DDS and the thread-bound values.
**Not run:** a game.

## Not ported, not done

- The 0.21d mode 3 battery patch is not needed: 0.26g serializes the
  next-battery time. (Patch 9 above is another defect, 0.26g's own.)
- A defect of the client itself is known and not patched: an input with the
  "previous weapon" bit from a player who holds something other than its two
  weapons can crash every client of a match
  (`../docs/notes/match.md` §20).

## Language in Settings (patch 79)

The game language is a setting (`../docs/notes/language-option.md`). The Rust
`language` patch registers the console variable `localization` (a saved
string, so user.cfg keeps it) at game::on_configs_loaded's start; after the
file runs and before the dictionary loads it resolves the language once: a
registered `-localization=` (then saved as the choice), else the saved choice,
else the language of the Windows user locale (GetUserDefaultLCID, by its
primary language: `LANGUAGES`' last column), else English; the resolved one
is saved as the choice (the launcher's registry language is not read); the
engine's language string and C locale are set as a start with that flag sets
them. The main menu's Settings (Gameplay) call `srp_language` (a pick, or a
query): the patch saves a registered pick and writes user.cfg at once, and
answers `root.srp_language` with the two texts, the saved and the running
language and every registered language by its own name. The launcher's PLAY,
the replay start and the scripts pass no language by default. See
`check_language_026g.py` and `../scripts/language-option/check.sh`.

## Lobby scene by pick (patch 81)

The lobby's scene is the one the player picks in the lobby window
(`../docs/notes/lobby-scenes.md`): the player panel's lobby button (in the premium button's place) opens four cards,
the Room (`lobby_scene/camp_069`, the default), the Basement (the classic,
`lobby_scene/day`), the Bunker (`lobby_scene/stk2_bunker`) and the Hangar
(`lobby_scene/hangar_046`). The pick is kept in user.cfg as `srp_lobby`
(registered at game::on_configs_loaded's start in the site's chain after
patch 79's and patch 83's handlers; user.cfg written when it changed), and the start's query
asks its project in the classic's place (0x21E5CC). The four lobbies ship
with the game: a project without its camera track is a bug, logged in
srp_patches.log. Patch 45 takes the weapon's spot from the lobby project's `srp_lobby` table
(Round 6).

The window sends srp the pick (`3B` at 2 095 000 000 + id); srp keeps it
and sends it back in `0xA1` with the squad commander's (two bytes after the
squad; the stock reader stops before them), and the patch reads them
(0x2747C3, 0x2748BC) into its own pick (saved) and `Squad::{None, Led}`;
srp without a pick (the sign-in) is sent the saved one (the lobby client's
0x270070). In a squad the lobby is the commander's pick.

A change shows at once: at the lobby camera's tick (0x2840F2, after patch
45's flight) with another lobby wanted than the one shown, the scene fades to
black under every panel (the movie's `srp_scene_fade`, stepped a frame each,
the loading spinner on it), then the patch asks that lobby's project alone
and puts it in the old one's place behind the black: the project halves of lobby_menu::clear_resources and
on_render_scenes_ready redone with the game's own functions (the old
project's remove and static collisions, the track player's stop, the
release; the new one's vtable+0Ch, its track's effect through the emitter's
vtable, the insert and its collisions), the eye adaptation reset (the
engine's byte 0xAC914C) and the camera at its stock pose; six frames later
the black fades out. A project that fails behind the black is logged and the
black fades out over the lobby shown; a match entered during a change leaves
no black; once
game::clear_resources (0x1ED030) runs nothing is asked. One enum (`Scene`),
one transition function (`Scene::step`). See `check_lobby_scene_026g.py` and
`../scripts/lobby-fade/check.sh`.

## Medical consumable countdown (patch 77)

The Rust `consumable_activity` hook publishes the medkit class's current use-delay
and activity state to the HUD quick slot. Anesthetic, medkit and other medical
consumables display the native artefact-style countdown in ceiling seconds during
the actual effect, including after the last item in a stack is spent. During the
initial use delay the icon is gray with the native filled radial sweep, without
a numeric countdown. The sweep uses the skill-modified native activation delay. End, cancellation, death
and HUD cleanup clear the presentation; artefacts retain their existing display.
This is presentation only. See `docs/notes/client_patch.md` and
`check_consumable_activity_026g.py`; the Flash lifecycle check is
`scripts/consumable-activity/check.sh`.
A drug a bullet stopped (patch 84) shows its icon red for one second of the
local player's simulation clock, the last unit's too; a spent drug, a death and
the clean-up show none. The hook tells it by the timers set_active(0) leaves
and keeps only each quick slot's last seen state; no ActionScript timer.

## Weight slowdown indicator (unreleased)

The lobby weight number turns coral when carried weight slows movement and
stays red at the carry limit. Its native warning tooltip shows the weight's
movement and sprint-stamina penalties, including the selected kit's skills,
and explains when weight or Heavyweight prevents sprinting. This is UI only.
`weight_ui_026g.c`/`.asm` extend the existing weight callback from the same
player config and native profile evaluator. Before it, they send the movie the
selected kit's 68 player modifier floats (`root.set_kit_modifiers`: the skills),
which a weapon card adds while Alt is held (`docs/notes/alt-weapon-stats.md`). See the carried-weight section of
`docs/notes/data.md`, `check_weight_ui_026g.py`, and
`scripts/weight-ui/check_ui.sh` for evidence and targeted checks.

## Results screen without a loaded match (unreleased)

A match's results (0x97) reaching a game that has loaded no match (one started
anew and reseated into a match that then ended) crashed it: the screen's map
name, lobby_menu+313Ch, is NULL until a match is loaded, and translate_text
read it (0xF6C47). `match_stat_map_026g.asm` (the arena block
`match_stat_map.code`, a jump at 0x29ABE1) reads a NULL name as "": the screen
shows no map name. See `docs/notes/results.md` section 6, hazard 10, and
`check_match_stat_map_026g.py`.

The same NULL came from expand_text (0x1FEF90): a text's `{USE}` whose
action has no key bound (the key table 0x207BF0 finds no entry) translated
NULL at 0x1FF09D. `EXPAND_TEXT_UNBOUND` retargets its branch (0x1FF073,
`je 1FF090` to `je 1FF0AB`) to the path of an unknown placeholder, which
appends nothing: "Pick up ()". Later builds (0.31a2 on) translate
`st_key_use_not_assigned` there; 0.26g's dictionary has no such key, and
translating "" would give the same empty text. translate_text's 0x200 is
only a log line ("Too long localization string"), it truncates nothing; the
catalog check already keeps every text under 512 bytes, so it stays.

## Knowledge database link (unreleased)

The lobby's Knowledge database button opens `https://026g.skhid.link/wiki`
after the existing confirmation. `KNOWLEDGE_BASE` changes the fixed URL and
skips the old support-site path appends; the native browser call is preserved.
The wiki uses browser language selection. `check_knowledge_base_026g.py`
executes the original and redirected callbacks for every game language.

## Breakable lamps (patch 81)

Shooting a configured lamp replaces its model, removes linked lights and
cones, and queues glass shards and sound once. State survives rollback,
late joins and replay snapshots; a new match restores the lamps. The content
covers 92 variants, including the v0.11/v0.21 imports. Legacy Vostok cones
are bound to their fixtures and aligned to their emitters.

The safe Rust `lamps` module uses typed config, geometry, history and project
views. One project registry owns renderer membership and native bindings.
`lamps.toml` declares generated hooks and runtime vtable copies; frame and
bullet hooks share ordered detours with ragdolls. There is no lamp C/NASM
code. Native Unicorn tests cover all authored maps. Details and test limits:
`../docs/notes/breakable_lamps.md`.

## The Settings' HUD group (patch 83)

The Gameplay tab's "HUD:" group (`../docs/notes/hide-hit-markers-scores.md`): four settings kept in
user.cfg, off by default, `srp_hide_hit_markers`, `srp_hide_scores`, `srp_map_names` and
`srp_hide_crosshair` (the crosshair list's Hidden). The Rust `hud_options` patch registers the saved bools at
`configs.start`, answers the main menu's `srp_hide_*` calls and `srp_hud_texts` (the group's
texts) at `options.callback` (after patch 79's hooks there, sharing `vostok::settings` with it), and
tells the hud movie the four at `player_icons.update_start` (after patch 65's hook) when the movie
or a setting changes, and again after each match's or replay's options read (a new hud can be made
at the last one's address). The hud hides the crosshair's hit marker, shows the Tab list as names
only, hides the crosshair, and shows the Tab map's names without Alt (Alt hiding them, patch
82). The stock quest progress and minimap controls are moved into the
group by the main menu alone. Interface only: no simulation change. `check_hud_options_026g.py`
runs the hooks from the game's code; `../scripts/hud-options/check.sh` the movies in Ruffle.

## Artefact score (patch 80, unreleased)

An anomaly's artefact scores 15 taken into an empty artefact slot, 5 with every
slot full (the owner, 2026-10-29; `docs/notes/artefact-score.md`). The game
counts event 5 for every artefact an anomaly gives; the Rust `artefact_score`
hook, before `artefact_container_core::use_execute` hands it over (0x31C810),
counts event 16 too when one of the user's artefact slots is empty, in the
user's place and kit cell, and tells the statistics' listener, as the game
counts one. srp weighs event 5 5 and event 16 10. A match's computation
changes: every client of a match needs it. `check_artefact_score_026g.py`
runs the hook from the game's site and compares it with the game's own count
of event 5 and `take_artefact`'s own slot search.

## Battery progress (patch 92, unreleased)

A battery's carriers earn 2 points for each 10 m the battery comes nearer their team's container
than it has been in its life, from the match's start or its last delivery, whoever carried it; the
points are held on the battery and paid when it is put into that team's container, to every carrier
at once, and dropped when the other team delivers it (the owner, 2026-11-01: the score ticking up
mid-carry told the enemy who brings a battery in; `docs/notes/battery_credit.md`). The Rust
`battery_progress` hook in `base_player::tick` (0x2DAAD3) holds the steps for the carrier's place
and kit; at a put-in (0x334CEA) the container team's carriers are counted 40 hundredths of event 6
a step and the statistics' listener told; each battery's life (its nearest band per team and the
held steps) is the patch's own state, begun again at start_match (0x334999) and at a put-in, and
saved after the record's own fields (0x334F66, 0x3350D0). A match's computation and the world's
statistics change: every client of a match needs it, and the server reads 3 bytes and 2 a carrier
more a battery. `check_battery_progress_026g.py` runs the hooks and the game's own serialize and
deserialize.

## Names on the Tab map (patch 82, unreleased)

While the Tab map is open and an Alt key is held, each player icon on it shows the player's name
(the owner, 2026-10-30; `docs/notes/tab-map-names.md`). The hud movie draws them
(`HUD.show_map_names`, the names from the Tab list's rows by place); the Rust `map_names` hook, in
`game_world::on_keyboard_action` with the input world's keyboard (0x25CD1C, before it asks for
Tab), reads both Alt keys' state and tells the movie `root.show_map_names` at each change. The
interface only: no simulation change. `check_map_names_026g.py` runs the game's own
on_keyboard_action, keyboard getter and is_key_down to past show_player_list in both builds.

## Tab list cursor and friend requests (patch 86, unreleased)

Ctrl pressed while the Tab list is held shows the hud's cursor (the kits'
own `show_cursor`, 0x23A930) until Tab is let go. The view and the mouse
buttons stop; the moves go on. That Ctrl press and its held and release
events are taken from the player, so it does not crouch. A right-click on
a row opens the hud movie's `PlayerRowMenu` with "Add to friends" and two chat mute entries. The
entry sends the lobby's own friend request (`4D` type 0, through
messaging_client::send_important_message 0x26AFB0) to that place's bare
name. The rules are in `../docs/notes/tab-cursor-friends.md`.

The Rust `tab_cursor` module has two lohooks of its own, and one more after
Blackout's at a match's options read (`static_info.options_read`,
`replay.options_read`, order 1), where the cursor state starts over:
- `tab_cursor.key`: the start of game_world::on_keyboard_action (0x25CD00).
  Its exit `taken` is the function's `ret 0Ch`, with AL 1.
- `hud.callback`: the start of game_world_ui::scaleform_callback (0x233D30).
  It answers `srp_player_menu`, `srp_add_friend` and `srp_chat_mute` itself;
  any other call goes on to the game's own dispatch.

The client's checks are the lobby's: never the account's own name or a
friend's; five requests at once, then one every 10 s; and the lobby
connection up, as the lobby's senders check it. A chat mute
(`../docs/notes/chat-mute.md`) is the lobby menu's own `49 05` (kind 254, the
mode), sent by lobby_client::comment_about_user (0x274470): never the own
name, one of the three choices that changes the state the lobby's ignore list
gives (a `<muted>` player's id marked with bit 31), the connection up.

Nothing a match computes changes. Blackout matches (no list) keep Ctrl as
crouch. `check_tab_cursor_026g.py` runs both hooks from the game's code
beside the original.

## World check (patch 87, unreleased)

A game parses every world it is sent (0xE8) before its own reader takes it,
with the parse the match server runs on every upload: the crate
`../crates/world-frame` (`Frame::parse`, no_std, no alloc), a path
dependency of `rust/patches` and a member of srp-026g's workspace
(`../docs/notes/world_frame_check.md`). The frame must walk from the front
to exactly its end, no mask bit may be at or past the game's 20 slots, no
player state may be over the game's buffer for it, and the body may not be
over the 0xF000 bytes hash() serializes it into (patch 98, which also lets
the sections grow, so they have no cap of their own). Each of
these would kill the game in `game_state_history_item::deserialize`
(0x324D80) or `mutable_buffer::reallocate` (0x7AB70), and no world a game
made fails one.

The Rust `world_check` module has two lohooks:
- `world_load.unpacked` (0x27C0D0): before the load of an unpacked world
  (0x27C0D9), the world in the frame's mutable buffer at EBP-24h;
- `world_load.raw` (0x27C0FF): before the load of a raw world (0x27C111),
  the world the rest of the packet's reader (ESI).

A refused world is never loaded: the packet's reader is read to its end,
patch 97 requests a fresh generation for this recipient, a line goes to
`srp_patches.log` (every refusal: at most a line a second, each counting the
refusals since the one before), and the exit `refused` is the handler's epilogue
(0x27C1E0). Nothing a match computes changes. `check_world_check_026g.py`
runs both loads beside the original exe; `--drift` the client build against
the server build on kept worlds.

## The weapon kit (patch 68, unreleased)

A weapon holds up to six upgrades (two decals, camouflage, scope, and later
a muzzle device and a grip) in the client's three upgrade records
(`../docs/notes/weapon-kit.md`): srp sends the decals as records of their
own and one kit record in slot 3 that holds the first of scope,
camouflage, muzzle and grip and names the others by dict id. The encoding
is the crate `../crates/weapon-kit` (no_std, no alloc), which the servers
link and `rust/patches` builds in. The Rust `weapon_kit` module:
- `weapon_kit.query_room` (0x2F1771, a `code` hook, weapon_kit_026g.asm):
  translate_query's call of 0x2F2060, then A0h of room below its frame;
- `weapon_kit.query_records` (0x2F1776, a lohook): for a kit, the cook's
  records ([ebp+40h]) are the records (the kit's own valued 0) and one for
  each dict id it names, in that room;
- `weapon_kit.flash_upgrade` (0x2882E3): a kit's object gets `kit_own`,
  `kit_camo`, `kit_muzzle`, `kit_grip` (the AS3 interface's SetMember
  0x7C6A20), which the lobby movie's `UpgradeWindow.kitUpgrades` reads;
- `weapon_kit.attached_pair` (0x2885CE): the dict ids a kit names mark the
  inventory's items as attached to the weapon;
- patch 48's apply calls the entry `weapon_kit.preview_apply` (generated,
  the stack aligned to 16) through its dword +23h, which the build writes.

A malformed kit is refused: the item is built from its records as they
are. The lobby's weapon window has two decal slots. Nothing a match
computes changes (weapon-kit.md, "The simulation").
`check_weapon_kit_026g.py` runs the four sites and patch 48's apply beside
the original exe; selftest check 68 reads the sites.
## Music (patch 88, unreleased)

The main theme on the login screen, the lobby theme in the lobby, and the map's theme from its
loading screen to 15 s past the match-start announcement, each fading in and out over 2 s, never
two at once (the owner, 2026-10-28 and 2026-10-31; `../docs/notes/music.md`). Stock 0.26g plays no
music; the tracks are the content pack `music@1` and each mode's `ui.loading_music`
(`music-1.json`), as retail's. The Rust `music` module plays them in a sound scene of its own (the
bus, class 38) that the game never makes current: the sound thread's tick (`sound_world.tick`,
0x59AB1) keeps it among the active scenes and steps its volume, which the engine's own fade path
gives its submix with the general volume; the volume is the stock Settings' Music slider's
(`s_music_volume`, which the game's option names but never registered: patch 88 registers it at
`configs.start`, a saved cc_float of 0 to 100, and the main menu no longer greys the slider out)
times the fade. Its events: `game.tick` (the bus asked for, the 15 s,
a fade done), `login_menu.activated`, `lobby_menu.activated`, `level_loading.ui` (the mode's `ui`
table), `game_world_ui.welcome` (the announcement), `game.clear_resources` (the unload). Its tracks
are single sounds loaded by `query_resources` (class 256) with a callback of its own
(`resources.query_done`, a `callback` site: the generated entry handed to a boost function made
by 0x176D0, no site rewritten). The presentation only: no simulation change.
`check_music_026g.py` runs every hook from its site and the callback through the game's invoker,
and the slider's variable through the game's lookup and setter.

## A bullet stops a drug (patch 84, unreleased)

A bullet that hits a player stops the healing drug he is using (the owner,
2026-10-31; `../docs/notes/drug-interrupt.md`): in its use time it never
starts healing, in its active time it stops, what it healed stays; the unit
stays used. The Rust `drug_interrupt` hook is in `base_player::hit_impl` once
`damage_model::hit_body_part` said the hit took health (0x2D9CB1): for a hit
of type `ranged`, each of the victim's inventory slots holding a drug in use
that heals (influences, so not the painkiller) gets the game's own
`medkit::set_active(0)` (0x33D390, `engine_medkit_set_active`), the end
medkit::tick gives a spent drug. A match's computation changes: every client
of a match needs it. `check_drug_interrupt_026g.py` runs the hook from the
game's site in the patched exe, and the game's own set_active(0) in the use
time, the active time and between them.
The stopped drug's quick slot shows red for a second of game time (display
only, through patch 77's hook and the hud's QuickSlots; `check_consumable_activity_026g.py`,
`../scripts/consumable-activity/check.sh`).
## Gun-stock hits kill (patch 108)

A melee hit kills, wherever it lands (the owner, 2026-11-05; `../docs/notes/hand-kill.md`;
revision 50 had it for the hands only). The Rust `hand_kill` hook is in
`damage_model::hit_body_part` once invulnerability is applied (0x2E50C7, patch 94's
`medical.hit`, ours first): a blunt hit on any zone with an attacker (another player; a
fall's attacker is the victim) deals 1000 from there, so its share of pain (5 % at the
least, an arm's or a hip's) empties it, the game's own pain death (the kill feed's pain
mark). A match's computation changes: every client of a match needs it.
`check_hand_kill_026g.py` runs the chain from the game's site in the patched exe.
## Dropped-weapon pickup (patch 85)

A player takes the gun a dead player dropped: Use on the gun lying on the
body, within 2.2 m. The spec and the evidence: `docs/notes/weapon_pickup.md`.
`rust/patches/src/weapon_pickup.rs` (the records, the decisions, the log)
over `vostok/pickup.rs` (the native moves) and `vostok/gun_cook.rs` (the
cooked gun); the shared rules (`crates/weapon-pickup`) are the server's too.

**The copy.** At a death that drops a gun (the held client weapon in slot 7
or 10), on every client at the same tick, that gun alone is cooked: one
query of the player cook's own inventory request (41h) from a copy of the
player's profile keeping that weapon slot and the ammunition slots, its
completion a boost member callback (0x27D280). A replayed death finds it; a
copy is never freed mid-match once it was registered or in hands (the
engine keeps raw pointers to a weapon, e.g. a bullet's +0Ch/+40h): it lives
until its last holder is destroyed, and an idle one is its donor's next
drop's gun with the same kit and slot. An untaken copy is freed at the frame
after its body left the world (base_player::remove(1)). 192 tickets: 120
recipes at most plus 60 in use. A pickup whose gun is not in yet waits on
dispatch_callbacks (0xE5AB0), at most 2 s.

**The record** (`weapon_pickup::Record`, 48 bytes, version 4) after each
player state's two kit indices (`pickup.save` 0x2DAF70, `pickup.load`
0x2DB4CF): its drop (open with kit and slot, or taken), the gun picked into
each weapon slot (tick and recipe), the pickup waiting for its switch (slot,
pick, the donor's magazine) and, for the server, what each pick parks of
the own kit (the own gun's magazine, up to four own stacks: slot and
rounds). The server reads the native fields from `FIELDS_AT` and counts a
borrower's own ammunition from the parked part (borrowed ammunition is
never charged).

**The pickup** (artefact_carry's `carry.filter` calls `on_filter` with the
raw actions first): the command (patch 78's bit 29 with two or more quick
bits) is decided by `weapon_pickup::decide` (any gun for the gun in hands,
as the carry rules after modifiers 64 and 25 allow); every check of the
install is made, the drop taken, the gun registered under a reserved
registry id of its own (core+18B4Ch ids 195..254, three a place) as the
pending object, and m_need_to_select_animations (+33Eh) set as a weapon
key's changed input does: the game's own switch (tick_active_object 0x2D7650)
holsters the gun in hands on that tick. Fire and reload are cleared from
the applied actions until the swap (E interrupts the firing; the gun ends
its shot). Weapon keys meanwhile are ignored (`pickup.tick_active`);
only the picker's death cancels; past 3 s the game is asked each tick to
look at the switch, never forced. At the switch (`pickup.switch`
0x2D76D0) the gun in hands is put away as the native switch does
(deactivate(1), forget_active_object, in its slot), then the slots are
swapped (set_item 0x2FC620), the gun's ammunition moved into the own gun's
slots (the own stacks parked in the copy's inventory) and rebound, the gun
bound to the core under the slot's reserved id, and set_user, initialize,
the donor's magazine, activate(1). base_player::remove(1) and a loaded
record put back what the record does not name (the held pick put away
first); a re-installed copy that was in hands here gets no weapon call of
ours (the reader set_users and activates it). The crate's checks and use
(`pickup.can_use`, `pickup.use_hint`, `pickup.resupply`,
`pickup.client_resupply`) see the own kit and refill the borrowed gun's
loaded stack by the game's bags rule, for free.

**The bullets.** A bullet record names its weapon by the shooter's slot and
the ammo's ordinal; after a swap the slot holds another gun. Its quiet byte
(+80h) is always written and read (`bullet.save_quiet` 0x32C148,
`bullet.load_quiet` 0x32BE12), and an ordinal past the loading gun's ammo is
its none (`bullet.load_ordinal` 0x32BDBA).

**The local client** (nothing a match computes). At `carry.weapon`
(0x2477C8) the gun of an open drop is kept by place (its transform and the
copy's model box); a taken drop's gun and a picked gun on a dead holder are
taken out of the scene. At `player_icons.update_start` (artefact_carry's
on_hud) the eye's 2.2 m segment into a kept box padded by 15 cm, patch 78's
ray clear, gives `show_artefact_offer("st_weapon_pick_up", name)`; a gun
aimed at that cannot be taken says why (`st_weapon_cannot_carry`,
`st_weapon_one_gun_only`, `st_weapon_no_gun_in_hands`,
`st_weapon_switching`, `st_weapon_already_taken`,
`st_weapon_cannot_pick_up`). At `input_builder.end` Use's press on it is
sent as the command (`weapon_pickup::encode`, `sane`), and stays the
pickup's until Use is let go.

Every decision and every copy's address at each step are logged to
`srp_patches.log`. `check_weapon_pickup_026g.py` runs the hooks on the
game's own code (set_item, remove_item, set_owner, tick_active_object, the
bullet record's branch sites). A row of `simulation_changes.tsv`.

## A weapon shown while aimed keeps its body (patch 91, unreleased)

After a resync the held rifle could be drawn as its sight and the hands alone (the owner's
screenshot, 2026-11-01; `../docs/notes/weapon-show.md`). weapon::on_show (0x243DD0) took an
aimed branch when the weapon's scope view (+181Eh, the world's: a resync gives it from the
snapshot) was on in first person: the aimed sight in, +181Dh (the aimed sight in the scene) set,
and so no body (the body's add at 0x243EFD waits for +181Dh 0). With a red dot (an optic whose
`hide_weapon_on_aim`, +11Ch, is false: every EOTech, Aimpoint, Cobra, Docter and DeltaPoint)
nothing added it back when the aim was released.

**The patch** (`WEAPON_SHOW`, every full build): 0x243E32 `jz near 0x243EC7` (`0F 84 8F 00 00
00`) becomes `jmp 0x243EC7` / nop (`E9 90 00 00 00 90`): on_show always adds the idle sight and
the body, as 0.10b's did, and the next weapon::draw (0x244F80) aims through the game's own
add_aimed_rifle_scope_to_scene (0x244B60), which takes the body out (and hides the hands) for a
full-screen scope alone. Six bytes; no relocation under them, nothing aims inside them.

Drawing only: +181Eh is never written; no row in
`../crates/session/src/v026g/simulation_changes.tsv`; games with and without it play together.
`../docs/notes/client_patch.md` section 3.60 has the proof, and why the body's test stays on
+181Dh rather than on the body's own in-scene byte.

Checked: `selftest_026g.py` check 91; `check_weapon_show_026g.py` runs on_show and the draw's
models with the game's code for the scope (the renderer stubbed with its in-scene byte): the
original loses the body in first person with a red dot, the build keeps it, every other case
ends each draw as the original. **Not run in a game.**

## Shots stay audible under heavy fire (patch 96, unreleased)

The owner (2026-11-05): with a lot of shooting the sound breaks up and the bullets' sound
disappears. The engine's limits, all fixed: the game world's sound scene has 512 sound instances
and 196 propagators (game_world::load 0x1F8DB6, 0x1F8DBD), the world 64 voices (sound_world +68h);
a shot is one instance and one propagator a layer of its composite, and a propagator lives its
layer's length plus its way to the listener. The ported guns' shot composites carry a 9.2 to 9.9 s
tail at every distance (`environments/level_01/automatic/outdoor/rudnya_outdoor*`: Vepr-12,
Saiga-12, Jackhammer, PKP, Glock and the other 0.29/0.69 ports), the AK family a 4.2 s forest
echo. Under automatic fire the tails take every propagator and every voice: create_sound_propagator
(0x5D2A0) returns 0 and the new sound is never heard; create_sound_voice (0x59E70) returns 0 and
the sound waits in silence, the voices given first come first served and taken back only from a
sound under the gain floor (the HDR cull, 0x5CAD4). So the next shots, impacts and steps go
silent until the tails end (the tails a fresh shot hears at an offset when a voice comes free).

**The patch:**

- `SOUND_POOLS`: 1024 sound instances and 1024 propagators (224 KiB of the scene's unmanaged
  allocation instead of 89 KiB).
- The Rust patch `sound_voices`, a lohook at `sound_scene.voice_made` (0x5CAB0, after
  calculate_hdr_audio_frame's call of create_sound_voice; the sound thread): a propagator refused a
  voice takes the voice of the quietest voiced and displaceable propagator of the scene (its sound's
  options' can_be_displaced, +11h, asked of the emitter's slot 2 as the HDR cull asks it,
  0x5CAC3..0x5CACD; true by default, 0x6129F) when that one's loudness
  (its gain by distance, its master's when it has one, times its sample's amplitude now) is under
  half its own. The voice is let go as the HDR cull lets one go (detach_voice 0x5CB80, +48h 2000
  ms) and the world asked again. Tails late in their decay give their voices to new shots; two
  sounds of about one loudness never take a voice back and forth; a quiet sound never takes a loud
  one's; a sound whose raw_options say `can_be_displaced` false never loses its voice (in the
  shipped content: the walk and crouch steps of most materials, the metal, mud, rubber and
  squeaky-wood impacts, and many first-person shot layers, the AK family's 4.2 s forest echo among
  them; the ported guns' first-person tails have no options of their own and are displaceable).

- The bound (`sound_voices_026g.asm`, three `code` hooks): voice_factory::new_voice (0x5E5C0)
  walks its mono or stereo source voices round-robin until one has no owner and never stops
  when all are owned (the count at 0x5E602 is never compared), so the sound thread would spin and
  no sound play again (0.27d0, 0.28a2, 0.31a2 alike; retail 0.46c2's 0x8D1370 walks one pass and
  returns 0). Now one pass, 0 back; sound_voice's constructor (0x631D9) stores the 0 and returns
  without writing through it; create_sound_voice (0x59F8E) lets such a voice go (its sound's
  release as ~sound_voice does it from 0x5A47B, the wrapper back to the pool) and answers 0, as for
  a full pool. With 64 sound voices and 64 source voices of each kind it is not reached today (a
  sound voice owns one source voice at most, its release clears the owner, 0x5A474), the voice
  hand-on included: it is the guard against a leaked owner or a later change of the counts.

Not taken from retail, and why:
- **Retail's loudness sort** (0.46c2 sorts the propagators before the grant loop, 0x8CEC21): in
  0.26g a propagator's grant loudness is its distance gain alone (+4Ch), so a shot's 10 s tail is
  as loud as the next shot of the same gun and a sort cannot put the shot first; and it hands out
  free voices only, while the tails hold all 64. The hand-on uses gain x the sample's amplitude
  now, which tells a fading tail from a new shot.
- **128 + 128 voices** (0.46c2's `snd_max_mono_voices`/`snd_max_stereo_voices`): three numbers
  (the 64 sound voices at +68h and both source pools, 0x58E79) and 192 more XAudio2 voices at all
  times for what the hand-on already gives; a PKP's tails alone (~100) would still fill 128.
- **Guards of the null orders** (free_object 0x5B44A, play 0x5AC9E, OnBufferEnd 0x5DE94 write
  through a failed allocation): those allocations are the world user's order allocator and the
  multi-threaded allocator (0x5DF10: 0 only when malloc 0x106EF0 fails or at shutdown, flag
  0x1F1D61B), one per order or buffer end; neither the bigger pools (allocated once with the scene,
  its unmanaged resource) nor the hand-on (no order) asks for more of them. Out of memory there is
  the game's end anyway.

Nothing a match computes (sounds only). Checked: `check_sound_voices_026g.py` (nine scenes, the
original never takes a voice; a sound not displaceable keeps its voice; new_voice with a free voice and with all owned, the original never
returning; create_sound_voice whole on a made-up world, with a source voice and without), the
self-test's `test_sound_pools`, `cargo test` and clippy of `rust/`. **Not run in a game.**

## The render thread never reaches a Scaleform queue being destroyed (patch 93, unreleased)

Two players' crash reports of revision 40, the same stack, at the switch from a match to the
lobby: the render thread's `flash_renderer::present` (0x2AF0F0) -> `RTHandle::NextCapture`
(0x50FEC0) -> `DrawableImageContext::OnNextCapture` (0x572660) -> `DICommandQueue::OnNextCapture`
(0x5EE080) -> `updateCPUModifiedImagesRT` (0x5ED340), whose EnterCriticalSection of the queue's
lock (+44h) wrote to 0x14: the queue was freed. A stock Scaleform race (0.21d's bodies are the
same): a DrawableImageContext keeps a list of its command queues guarded by its lock (+24h), which
Add/RemoveCaptureNotify (0x5726B0, 0x5726F0) take, but OnNextCapture walks the list without it,
and a queue whose last reference goes on the main thread is destroyed (its destructor 0x5ED1E0
removes it, then frees it) while the walk is at it or about to go to it.

**The patch** (the Rust patch `scaleform_capture`, two lohooks, `sites_026g.tsv`):

- `scaleform.di_next_capture` (0x572675, after OnNextCapture's own ExecuteNextCapture, its
  vtable+4): our walk instead of the stock loop, then its epilogue (0x57269C). Each queue of the
  list whose count is not 0 gets `DICommandQueue::OnNextCapture` with the capture's notify, in the
  list's order, with a reference of ours held across the call; the context's lock is held only to
  read the list and take that reference (a compare-exchange from a count not 0), and the reference
  is let go after the lock is left (`vostok/drawable_image.rs`).
- `scaleform.di_queue_destroy` (0x5ED1E0, the destructor's start): the queue leaves its context's
  list (RemoveCaptureNotify) while its count is still 0; the stock body then writes the count 2
  and removes it again, which finds it gone. So a queue in the list has its true count, and one at
  0 is being destroyed: the walk passes it over.

The lock is not held around the walk's calls (the shape that only locks the stock loop): the main
thread takes it with a queue's own lock held (mergeQueueWith 0x5731F0 releases the other image's
queue with its own queue's lock held, and a last release runs the destructor, which takes the
context's lock), while the walk's call takes each queue's lock: that order would deadlock. Here the
context's lock stays a leaf, as in the stock code. Drawing only: no row in
`../crates/session/src/v026g/simulation_changes.tsv`. `../docs/notes/client_patch.md` section 3.61
has the evidence.

Checked: `check_scaleform_capture_026g.py` runs OnNextCapture from its start and the destructor
past its own removal, the game's Release and RemoveCaptureNotify, on made-up contexts of three,
none, a dying queue, a queue destroyed while the walk is at the one before (the original reaches
the freed queue), and the walk's own reference the last; the lock never held across a call, every
list read made with it, the references and registers as they were. **Not run in a game.**

### 95. A missing animation id is logged (a diagnostic; the next release's revision)

The rev47 crashes at 0x1F18C8 (drawing a player's held weapon) read the animations registry's
ids' vector at 0FFFFh: the registry's lookup (0x2D6590) had no entry for a clip pair, and its
0FFFFh went into the player's animation tree. Which clip, and why nobody registered it, the
dumps cannot say (no heap). `../docs/notes/animation-ids.md` has what is known: the registry,
who registers (`player::register_animations` 0x2D8100 and the match rules, at the world's load,
and a late join's install), the full dump's 221 entries, the owner's crash and the pickup just before it.

**The patch** (the Rust patch `animation_id_log`, one lohook, `sites_026g.tsv`
`animations.id_missing`, 0x2D65EE: the lookup's 0FFFFh exit, its `pop edi / pop esi / mov
eax,0FFFFh` replayed): each distinct missing pair, at most eight a run, is written to
srp_patches.log: the clips' addresses and names (the resource's name registry entry, +B0h, its
name +8), the registry's id count, the resolver's return address and the six stack words above
its arguments. The lookup still returns 0FFFFh: nothing a match computes changes, no row in
`../crates/session/src/v026g/simulation_changes.tsv`.

Checked: `check_animation_id_log_026g.py` runs the lookup from its entry on a made-up registry
in both exes: a registered pair (its id, no line), a missing one (0FFFFh, the registers kept, the
lines), the same again (no line), a pair of two resources, a resource without a name, a ninth
pair (no line). **Not run in a game.**

## A world takes no input whose rotation no client makes (patch 99, unreleased)

0.26i fixed "crashes of all clients in the game associated with sending 'bad' input of one of the
clients": from 0.27d, `game_world_core::change_input<player_input>` refuses an input whose
rotation (two half floats) is a NaN or an infinity before it touches the world ("change input
(client %s) request REJECTED, reason: input is invalid", event action 6). 0.26g's (0x1FAB50) takes
any, and every world then adds it to that player's angles for good. The match server already
refuses such inputs (`PlayerInput::rotation_is_sane`); this is the client's twin.

**The patch** (the Rust patch `input_check`, one lohook, `sites_026g.tsv`): `change_input.player`
(0x1FAB50, the function's start; every input a world takes comes through it: local, relayed,
moved, a replay's). An input whose two halves are each a number under 1024 radians goes on; any
other is refused as 0.27d refuses it: EAX 6, the exit `refused` (0x1FAC4E, its `ret 24h`), nothing
of the world written, a line to `srp_patches.log`. A refused local input is never sent. The bound
is the server's (a superset of 0.27d's): the server never relays an input it refuses, so no
relayed or replayed input is refused here that the server took.

Every world refuses the same inputs, so they stay the same, but a world given one now plays on
without it: a row `next` in `../crates/session/src/v026g/simulation_changes.tsv`.
`../docs/notes/client_patch.md` section 3.62 has the evidence.

Checked: `check_input_check_026g.py` runs change_input's start on rotations a mouse makes (the
build at 0x1FAB56 as the original, every register the same) and on NaNs, infinities, 65504 and
±1024 as either half (the original keeps them; the build returns 6 with its arguments popped, the
callee-saved registers kept, nothing written, the line logged). **Not run in a game.**

## A side mouse button is no left click in the menus (patch 102, unreleased)

The mouse gives every listener the key 151h + its button. The five handlers that pass a mouse key
to their Flash movie (the lobby, the login screen, the options, the match's menus and the swf
input translator) turn 151h/152h/153h into the movie's left/right/middle and every other key into
the left button: the side buttons clicked whatever was under the cursor (0.60's "pressing
additional mouse buttons in the lobby counted as pressing LMB"). `MOUSE_BUTTONS` retargets each
mapping's `jne` past 153h to the handler's own return after the movie's call (five 2-byte writes):
the key is taken, the movie gets nothing. Nothing a match computes. Addresses: `client_patch.md`
3.63; checked with `check_mouse_buttons_026g.py`; not run in a game.

## A step first made by a re-simulation is heard (patch 101, unreleased)

The players' "footsteps randomly drop out" (B20). `on_player_sound_event` (0x2A6470) plays a
step only when its time is past the newest tick simulated once (`maximum_time`, core +18FC4h,
compared at 0x2A6488). A game re-simulates another player's ticks when his input comes late, and
a step that only the re-simulation has (he started, stopped or changed pace) falls at a played time
and is never heard. 0.27d0 dropped the test (its effect plays each foot's newest step time at the
next frame).

**The patch** (the Rust patch `footsteps`, one lohook, `player_sound.step_guard` at 0x2A6485):
a step past the played time plays as before; a re-simulated one plays when it is less than 250 ms
before the played time and the same foot of the same player object played nothing in the 250 ms
before it (a copy of a played step, moved by the correction, stays single; a long re-simulated
window plays each foot's newest step at most). Rustles and the other tests are the game's. Nothing a match computes.
The rules and the 0.27d0 code: `../docs/notes/footsteps.md`; the addresses: `client_patch.md` 3.64.
Checked with `check_footsteps_026g.py`; not run in a game.

## The sound follows the audio output device (patch 100, unreleased)

Players' report (B43): when the audio output device goes or changes (a headset unplugged, a
Bluetooth headset connecting, the default device switched), the game has no sound until it
restarts. 0.26g makes its XAudio2 2.7 engine and mastering voice once, on the default device of
that moment (`sound_world::initialize_xaudio` 0x59080), registers no engine callback and watches
no device; after XAudio2's critical error (its device gone) the engine is stopped for good, and a
new default device is never used. XAudio2's only way back is a new engine, and a mastering voice
goes only after every other voice, so 0.69 (`stop_mastering_voice` / `restart_mastering_voice`)
makes every voice again; this patch does the same for 0.26g.

**The patch** (`audio_device_026g.c`, `audio_device_engine_026g.c`; the arena's blocks
`audio_device.code` and `.state`; `AUDIO_DEVICE_HOOKS`):

- initialize_xaudio's two XAudio2Create calls (`call 0x5A640`, 0x59187 and 0x591C8):
  `audio_device_create` makes the engine and gives the game our proxy of it; each voice made
  through it is a proxy of ours that forwards every call and keeps what it set (sends, effect
  chain and parameters, volumes, matrices, filters; a source's format, callback, frequency ratio,
  started state and the buffers whose OnBufferEnd has not come). An engine callback of ours
  (OnCriticalError) and an IMMNotificationClient (OnDefaultDeviceChanged, render, not
  communications) raise event bits.
- sound_world::tick's process_orders (`call 0x59DD0`, 0x59AAC, the sound thread):
  `audio_device_orders`, then process_orders. After an event (a default change only when the
  default device's id is another), a rebuild: a new engine and a mastering voice on the device the
  game's rule picks first (on failure nothing changes, tried again 2 s later, quietly); then the
  old voices, master and engine go, and every voice is made again from what was kept, the pending
  buffers queued again in order (a flushed one, which the game makes at every detach_voice and a
  dead or stopped engine never ends, is ended by the rebuild itself and never played again), the
  game's pointers unchanged.

New engine-function rows: `engine_xaudio2_create` (0x5A640, cdecl), `engine_process_sound_orders`
(0x59DD0, stdcall), the imports `engine_co_create_instance` (the delay-loaded slot 0xAF0064),
`engine_heap_alloc`, `engine_get_process_heap`, `engine_heap_free`. Sound only: no row in
`../crates/session/src/v026g/simulation_changes.tsv`. The evidence and the spec:
`../docs/notes/audio_device.md`.

Checked: `test_audio_device_026g.c` (its header has the command; i686 gcc, ASan/UBSan) runs the
core against a made-up XAudio2 that keeps XAudio2's rules: the game's graph rebuilt the same after
a lost device and a new default, nothing on the same default or a communications one, kept and
retried with no device or no engine, every buffer ending once, everything given back at the end.
The exe built and its three sites disassembled. **Not run in a game.**

## A world up to 0xF000 bytes, its sections on the heap (patch 98, unreleased)

A 10v10 bot match of revision 47 stopped every game in it: twenty players
and some 93 buckshot pellets in flight made a world of 33.3 KB, and
`game_state_history_item::hash` (0x325150), which serializes the world into
0x8000 bytes of its stack with no allocator for every hash report, asserted
in `mutable_buffer::reallocate` (0x7AB70, "mutable buffer overflow"). Behind
it, the history item's bullets section holds 0x2400 bytes inline (about 133
bullets in flight), so two full-auto buckshot shotguns overflow it in
game_world_core::serialize.

- hash(): 0x325176 `mov eax,8000h` and 0x32519D `mov dword [ebp-0Ch],8000h`
  (the stack allocation and the buffer's capacity) become 0xF000 (every full
  build): what the upload's u16 total holds, 128 chunks. The upload's own
  buffer (`snapshot_upload_026g.asm` `BUFFER_SIZE`) is 0xF000 too, and so is
  the 0xE8 handler's unpack buffer (0x27C059 and 0x27C07C, the same two
  immediates), so the server keeps every world packed
  (`COMPRESSED_SNAPSHOT_BUFFER`): a raw world past 0x8000 bytes would be some
  100 parts, each holding one of the client's 256 pool packets until it
  completes. The game thread's stack is 8 MiB; on_hash_mismatch takes
  0x10000 bytes of it.
- The sections (with the late join): a jump at the end of
  `game_state_history_item::construct` (0x3246C0, its last store) to
  `world_sections_026g.asm`, which gives each of the six world sections'
  buffers (bullets, statistics, objects, physics, rules, effects) the global
  multithreaded allocator (0xA8001C) and a block of its inline capacity
  (through reallocate). They then grow as any buffer with an allocator does;
  the item's destructor already frees such a buffer. Nothing reads the
  inline bytes but through the buffer's data pointer, so the item's layout
  stays.
- Patch 87 (crate `world-frame`) refuses a body over 0xF000 bytes instead
  of a section over its inline buffer; the match server reassembles uploads
  of up to 0xF000 bytes (`upload::MAX_TOTAL`, the transport's
  `MAX_SNAPSHOT_TOTAL`).

Worlds are byte-identical: nothing a match computes changes, no row in
`../crates/session/src/v026g/simulation_changes.tsv`. What stays: a world
over 0xF000 bytes (some 550 bullets with twenty players), a player state
over its inline 0x1000 (none past 0x3A8 seen), and the objects item's
second, unserialized buffer (inline 0x2000 at 0xBDF58, which
game_world_core::serialize fills each tick). `../docs/notes/client_patch.md`
section 3.66 has the evidence. `check_world_sections_026g.py` runs the
constructor, the sections' writers, hash, deserialize and destroy of both
exes on made-up items: the rev47 world (0x818A bytes), 300 bullets with
twenty players, a world over 0xF000, and a packed 0xE8 of a 0xA014-byte
world through the whole handler with the game's own decoder. **Not run in a
game.**

## A bullet meets a player where he stood when it crossed him (patch 103, unreleased)

Bullets are ticked on the 50 ms grid only (`bullet_manager::tick` 0x321D10): at a grid step a
bullet's one ray spans its flight since the last step, and every player is tested where the step
left him, up to 50 ms after the bullet passed (25 cm at a run). The patch (the Rust patch
`subtick_hits`, one lohook at `bullet.ray_traced` 0x32D627, first of the chain the ragdoll and the
lamps share) tests each player near the ray where he stood at the ms the bullet reached him: the
game's own hit time of the ray's point nearest him, and back along his character controller's last
step (`m_last_pos` to its position, world state) by the share of the step since then, at most the
step's whole move. A ray whose closest hit the game gave is a hit model is traced again for the
level alone (the game's ray_test, mask 8), then each body near the ray is tested by Bullet's
rayTestSingle (0xA02D0) through the game's own closest-ray callback (0x75B40, its filter and its
hit model delegate) at a copy of its transform: nothing of the physics world moves. With no player
moving near the ray the game's result is left as it is. The rules: `../docs/notes/subtick_bullet_hits.md`;
the evidence: `../docs/notes/client_patch.md` 3.67.

It changes what a match computes: a row `next` in `../crates/session/src/v026g/simulation_changes.tsv`.

Checked: `cargo test` (the crossing time, the share, the clamps, a split step, a bullet spawned in
the step, a ray of no length, the clock's wrap, the controller's move in the game's frame); `check_subtick_hits_026g.py` (and under
`align_probe_026g.py`): check_collision's ray test from 0x32D5E5 to 0x32D631 in both exes on
made-up spheres, the game's own Bullet code end to end; a still target byte for byte the
original's, also with players moving far from and near the ray; a target moving across the ray
that the original misses is hit as the original hits him standing where he stood; walls in front
and behind, the shooter's own body, a target spawned in the step or dead, a step in parts, a move
along the game's +z put back behind his step's end (the controller's positions are Bullet's, z
negated) and a diagonal one; every byte of the made-up world the same after the run. `check_ragdoll_026g.py` and
`check_lamps_026g.py` (the site's other hooks) pass. **Not run in a game.**

## A thrown grenade stays above the ground (patch 105, unreleased)

Grenades often went through the ground where they hit it (players' reports,
2026-11-04). Stock 0.26g: `grenade_core::load` (0x303890) gives the grenade a
0.05 m capsule and continuous collision with a swept sphere of 0.01 m; vostok's
`integrateTransforms` moves a body whose sweep hits to the sphere's time of
impact, which leaves the capsule 4 cm or more inside the level's one-sided
collision mesh, and its contact then often pushes it through. `GRENADE_CCD`
(0x303A13) loads the 0.04 of 0xA2D7DC instead of the 0.01 of 0xA2D774 (8
bytes, the same instruction; its base relocation stays): the sphere is inside
the capsule with a centimetre to spare and the capsule stops at the surface.
A simulation of the game's world, step and body over vostok's Bullet 2.79
(`grenade_ccd_sim.cpp`) loses 10 to 23 % of throws under the ground stock and
none of 4700 with 0.04; a grenade rolls on about half as far after it lands
(stock, the sunk capsule was pushed back out with speed). What a match
computes changes: a `next` row in
`../crates/session/src/v026g/simulation_changes.tsv`.
`../docs/notes/grenade_ccd.md` has the evidence. Checked: selftest check 105
(`test_grenade_ccd`) and the late join's site and relocation checks.
**Not run in a game.**

## Chat text follows the keyboard layout (patch 107, unreleased)

Under Wine, Alt+Shift never reached the chat (the owner's report, 2026-11-05, Wine's Wayland driver
under niri). `keyboard::translate_text` (0x3B92D0) types a key on the engine's logic thread with
`GetKeyboardLayout(0)`, that thread's layout. The window procedure (0x181E0) gives the logic thread
a new layout with `ActivateKeyboardLayout(hkl, KLF_SETFORPROCESS)`, and only while a text input
has the keyboard: it drops a layout request otherwise. Wine ignores the flag, so the logic thread
keeps its first layout. With the X11 driver this does not show (its text ignores the layout); with
the Wayland driver the chat stays in the US layout.

**The patch** (the Rust patch `keyboard_layout`, three lohooks, `sites_026g.tsv`): the window
procedure records the HKL of every WM_INPUTLANGCHANGEREQUEST (`window.lang_request`, 0x1835F)
and WM_INPUTLANGCHANGE (`window.lang_change`, 0x183E3), before the game's text-input check, in one
atomic. translate_text types with it (`keyboard.text_layout`, 0x3B92E2), or with the thread's own
until one is recorded. X11: unchanged. Wayland: Alt+Shift switches the chat. Windows: a switch made
outside a text input carries over to the next chat. Nothing a match computes. The rules:
`../docs/notes/keyboard_layout.md`; the addresses: `client_patch.md` 3.68. Checked with
`check_keyboard_layout_026g.py`; not run in a game.

## The desktop's moved keys (patch 110, unreleased)

Since revision 49 (#1240, Wine's Wayland driver under niri) a key the desktop moves does not move in
the game: with xkb's `caps:swapescape` the Caps Lock key stayed Caps Lock. The game reads the
keyboard through DirectInput's buffered events (keyboard::execute, 0x3B8D20). Wine's X11 driver
made each key's scan code from its keysym, so moved keys arrived moved; the Wayland driver gives the
physical key's scan code (`key2scan` of the evdev code) and its virtual key from a fixed QWERTY table,
so neither DirectInput nor WM_KEYDOWN sees the move. Only the layout's key names (each physical key's
keysym, `GetKeyNameTextW`) and its text say it: under `caps:swapescape` the Caps Lock key is named
`Escape`.

**The patch** (the Rust patch `key_remap`, one lohook, `sites_026g.tsv`): at `keyboard.events`
(0x3B914B, after GetDeviceData, before the loop that sets the keys' states and before process), each
event's key among the 14 function keys (Esc, Backspace, Tab, Enter, the Ctrl, Shift, Alt and Windows
keys, Caps Lock, Menu) becomes the function key its name says; letters and digits stay physical, as
on Windows. The names are DirectInput's (the game's own get_dik_name, 0x3B9C00) in the layout patch
107 recorded, lent to the polling thread for the reading (`ActivateKeyboardLayout(hkl, 0)`, then its
own back), once a layout. Under Windows (no `wine_get_version` in ntdll) and before a layout is told
nothing moves. X11: the names are the moved keys' own, so nothing moves twice. Each client's own
keys: nothing a match computes. The rules: `../docs/notes/keyboard_layout.md`; the addresses:
`client_patch.md` 3.69. Checked with `check_key_remap_026g.py`; not run in a game.

## The mouse look's pitch: 16:9's at every window, and "Lock vertical sensitivity" (patch 112, unreleased)

Players' report (Discord suggestions, 2026-11-03): the mouse is right only at 4:3; at 16:9, looking
up and down is slower than turning. `player_input_handler::on_mouse_move` (0x240200) turns the yaw
by x × π/180 × s radians and the look pitch by y × π/180 × s × h/w × 3/π units (h/w is the render
window's). A unit is 60° of view up and 67.5° down (the look clip `on_site_camera_look`, linear on
each half; decoded again). So the pitch turned 0.75 of the yaw at 4:3 (0.84 down), 0.5625 at 16:9
and 0.42 at 21:9. 0.69d0 keeps the same formula.

**The patch** (the Rust patch `mouse_pitch`; `sites_026g.tsv` `mouse.pitch_step`,
`mouse.death_pitch`): a lohook at on_mouse_move's pitch step (0x2402E9, 18 bytes), and the owner's
design (2026-11-05) in Settings, Controls, under Mouse sensitivity: "Lock vertical sensitivity"
(ticked by default) over a slider "Vertical sensitivity".

- **Ticked (locked):** the stock formula with h/w fixed at 16:9's 0.5625 at every window. A 16:9
  window turns bit for bit as the original game; every other window gets 16:9's feel (4:3 25 %
  slower vertically, 21:9 +33 %). The death camera and the warmup camera use the same constant.
  The greyed slider shows Mouse sensitivity × 0.5625 (1.25 → 0.70) as the main slider is dragged.
- **Unticked:** exactly 1:1. A count's pitch radians equal the yaw's at the vertical value. The
  factor is 1 while the local player's look pitch is up, 60/67.5 while it is down, and π/3 in the
  warmup camera's orbit; the death camera's pitch step equals its yaw step. Unticking starts the
  value at what the greyed slider showed.
- **Storage:** `srp_mouse_vertical_locked on|off` (on by default) and `srp_mouse_vertical`, in
  user.cfg. They are registered at `configs.start` and answered at `options.callback`
  (`root.srp_check_option`, `root.srp_slider_option`), and read once per move into one enum,
  `Pitch` (`Same` | `Split`).
- **The menu** (main_menu: `SrpMouseVertical`, `Settings.layoutControls`): Invert mouse and the
  Lobby keys move down under the new rows. The Chat keys move to the right column under the
  game-key list, which scrolls. The owner has not yet decided on this layout.
- The input carries the turn, not the mouse's counts: nothing a match computes changes.

The rules and the decoded clip: `../docs/notes/mouse-pitch.md`; the addresses: `client_patch.md`
3.70. Checked with `check_mouse_pitch_026g.py`, `check_death_camera_026g.py`, selftest checks 112
and 46, `../scripts/hud-options/check.sh` and `cargo test`. Not run in a game.

## Native deserialization bounds (patch 115, unreleased)

Extracted from PR #478 into generated hooks on the current Rust framework.
A missing FSM state skips its virtual reader; an anomaly id outside its state
vector takes the native no-state path; an artefact id outside the world registry
takes the native null-object path. Valid ids execute the original instructions.
These guards also cover item bodies whose format cannot be walked statically.

`check_deserialize_guards_026g.py <stock exe>` compares valid reads with stock and
checks invalid/empty tables at all four stack alignments (84 cases). The i686
patch crate's i686 tests and Clippy pass. No release revision is reserved.

## A presenter ignores an effect it does not hold (patch 117, unreleased)

The 2026-11-06 crash (rev50, `abort` from the allocator under
sound_game_effect_presenter::on_event 0x2A9EF0). `base_player::remove(1)`
(a respawn, a departure, a snapshot's kit change) clears the player's
presenter (0x2DA81C, vtable +4) while its game_effect_player plays on; a
damage zone stops its effect on the player when its sensor sees the player
gone, the tick after (damage_zone_core 0x31A210 -> game_effect_player::stop
0x2DE350). The sound and particle presenters' `on_event` search their
entries for the effect and erase the one found; a search that finds none
leaves its index at -1 and the erase starts one entry before the list: the
sound presenter's own fields are released as an entry (the word at its
capacity pointer's +28h counted down, its random seed at +10h freed as an
effect reference: the crash), the particle presenter's likewise; the sound
presenter's update reads entry 0. The patch (`effect_presenters`, three
lohooks after the searches) returns from `on_event` when the search found
nothing, with a line in `srp_patches.log`; the native code runs unchanged
when the entry is there.

`check_effect_presenters_026g.py <stock exe>` runs both presenters (made by
the third-person presenter's constructor) on a stop and an update of an
effect they hold and of one they do not, in the original and the build, at
the four stack alignments (64 cases): the original frees the seed, faults or
erases before the list; the build returns and changes nothing; a held
effect's stop and update are the original's byte for byte. Not played.

## Match-message checks (patch 114, unreleased)

Before dispatching a server message, the client checks fixed lengths, player
indices, relayed input numbers and compressed-output bounds with `srp-validate`.
A refusal consumes the payload and skips its native handler through the existing
dispatch exit. It does not close the match. Bad world/handshake envelopes request
recovery through patch 97; other bad messages are discarded. Valid messages follow the original dispatch table. The hook precedes the
id subtraction, avoiding the table jump's base relocation.

This checks the 0xE4 envelope and late-join proposal header; the existing late-join
validator still checks its profile. It does not yet connect the shared static
options parser to the decompressed 0xE4 reader. The server does not emit 0xED/0xEE
batch inputs; those remain refused, as in the source PR.

Validation: 112 native dispatch cases, the i686 patch tests, current server encoder
regressions (including Blackout, late join and pickup commands), and Clippy.

### Semantic world checks (extension of patch 87, unreleased)

Both existing load hooks now use `srp-validate` for the player, bullet,
statistics, rules, physics and effects checks. They read mode, battery count and
respawn count from the live core's rule objects. Research is identified by its
owned native rule type 5, so no relocated vtable address is hardcoded. Missing,
duplicate or malformed rule context refuses the load.

The shipped frame parser still owns framing, and patch 85 still prepares pickup
resources after acceptance and before native deserialization. Unknown profile,
inventory and map-object layouts are not guessed into restrictive bounds; the
native per-object bounds remain at their readers. This is not a complete parser
for every opaque native item body.

`check_world_check_026g.py --drift <srp-validate drift_dump.tsv>` compares host and
i686 verdicts using the same match context. `check_world_roundtrip_026g.py` keeps
the historical TDM corpus; `--contexts <drift_dump.tsv>` supplies context for other
corpora. Pickup-only fixtures now contain valid current snapshot sections.

## Recipient recovery (patch 97, unreleased)

Deploy the server receipt support before this client. D8 capability 0x82/0x83
negotiates application-level load receipts; old 0x80/0x81 clients retain their
existing server behavior. A transport ACK alone does not admit a new recipient.
After native world deserialization returns (0x27C116), the client sends Loaded;
a refused world sends Rejected without changing simulation state. The server
retries only that connection with a newer generation, at most once per second;
a missing receipt triggers a retry after two seconds. Corrupt E3/E4 envelopes
request a resend of that connection's pending handshake.

Receipts use a distinct impossible E0 upload prefix, match seed and generation.
Stale, duplicate and unsolicited receipts cannot reset another player or accuse
a donor. A receipt is sent once per event, never dropped for time: a world's
Loaded or Rejected once per generation (a flood of one corrupt world sends one;
the next generation's refusal goes at once, however soon), a Rejected handshake
once per refused E3/E4 (not for a refused late-join proposal), and an Unreadable
for a world whose generation cannot be read (its reader outside its buffer, or no
byte), both numbered 1, 2, ... on their connection in the generation byte; the receipt rides E0, which the transport
resends every 500 ms until acked, and the server tells a copy from a new refusal
by that number or generation. A receipt the packet pool has no room for waits (the
newest only) and goes on the next frame with room (`game.tick`, order 2). Every
refused message and world gets a line in `srp_patches.log`: at most one a
second for the game, each counting the refusals since the last line
(`vostok::log::refusal`). The server logs each refused world or handshake a
client reports, and each receipt it ignores, the same way.
The existing packet-pool reserve is checked before allocation. Match close
(0x27A790) clears the owner; reused pointers/seeds cannot retain a stale budget.

`check_packet_recovery_026g.py` exercises 2,000 malformed messages, recovery,
post-deserialize receipts, early handshake, full pool and teardown. The server
suite covers first worlds, resyncs, lost reports, repeated refusal, forged/stale
reports and subsequent successful play. This is not a transport flood defence
or a replacement for a real-match release-candidate test.
## The hud's kits and cursor (patch 113, unreleased)

The owner's two reports (2026-11-06), both before a match's start, with the kits shown:

- **The last match's kit on the tab.** A match where kit 2 was picked, then the next match: kit 2's
  tab chosen, kit 1 played. The hud (`game_world_ui`, inside the game's one game world, game+18h+1FCh)
  lives as long as the game, and its kits' state (+2DFh) and cursor (+2DCh) are written only by its
  constructor, `show_profiles_selector` (0x2383B0) and `show_cursor` (0x23A930). Patch 31 keeps the
  kits shown until play, so a match that ends with them shown (left before the start, dead at the
  end) leaves +2DFh set. The next match's `show_profiles_selector(1)` then returned at once
  (0x2383C7), and its `root.select_player_profile` with the place's next kit (0x238422, from
  0x1F8120: the lowest loaded kit at a match's start, `../docs/notes/match.md` section 19) never ran;
  the hud movie, the last match's too, kept that match's tab.
- **Esc doubled the cursor.** Esc (`game::activate_main_menu`, 0x1EDBC0) shows the main menu
  (`game_options`, game+57F4h, shown +44h) with a cursor movie of its own (+10h). The hud's cursor
  movie (+0Ch, in the game world's scene) stayed drawn under it, still, where the mouse last was.

**The patch** (the Rust patch `hud_cursor`, four lohooks, `sites_026g.tsv`):

- `hud.initialize_match` (0x234880, once a match; the late join's rebuild enters at 0x234E71): the
  cursor hidden (`show_cursor(0)`) and +2DFh 0, as the constructor leaves them. The hud movie's
  `HUD.reset`, which initialize_match invokes at 0x234AE8, hides its kits too
  (`show_profiles_selector(false)`, as its constructor). The match's first showing of the kits is
  then a whole one, with the right tab.
- `options.activate` (0x200F50): a closed menu about to be shown with the hud's cursor shown
  (+2DCh), in a match or not, takes its movie off (base_game_scene::hide_movie 0x23DC70). activate
  returns at once when the menu is shown already (0x200F62), and so does the patch.
- `hud.cursor_movie` (0x23A992): while the menu is shown, `show_cursor(1)` (a death under the menu,
  the kits again) writes its flag but does not show the movie.
- `hud.cursor_hide` (0x23A9B0): while the menu is shown, `show_cursor(0)` (a spawn under the menu,
  the next match's start) writes its flag but does not take off the movie, which is not there.
- `options.deactivate` (0x201040): a shown menu about to close gives up a key its settings await
  (below, "The menu's own cursor"); then, with the hud's cursor shown, it puts the
  movie back (show_movie 0x23DC00), over a running match and at its end alike (a match's end clears
  +D4h, then closes the menu: 0x1F952B, 0x1F956D).

So the hud's cursor movie is in the game world's scene exactly when +2DCh is set and the menu is
closed, as in the original without the menu. The hud's flag is the game's throughout, so the mouse
goes where it went. Nothing a match computes. Checked with `check_hud_cursor_026g.py` (each site
against the original, and the game's orders of calls on a scene's movie list), `cargo test` and
clippy. Not run in a game.

**The crash it had (rev51 to rev54, exe+0x14EE8D).** The first build took the movie off only over a
running match and put it back only over one, so a match that ended under the menu (Esc over the
kits or the death screen, then Leave, or the match's end) left +2DCh set with the movie off the
scene; under the menu, show_cursor(1) set the flag without the movie, and show_cursor(0) ran the
stock hide. So the next `show_cursor(0)` after a match's end (the next match's `initialize_match`)
or under the menu (a spawn, the start of play) took off a movie the scene did not hold. The scene's remove_movie (inlined in world::hide_movie 0x118FE0, as 0.21d's
scene_view::remove_movie) erases what std::find (0x3C5510) returns, and for a movie not there that
is the end: buffer_vector::erase (0x15EE30) then copies from end+1 until it meets the end, which it
never does, and writes through the list's inline buffer until it meets a word that is not a pointer
(write to 0x17789 live on rev52, 2026-11-07; to 0x2A2 locally on rev54, 2026-11-09). Both on the
render thread at dispatch_callbacks (0x1024F3), about 0.2 s after a match's players were made. The
local full dump: the scene (game world +8, 0x41194868) holds the hud movie and the player icons
(begin 0x411B25A4, end 0x411B25AC, buffer of 32), the command's movie is the hud's cursor movie
(hud +0Ch, 0x7C1214D8), +2DCh is 0 (the hide had run); the server log has that game leave the match
before its start "by its own word" (the menu's Leave), then its next match's load.

**The menu's own cursor (the same crash, from the stock game).** The main menu's key-binding
settings take the menu's cursor movie (game_options +10h) off the scene while they wait for a key:
the movie's `start_bind_key` (scaleform_callback, 0x1FF889) sets the awaited action (+3Ch, 48h none)
and hides +10h (0x1FF8A9). Only a key ends the wait: on_keyboard_action (0x2015E0; Esc too, DIK 1 at
0x2010F2), on_mouse_key_action (0x2016D0) and on_mouse_move's wheel (0x2017A0) hand it to
process_key_input (0x2010E0), and finish_binding (0x2015A0) or its inline copies (0x2016FB,
0x2017D9) or a conflict's message (0x201343) put +10h back and set +3Ch 48h. While it waits every
press is the wait's, so neither Resume, Leave nor Esc can close the menu, but deactivate's other
callers can, each a scene's on_deactivate as the game switches scenes (switch_to_lobby 0x1ED23B,
switch_to_game_world, switch_to_login, clear_resources): a match's end, a kick or a lost connection
(game_world::on_deactivate, 0x1F956D) and a match starting over the lobby's menu or the lobby's
fall to the sign-in (lobby_menu::on_deactivate, 0x21D50E). deactivate (0x201040) hides +0Ch and
+10h whatever +3Ch says (0x20104D, 0x201058) and leaves +3Ch, so the second hide takes off a movie
the scene does not hold: exe+0x14EE8D, with the queued command's movie game_options+10h. 0.21d has
the same code (scaleform_callback 0x15DD31, finish_binding 0x15F5E9). No report has shown it yet:
the three local full dumps at exe+0x14EE8D (2026-11-09) all hold the hud's cursor movie (hud +0Ch,
0xA96A68) in the command, and game_options (0xA9C03C) +3Ch is 48h in each.

`options.deactivate` now gives an awaited key up before the stock close, as Esc gives it up:
finish_binding (`root.end_keybind`, which closes the movie's "press a key" message; +10h back;
+3Ch 48h), then deactivate takes both movies off. The show and the hide go to the render world in
one queue (0x1F2C0 on +200058h), in that order. So the menu's movies are in the scene exactly while
it is shown, +10h only while no key is awaited, and a closed menu never awaits a key. A conflict's
message (two actions on one key) open at the close is left as the stock leaves it: its +10h is
back already, and its OK or Cancel at the next opening still works. Checked by
`check_hud_cursor_026g.py` (steps 7 and 8: the original takes +10h off twice, the patched exe never).

## A ban's reason and end on the sign-in screen (patch 116, unreleased)

A banned account's sign-in said "You are banned. " and one of the game's 14 reasons, never for how
long, and the reasons could not say what srp bans for (pressing the players who don't farm). srp
now has its own reasons (`../crates/ban-reason`: the list, each a dictionary key
`st_srp_ban_reason_<name>` in every language, and the answer's bytes). The login server takes one
from the admin's free text by its words (`BAN_WORDS` in `../servers/login-server/src/v026g.rs`;
`farm` first, none: "Decision of the administration.") and answers a banned account with the game's
closest code, then the reason's id and, for a ban that ends, its seconds left (u32 LE), in the same
TLS write. The game reads that answer with one read of at most 52h bytes and dispatches on byte 0
alone (0x37FB47), so a game without the patch shows its own reason as before. A banned address is
answered in the plain step, one byte, and gets the game's text.

**The patch** (the Rust patch `ban_time`, two lohooks, `sites_026g.tsv`):

- `login.other_answer` (0x37FB66, on_sign_in_answer_received for every answer but 08h and 25h):
  a ban's code with srp's tail keeps the reason and the end (now + the seconds, a FILETIME, or
  never); any other answer clears them.
- `login_menu.status_text` (0x21F217, set_status's join once the text is built): a ban's status
  (0Ch..19h) with a reason kept is written anew: `st_login_menu_status_user_banned`, the reason,
  then `st_srp_login_ban_forever` or `st_srp_login_ban_ends` with the end in the machine's time zone
  (`14:10`; another day `08.11 14:10`) and the time left, rounded up to the minute
  (`st_srp_unit_minutes`, `_hours`, `_days`: `29 min`, `2 h 15 min`, `3 d 4 h`). English: "You are
  banned. Harassing or team-killing players who don't farm. Ends at 14:10 (in 29 min).".

`release/moderate ban` prints the reason the player will see. Nothing a match computes. Checked
with `cargo test` and clippy (patches, ban-reason, login-server). Not run in a game.

## Resistance: isolation against anomalies (patch 109, unreleased)

The artefact tree (`../docs/notes/artefact-skills.md`). Its skills the exe computes are packed in
kit modifier 40 (`artefact_power`, read by nothing of 0.26g), parsed once (`artefact_skills::Skills`).
Resistance (502, 5 ranks): a hit of type 4..7 (toxic, radiation, fire, shock) on a part takes
its summed armour + 0.05 a rank (+5 isolation points) at `artefact.isolation` (0x32A083, after
`movss xmm0,[eax+8]`), before the piercing's subtraction and the clamp. Other types unchanged.
Onyx II: `artefact.landing` (0x2D8220, the landing's start: all it does is the fall damage) exits at its
`ret 4` (0x2D84AE) for a player with the skill and an Onyx of its containers in state 4. Larkspur II:
at `medical.artefact` (0x313333, the actual activation; after Medic's handler) a Larkspur's holder
with the skill gets its stamina's maximum (+6C0h, current +6Ch). Spring II: patch 54 gives each
Spring two reload nodes (list 13, +3D8h and +3E8h; the object 3F8h), the first linked with the
passive (new hooks at 0x311AD0 / 0x311D10), the second with the active; patch 109's entry
`artefact.spring_reload` sets their values at the passive's link (+0.05 / +0.10 with the skill, else 0).
Resonance and Cascade (bits 4 and 8, an either/or pair): one aura, read where each effect is used and never
kept: a living teammate's carried Springs, Lifebones and Onyx give a recipient their passives
within 9 m (Resonance) and their actives within 6 m (Cascade), at most 4 copies of a kind for
each, the recipient's own first. The Springs' nodes at the two getters (`artefact.shared_modifier`
0x2D5AE6, `artefact.shared_modifiers` 0x2D5C18; the kit's player found in the match of the engine
world's game's world), the Onyx before a part's protectors (`artefact.shared_protection`
0x32A020), the Lifebones at a part's regeneration tick (`medical.regenerate`, before patch 94's);
Every kind's activation (its glow on the recipients for the active's length, a Lifebone's heal) is
patch 94's Medic (`share_active`, every kind since the owner's rev51 games, before a Lifebone's only),
whose bit (modifier 43 bit 20) skill 542 sets.
The shared movement (list 4) is read only when a player's animations are selected, so at each
player's tick before tick_animations (`battery.carrier_tick` 0x2DAAD3, order 2) a change of it
since the tick before sets +33Eh (m_need_to_select_animations), as patch 94 does for Adrenalin;
each place's last 64 ticks' sums are kept, so a re-simulated tick answers as at first.
At most 4 copies of a kind count for a player, own copies included (the owner, 2026-11-07; the
stock game sums every carried copy's passive): of a player's own copies of a kind in their passive
or active, in container order, the first 4 count, the one count the sharing's room and the leg
quarters use (`vostok::artefact_skills::own`). One past them gives no passive: the getters' list
sums (`artefact.own_modifier` 0x2D5B0C, `artefact.own_modifiers` 0x2D5C46) leave out its passive
nodes (+340h, +350h, +3D8h), summing the list's other nodes as the game does and going on at the
loop's end; an Onyx's protect in its passive returns (`artefact.own_onyx` 0x312BF0, `ret 0Ch`);
a Lifebone's regeneration modifier returns (`artefact.own_lifebone` 0x312590, ECX the Lifebone,
`ret 8`). Nor its active: patch 54's `ready` (artefacts_026g.asm) asks the entry `artefact.fire`
(cdecl: the copy, its fires pushed 1, made 0 for one past the 4), so a press fires at most the 4
that count, a fifth stays ready (state 3, no cooldown) and adds no Onyx time; each carrier shares
only its copies that count (`shared`).
Checked with `check_artefact_skills_026g.py` (`check_own_cap`, `check_active_cap`); not run in a game.

## Keeper of the Artifacts: an artefact of the match's kind at the match's start (patch 109, unreleased)

The artefact tree's 521 (`../docs/notes/artefact-skills.md`). srp draws one kind a match
(`artefact_carry::keeper`: Larkspur, Onyx, Spring) and writes it into modifier 40 bits 9..11 of
every kit; a joiner taken once the match is in process also gets bit 13 in its kits (late:
`survarium::v026g::keeper::give_late`; one taken in the countdown does not). Every
map has a hidden anomaly (content edit `artefact-keeper-1.json`): no container, energy off, one
empty state, 20 dormant artefacts of each kind. `keeper.register`
(generic_anomaly_core::register_artefacts' entry, 0x31B8E0) keeps the id range of the anomaly with
no container. At each player's tick (`battery.carrier_tick`, base_player::tick before
tick_animations 0x2DAAD3, order 4, after the glow's), a kit with the Keeper bit, once the time
rule is in process (core +18FE4h 3: the countdown's end) and, for a late joiner, once its place
has a death or suicide counted, takes its place's artefact of the drawn kind (place p the p-th of
the kind by id) into its first empty container through the game's own take_artefact; dead, or
with every container held, it waits for a later tick. Once a match: the take marks the artefact
given in its activation time (+2A0h: u32::MAX from its constructor, 0 at the gift, an
activation's time after; read only in state 4, saved in every world in every state), so nothing is
kept outside the world. Once given it is an artefact as any found one: a death keeps it in that
kit's container, and a take with every container held that replaces it leaves it dormant and
still given. Only a Keeper's tick reads the match. The pool's anomaly ticks and saves it, so
nothing is added to a world by the patch. Checked with `check_artefact_keeper_026g.py` (with a
death under flushed denormals and a replacement) and the patch's `gift` tests; not run in a game.

## A character controller's position stays finite (patch 118, unreleased)

In live match `rec026g_20261106-211841_03439962` (revision 50) a server bot's body went NaN in
one step, around 539 850 ms, after it stood up under a ceiling and walked. It stayed NaN for the
rest of the match in the one game that simulated it, falling ever faster. Its speed, its
`m_last_pos`, its bullets' origins and its animation stream followed (`../docs/notes/snapshot_rules.md`
7.2.1). A 0 ms step is not the cause: the game never steps 0 ms (`tick_events` 0x2D22DE).

**The patch** (the Rust patch `controller_position`, one lohook, `sites_026g.tsv`):
`controller.step_end` (0x7C7D7, bullet_character_controller::player_step's epilogue, which every
path joins). When the controller's position (+0B0h, the ghost object's transform) has an infinity or
a NaN in any float, it is put back to `m_last_pos` (+50h), where the step began, bit for bit. Any
other step is the game's. Nothing reads the position between the step's end and the epilogue, and
the ghost's AABB is next updated from it after the step, so no NaN reaches the broadphase.

A simulation change (`simulation_changes.tsv`'s `next` row). Checked with
`check_controller_position_026g.py` (finite steps byte for byte the original's; NaN and infinities
restored; the original fails) and the patch crate's tests. Not run in a game.

## A kicked game goes back to the lobby (patch 119, unreleased)

The match server kicks a game with 0xFD (`Transport::kick`: repeated invalid inputs, or a game
that restores did not bring back in step, `game.rs`). The game's handler calls
`instant_disconnect(4)` (0x3FDF0A), whose callback `on_match_disconnected` (0x21BE50) calls
`close_current_match(4)` (0x27A790). Its jump table (0x27A958) had for 4 the function's return: the
link was closed by the transport, nothing else was, and the game stayed on its screen (the free
camera, the world) with no match link for good. A lost connection (reason 1) tells the lobby `31`
("back from its match": srp's lobby answers with the lobby state, and the results if the match has
ended) and ends the world (`on_finish_match` 0x1F96F0: the world unloaded, the lobby shown). No
text: the lobby movie shows the lobby as after any match.

**The patch** (`KICK_TO_LOBBY`, one dword, every full build): the table's entry 4 (0x27A968) is
0x27A930, reason 1's `31` and world end, not 0x27A94C, the return. Reason 1's log line before it
(0x27A867) is left out: it names the reason as 1, 5 or else `disconnected_by_initiator`, which
would call a kick the player's own leave. The entry is an address: its relocation stays.

**In state 0 nothing changes.** `close_current_match` returns at once for every reason while the
time rule's state the client keeps (+1F1C60h) is 0 (0x27A7AA): from the connection until the
first tick played after the first world, and again after a close. A kick (or a lost link, a timeout)
on the loading screen or before the first world still leaves the game where it is, as before, and
srp kicks such games too since #1492 (`Removal::NoWorldToJoin`, a late joiner waiting for its first
world; `Removal::NoWorldToResync`, a game back in its match on a new connection): patch 120 ("A match
closed before it played") ends those. A resync keeps the state:
a game kicked there goes back to the lobby as one that timed out there does (reason 0 ends the
world the same way).

Nothing a match computes. Checked with `check_kick_lobby_026g.py` (Unicorn; `../docs/notes/client_patch.md`
section 3.72). Not run in a game.

## A match closed before it played (patch 120, unreleased)

`close_current_match` (0x27A790) returns for every reason while the time rule's state the client
keeps (+1F1C60h) is 0: from the connection until the first tick played after the first world, and
again after a close. So a game kicked (0xFD), timed out, cut off, or left by the menu's
leave_match in the handshake, while its level loads or with its level loaded before its first world
stayed on the loading screen or in the free camera for good. srp removes such games on purpose since
#1492 (`Removal::NoWorldToJoin`, `Removal::NoWorldToResync`, a refused initial world's 0xEA) and #1482
(handshake and refusal caps).

**The patch** (`unplayed_match`, Rust, five lohooks; `../docs/notes/client_patch.md` section 3.74).
The patch keeps one state: a match is open from connect_to_match's connect (0x21C07C) until its first
close, and given up at a logoff (network_client::disconnect 0x27B110). The guard's 0 means "never
played" for an open match and "closed already" otherwise. At close_current_match's guard (0x27A7A4),
for an open match never played:

- **the handshake** (no level asked for: no match core, the world not loading): `31 <order> FF` to the
  lobby, what a lost connection sends; srp's lobby clears the match and answers 0x85 00, which takes
  the match-making panel off the lobby scene. There is no world to end. (A 0xEA cannot come there:
  without a match core the handler skips it, 0x27C6A2.)
- **a level loading** (game_world+4E20h): nothing is ended under the load. The resource queries
  have no cancel, and the game's own abort (on_project_loaded, the lobby's status 0) unloads the level
  but leaves the loading screen up (only lobby_menu::on_deactivate hides it). The close waits: the
  load finishes, on_match_ready shows the world (its 0xDA goes to the closed link, as before), and at
  its end (0x27AEBF) close_current_match runs again for the reason the close came with, past the
  guard. If the lobby says "the lobby" (0x85 00, srp's match over) before the load ends, the abort is
  skipped for an open match the same way (reason 0, the world ended).
- **a level loaded, waiting for its first world** (the match core set): past the guard (0x27A7B0):
  the reason's own close, as a match that played: `31` and the world ended (on_finish_match: unloaded,
  the lobby shown) for a lost link, a kick or an expired message; 0xE0 first for the menu's leave
  (reason 2); the world ended and the results for the 0xEA farewell (3); the world for a timeout (0).

A played match (states 1-3, a resync's) and a closed one go as before (patch 119's path for a kick).
Nothing a match computes. Checked with `check_unplayed_match_026g.py` (Unicorn) and the patch crate's
tests. Not run in a game.

## Room for the resources helper and render arenas (patch 124, unreleased)

A match load with many decorated players (about 800 camouflage and decal bakes) runs out of two
fixed engine arenas: `out of memory in arena "resources helper allocator"` (8 MiB, 93-96 % used) or
`out of memory in arena "render"` (224 MiB). The rev54 test build, one headless client with 19 bots
wearing live players' gear: 0 of 4 loads got through the stock sizes (2 helper, 2 render); with
`-rh_allocator=32 -render_allocator=320` on the command line 3 of 3 were clean.

**The patch** (`ARENA_DEFAULTS` in `patch_026g.py`, the builds with the late join): the two
defaults, so every way of starting the game (the launcher, the Linux and macOS scripts, a replay)
gets them. One immediate each:

- `memory::preinitialize` (0xE0F20) stores 8 MiB in bytes as the u32 the key's reader
  (`is_set_as_number<u32>` 0x141B0, rh_allocator 0xA80380) writes a value to:
  `mov dword [esp+10h],800000h` at 0xE1099 becomes `2000000h`, 32 MiB. A value given is still MiB,
  `shl eax,14h`.
- `engine_world`'s constructor (0x13660) has 224 in EDI before the reader for render_allocator
  (0xA7C088): `mov edi,0E0h` at 0x13A95 becomes `mov edi,140h`, 320 MiB. A value given is still
  raised to at least 224 (the `cmp edi,0E0h` at 0x13AA7 is kept), then times 1 MiB.

Past these sizes the arena's full allocation still ends the game in out_of_memory_with_crash.
Nothing a match computes; a game with it and one without play together.

**The address space.** The arenas are reserved at the start (memory::allocate_region 0xE1120,
platform::allocate_arenas 0xE9580). The engine sums the fixed arenas and sizes the resource arenas
and its video budget from the address space left; the resource arenas keep their 768 MiB floor
(patch 37). In the test logs the engine's total went from 1 944 846 328 to 2 070 675 448 bytes
(+120 MiB) of the 4 GiB of the large-address-aware process, the resource arenas stayed 512 + 256
MiB, and the video budget (the address space left for the driver) went from 1 848 233 992 to
1 722 507 272 bytes. Nothing else in the exe reads the old sizes: the arenas keep their size from
the registration.

`check_memory_defaults_026g.py <original exe>` (Unicorn 2.1.4) runs both spans, from the key's read to
the registration (0xE108F..0xE10D7, 0x13A81..0x13AF4), with the native `is_set_as_number<u32>` and
a stubbed key reader: absent or not a number, the original's 8 and 224 MiB and the build's 32 and
320; values given (helper 4 to 64, render 100 to 512) the same size in both, 100 raised to 224.
Against the build without it, the default build differs in the four bytes of the two immediates and
the PE checksum. The built exe was not run in a game: the runs above gave the same sizes as options.

## The game tells how much memory it has left (patch 126, unreleased)

The game is a 32-bit process: its address space is cut at the start into fixed arenas and what is
left for the video driver, and a full arena (or a driver without room) ends the game. We sized the
arenas from a handful of crash dumps (patches 36, 37, 78, 124); these reports give the real margins
of players' games. The rules: `../docs/notes/memory-report.md`; the reading: `../docs/notes/client_patch.md`
section 3.76.

**The patch** (`memory_report`, Rust, two lohooks; `vostok::memory`, `srp_validate::memory`). When a
match connection's first world is in (`world_load.loaded`, order 3) and when the match closes on the
game (close_current_match's entry, `match_recovery.close`, order 1, only after a first world), a
snapshot: the address space's free bytes and largest free region (`VirtualQuery`), the engine's video
budget, and every arena of the engine's list with its name, size and use: a doug_lea heap walked
chunk by chunk (used, largest free block), the managed resources' and the multithreaded region's
counts, unknown otherwise. Nothing of the engine's is called: its size queries walk heaps other
threads use without a lock; ours reads only inside each arena, checked readable, and gives up on a
torn walk (a clean one can be slightly stale). The snapshot is written to `srp_patches.log` (a line for the process, one an arena) and
sent in the generic client-report carrier (`srp_validate::report`: 0xE0, prefix
`ff ff ff ff ff ff 03 00`, the match id, the kind, part and parts), all parts at once or none,
through patch 97's one 0xE0 path (`Connection::tell`).

**The server.** The match server logs each part once per connection, of its own match only, to
`logs/memory.log` (16 files of 4 MB): `<name> (account <id>), match <id>, loaded, part 1/2: process
free <bytes> largest <bytes> video <bytes> | arena "render" size <bytes> used <bytes> largest <bytes> | ...`.
`release/moderate memory` lists each arena, tightest first, with its least room left and least largest
free block, and the address space's least largest free region, with who and when. An older server takes
a carrier alone in its datagram for a chunk whose offset is past its total and ignores it, but it
refuses a whole multi datagram that carries one (it cannot measure the carrier): the server ships in
the same release as the patch or earlier. The transport measures the carrier with the codec's own
`srp_validate::report::carrier_len`.

Nothing a match computes. Checked with `srp-validate`'s tests, the patch crate's tests and Clippy, and
the match server's test. Not run in a game.

## Engine threads get 2 MiB stacks (patch 128, unreleased)

Every thread the engine starts had an 8 MiB stack, committed: 57 of them in a 19-bot match on a
24-thread CPU, 456 MiB of the 32-bit process's 4 GiB. The deepest touched 536 KiB, 51 of 56 never
more than 64 KiB (2026-11-09, under Wine, the bake-stress load: map 61, 777 camouflage/decal bakes,
about 5 minutes of play).

**The patch** (`THREAD_STACKS` in `patch_026g.py`, the builds with the late join): one immediate.
`threading::spawn` (0xE3770) gives `spawn_internal` (0xE0930) a fixed size, `mov edx,800000h` at
0xE37BA (file 0xD2BBA), whatever the thread; `spawn_internal` passes it to CreateThread with flags
0. It becomes `mov edx,200000h`. Every engine thread goes through it: splash, sound, network,
logic, hdd, dvd, res_man, res_cook, process termination and the task pool's 2 x min(32, cores)
"task #n" (the count at 0xE425E, the spawn at 0xFC4F1). The game's main thread (1 MiB, the exe's
own reserve) and the threads of the CRT, the driver and Wine are not engine threads.

On Windows the size is committed (no STACK_SIZE_PARAM_IS_A_RESERVATION) and, above the exe's 1 MiB
reserve, is the reserve too: 6 MiB of address space and commit charge freed a thread. 150 MiB with
8 logical CPUs, 342 with 24, 438 with 32.

`platform::allocate_arenas` still counts (threads x 2 + 8) x 8 MiB of stacks into "engine(fixed)"
(0xE98C2..0xE98CB) for its budget (patch 129 makes that term 2 MiB): kept here, so the resource arenas and the video budget are sized
byte for byte as before and the space freed stays free. Lowering that term too would size them
from the real stacks, and would keep a CPU with very many threads (64 and more: 1088 MiB counted
there, without the pool's cap of 32) from failing the "Not enough memory" check at the start
(read in the code, never seen).

**Frames above 2 MiB.** Every frame over 4 KiB calls `_chkstk` (0x429F20) with its size: 1754 of
them, 6 above 256 KiB. `network_client::draw_match_stats` (0x27F780, 5.8 MiB) runs on the logic
thread, from game::tick, only when `-net_stats` is on the command line: with this patch that
option overflows the logic thread's stack. Nothing we ship passes it. The other one above 2 MiB,
`static_render_surface::add_shadow_vertices` (0x16E550, 2.4 MiB), LIKELY never runs: in 0.10b
only `shadow_batched_geometry::build` calls it, and in 0.26g the scene's shadow geometry
(scene+410B6Ch, the one such object) is referenced only by the scene's constructor and
destructor. Neither ran in the measured game (a `_chkstk` touches every page of its frame). The
rest: `on_fs_iterator_materials_ready` (0x1176E0, 560 KiB), three
`scene::gather_streamable_textures` (276-278 KiB), none above 72 KiB after them; the 21 calls whose
size is not a `mov eax` just before take 4 to 40 bytes. The recursion guards in the exe are
Scaleform's (the main thread). Not proven: how deep a chain of calls or a recursion goes on an
engine thread outside the measured game.

Nothing a match computes. `check_memory_defaults_026g.py` runs threading::spawn from 0xE37BA into
spawn_internal to its CreateThread call in both exes: 8 MiB and flags 0 in the original, 2 MiB and
flags 0 in the build. Against the build without it, the default build differs in one byte (file
0xD2BBD, 80h to 20h) and the PE checksum.

Run in the bake-stress rig (2026-11-09, one headless client under Wine/DXVK, 19 bots, map 61,
the build of main with it against the same build with 8 MiB): 52 or 53 stacks of 2 MiB and none of
8 MiB, the deepest touched 540 KiB; the address space in use in the match at most 2757-2773 MiB
against 3085 MiB. Three of four loads clean; the fourth filled the multithreaded arena 17 s after
Play, as one of the two loads with 8 MiB did. Not run on native Windows, nor with a replay, a late
join, the death camera or another map.

## The multithreaded arena: at least 1024 MiB (patch 129, unreleased)

In the bake-stress load (one headless client, 19 bots in live players' gear, map 61, an empty
`streamed_textures`, about 780 bakes) the global multithreaded arena (768 MiB, patch 37) filled 17 s
after Play in 2 of 6 loads, 795 and 800 of 805 MB used, `not enough memory for arena [global mt
allocator]`, with 8 MiB stacks and with 2 MiB ones. The memory profiler (patch 127, not merged) saw
it peak at 760 of 768 MiB in one such load, mostly one 410 MiB transient block from
`query_result::finish_translated_query`. Patch 128 freed about 312 MiB of peak address space; this
patch spends 256 of it on that arena.

**The patch** (`MT_ARENA` in `patch_026g.py`, the builds with the late join): three immediates.

- `memory::preinitialize` (0xE0F20) presets the u32 that `is_set_as_number<u32>` (0x141B0) writes
  the key `mt_allocator` (0xA80150) to: `mov dword [esp+10h],1000000h` at 0xE0FFA. Without the key
  the code added 360 MiB to that preset (patch 37: 752); with it, `shl esi,14h`. The preset
  becomes 400h (1024) and the two branches (0xE100B..0xE1019) one floor: `mov eax,400h` /
  `cmp esi,eax` / `cmovb esi,eax` / `shl esi,14h`. No key or a key that is no number: 1024 MiB; a
  value under 1024: 1024; a larger one: itself.
- `platform::allocate_arenas` (0xE9580) counts (threads x 2 + 8) x 8 MiB of engine-thread stacks
  into "engine(fixed)" (`mov ecx,800000h` at 0xE98C6) before it sizes the resource arenas and the
  video budget from the address space left. The 8 MiB becomes patch 128's `THREAD_STACK_MIB` (2):
  the budget counts the stacks there are. "engine(fixed)" changes by 256 - 6 x (2 x threads + 8)
  MiB: +160 on 4 threads, +112 on 8, +64 on 12, +16 on 16, less from 18 on (-80 on 24). On a
  machine limited by its address space, the budget moves by the opposite (here, 24 threads:
  1643 -> 1723 MiB), and machines with fewer threads start from a budget higher by the same
  amount, so every one stays above 1 GiB. It also takes the start's "Not enough memory" check
  away from very large CPUs (64 threads counted 1088 MiB there).

**Machines limited by RAM** (read in the code, not run). `calculate_desirable_resource_arenas`
(0xE8DC0, 0.10b `memory_win.cpp`) also bounds everything by RAM: the physical memory (0x1B3A1D0)
less the kernel's (0x1B3A200, at least 512 MiB) less "engine(fixed)" must be at least 320 + 768
MiB, or the game stops at its start ("Not enough memory to run Vostok Engine"); the budget is then
at most that less the 768 MiB floor. On fewer than 18 threads "engine(fixed)" grows (above), so
on such a machine the budget drops by the same 160, 112, 64 or 16 MiB, and patch 124 (the same
release) adds 120 MiB on every machine. With the kernel at its 512 MiB floor, the RAM Windows
reports where each line falls, live revision 53 -> with patch 129 -> with patches 124 and 129:

| Threads | Budget under 1 GiB (auto level 3, 0x3CF260 1024 not 2048) | Under 640 MiB (0x3CF260 512) | The start refuses |
|---|---|---|---|
| 4 | 3519 -> 3679 -> 3799 MiB | 3135 -> 3295 -> 3415 MiB | 2815 -> 2975 -> 3095 MiB |
| 8 | 3583 -> 3695 -> 3815 MiB | 3199 -> 3311 -> 3431 MiB | 2879 -> 2991 -> 3111 MiB |
| 16 | 3711 -> 3727 -> 3847 MiB | 3327 -> 3343 -> 3463 MiB | 3007 -> 3023 -> 3143 MiB |

So a "4 GiB" PC that reports about 3.5 to 3.8 GiB, on 16 threads or fewer, can now get the lower
automatic level and the 1024 size where it had the higher; a "3 GiB" PC that reports about 2.8 to
3.1 GiB can now refuse to start where it started. A larger kernel moves each line up by as much.
On 18 or more threads patch 129 lowers the lines (patch 124 still adds its 120). No change here
keeps today's numbers on few threads simply; it is left as it is.

**The one owner of the number** is the exe (`MT_ARENA_MIB`). No launch line passes `-mt_allocator`
any more (the launcher's arguments, the Linux and macOS scripts, `play_skhid.bat`, the updater's
replay, `Skhid-without-updater.bat`, the local clients); they keep
`-minimum_resources_memory_size=768`. A launcher or a script of before still passes
`-mt_allocator=768`, which the floor raises to 1024: a new exe gets 1024 MiB with any launcher, an
old exe (patch 37) 768 MiB with any, as today. A cert-only build (no patch 37 either) gets the
retail 376 MiB without the key; it is no players' build.

**The video budget** (u64 0x1B3A1F8, the startup dump's "video memory stats for engine") is not
reserved: it is the address space the engine leaves for the driver. The render gets it at its
creation (engine_world 0x15574 -> render::world -> render::engine::world 0x117820 -> device+1B0h
and resource_manager+10h) and reads it only against thresholds: above 1 GiB or not (the automatic
level 2 or 3, e.g. 0x1C96E0, 0x190508, 0x19377B, 0x11AB9C), 640 MiB and 1 GiB (512, 1024 or 2048,
0x3CF260), and in the statistics (MiB, 0x13D766). 1643 or 1723 MiB is above both; a machine limited
by RAM can fall under them (above).

Nothing a match computes. `check_memory_defaults_026g.py` runs the key's read to the size stored
in both exes (absent, not a number, 256 to 1536, 2048) and the stack term for 4 to 64 threads
(8 MiB a stack in the original, 2 in the build). The self-test's default-build variant passes.

Run in the bake-stress rig (2026-11-09, under Wine/DXVK, 24 threads, main + this patch): 6 of 6
loads clean, about 5 minutes in the match each; 3 with the launch line without the key, 3 with the
old `-mt_allocator=768`; all 6 logged the arena at 1 073 741 824 bytes, the engine's arenas at
2 339 110 904 bytes (+256 MiB), the resource arenas 512 + 256 MiB, the video budget 1806.5 to
1806.8 MB (1723 MiB, from 1643). The arena's touched extent peaked at 558 to 816 MiB (816 is past
the old arena's end), the address space mapped below 4 GiB at 3001 to 3037 MiB (2757 to 2773 with
768 MiB, 3085 before patch 128). With 1024 MiB a 760 MiB peak has 264 MiB to spare; the 410 MiB
transient needs one hole of its size, so two at once would not fit. Not run on native Windows, nor
with a replay, a late join or another map.

## The memory profiler (patch 127, unreleased)

What fills the engine's fixed arenas (the "render" arena full live on 2026-11-08, "ai" on 2026-10-17)
was known from one dump walked by hand. Patch 127 counts every block of the engine's heaps to its arena
and the place in the game that asked for it. It is the owner's tool, and a worker's when a local test
needs it; players never set its switch: `SKHID_MEMORY_PROFILE=on` in the game's environment (the owner:
`SKHID_MEMORY_PROFILE=on ./Skhid-local.sh launcher`; Windows: `set` it before `Skhid.bat`; run.py passes
it on only when it is started with it).

**Off** (every player's game): the exe keeps the game's bytes where the profiler hooks the allocator. Its
five gates (dlmalloc's `mspace_malloc`, `mspace_realloc`, `mspace_free`, ptmalloc3's two frees of a mapped
block) and its out-of-memory lohook are linked but installed on demand (`install = "on_demand"`,
hooks_026g.py): no instruction more on any allocation or free. What runs: `memory.arenas_ready` once at
the start, and one call per world loaded and per match closing in lohook chains other patches already
have. **On**: at the start (`vostok::memory_profile::start`, checked to run in allocate_region's frame, once,
while only the main thread runs) every site and page is readied, 16.8 MiB of tables reserved with VirtualAlloc,
then the six jumps written (VirtualProtect, the write, the protection back, FlushInstructionCache); a refusal
at any step writes nothing and gives everything back. Snapshots are
written to `srp_memory_profile.txt` in the user data folder when a world is in, when a match closes, before
a full arena ends the game, and at Scroll Lock (polled by the gates); the tables are behind a lock whose
waiters yield after a few spins (`vostok::YieldingLock`). `../scripts/memory-profile/analyze.py`
prints each arena's use, holes and top call sites by live and peak bytes (named from the re-kit) and the
address-space budget. Nothing a match computes. Details: `../docs/notes/memory-profile.md`. Checked with
`check_memory_profile_026g.py` (Unicorn) and the patch crate's tests.

## Shader constants: a faster copy, no upload of unchanged bytes (patch 134, unreleased)

The perf profile of a live rev54 game (the owner's, shooting) had shader_constant_buffer::set_memory
(0x11B5A0) at 11.65 % of the main thread, the most of any function (5.2 % without shooting), and
d3d11.dll (DXVK) at 7.7 %. Before each draw the backend's constant tables copy every bound constant
into their buffers' shadows (constants_handler::assign 0x3D6300/0x3D6640/0x3D6830, a bone palette
from skeleton_render_model_instance::set_constants 0x15A520, 70 call sites), then update
(0x3DDFD0, from update_buffers 0x3D63F0) maps every buffer whose m_changed (+64h) is set with
WRITE_DISCARD, copies its whole shadow (+58h, +5Ch bytes) and unmaps it. Two costs:

- set_memory copies one byte at a time with a compare (about 10 instructions a byte: 11081 for a
  1 KiB bone palette);
- it sets m_changed always (0.10b's source set it only when a byte differed: `m_changed |= diff >
  0`; 0.26g compiles to `mov byte [ebp+64h],1`), so every bound buffer is uploaded before every
  draw, also when its bytes are the ones on the GPU (a skinned surface's palette, the same for its
  depth pass, its colour pass and each shadow cascade; the frame's view constants).

The patch (`constant_upload_026g.asm`, four `code` hooks of the Rust patch constant_upload):

- set_memory, the whole function: the same copy 16 bytes at a time (SSE2, unaligned), the original's
  loop for a tail under 16; the registers left as the original leaves them (EAX = ECX the end, DL
  the XOR of the first differing byte pair or 0, EDX's upper bytes, EBX, ESI, EDI, EBP; XMM0 and
  XMM1, which it uses, saved and restored), m_changed set as before. The 67 copies of set_memory
  the compiler inlined elsewhere are left as they are.
- the constructor (0x3DDFA1, 0x3DDFAC): the shadow's block is 2 x size + 16 bytes, all zeros: the
  shadow, the upload's state (a dword: 0 never uploaded, 1 uploaded) and 12 bytes, then a copy of
  the shadow as last uploaded. Both ways a buffer is made (resource_manager 0x3BFEEC, a temporary
  on the stack, and 0x3BFFCF) go through it, and no other code stores a malloc_impl block at
  +58h (0x3DDFB3 is the only one); the destructor frees the block as before.
- update's start: with m_changed set it compares the shadow with the copy, copying it as it goes;
  when the buffer was uploaded before and no byte differs it clears m_changed, as the upload does,
  and returns; else the original's Map, copy and Unmap (0x3DDFDD). The test is of the bytes, not
  of m_changed, so a shadow written by an inlined copy is uploaded as before.

The GPU's bytes are the original's at every draw: a buffer is a dynamic one (usage 2, CPU write)
and update is the only code that writes it (every Map of the exe read: 0x3DDFFA is the only one of
a constant buffer; a dynamic buffer takes no UpdateSubresource); skipping a WRITE_DISCARD whose
bytes are the ones mapped last leaves the same bytes bound. Not equivalent only for a source that
overlaps the shadow less than 16 bytes ahead (the copy goes 16 at a time); no caller passes one
(the sources are the constants' own values).

Measured in `check_constant_upload_026g.py` (instructions in Unicorn): set_memory 593 -> 97 for
64 bytes, 1801 -> 202 for 192, 11081 -> 924 for 1 KiB, 33935 -> 2661 for 4 KiB; a made-up run of
150 draws over four buffers (a view matrix a frame, a world matrix each draw, a bone palette new
half the draws, material constants rarely new) 2.14 M -> 0.48 M instructions and 600 -> 268 Maps.
In the game: set_memory's 11.65 % is expected to fall to about 1 %; the Maps saved (and DXVK's work
for each, on the main thread and its CS thread) depend on the scene and are not measured yet.

Nothing a match computes (rendering only). Checked: `check_constant_upload_026g.py`, the self-test's
test_writes, test_call_shapes, test_rust_hooks and test_rust_code on the default build. **Not run in
a game.**
