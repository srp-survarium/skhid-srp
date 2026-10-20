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
| `patch_026g.py` | Builds the patched exe from the original. Python 3, nothing else |
| `snapshot_upload_026g.asm` | The source of the upload code (nasm) |
| `snapshot_upload_026g.section.bin`, `.hash-mismatch.bin` | That code assembled, for the two places it can go; `patch_026g.py` reads them. Assembling the source again gives the same bytes |
| `late_join_026g.asm`, `late_join_validate_026g.c`, `late_join_runtime_026g.c`, `late_join_install_026g.c`, `late_join_runtime_026g.h` | The source of the late-join code (patch 11): the hooks and the game's calling conventions in assembly, the checks of what the server sends and the bookkeeping in C |
| `late_join_026g.section.bin`, `late_join_026g.section.json` | That code built, which `patch_026g.py` reads, and beside it where the C functions are that the assembly calls and what built it (gcc 13.2.0, GNU ld 2.41.90, NASM 2.16.01). The same tools give the same bytes; another C compiler gives other bytes that do the same, so the file is kept, not rebuilt for a release |
| `late_join_026g.py` | Builds those two files (`--write-bin`), says whether they are what the sources give on this machine (`--check-bin`), or builds an exe straight from the sources for work on the code. Needs nasm, a C compiler for x86 and GNU ld |
| `check_late_join_026g.py`, `test_late_join_runtime_026g.c` | Checks of the late-join code: the first runs the patched machine code on made-up objects (needs Unicorn 2.1.4), the second the C bookkeeping under the sanitizers |
| `check_fov_026g.py` | Runs the original and patch 22's camera projection, the game's matrix builder included, and compares the matrices (needs Unicorn) |
| `check_fov_lod_026g.py` | Runs patch 40 (a model's LOD and screen factor at a wider field of view are the original's): the projection's k at several settings and zooms, the game's select_lod and screen factor of the original and the build on made-up models (needs Unicorn) |
| `pickup_below_026g.asm`, `pickup_below_026g.section.bin` | The source of patch 23's code and that code assembled, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `check_pickup_below_026g.py` | Runs the original and patch 23's use scan on made-up ray hits (needs Unicorn) |
| `replay_seek_026g.asm`, `replay_seek_026g.section.bin` | The source of patch 26's code (a replay's seek empties the weapons' sound queues) and that code assembled, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `default_server_026g.asm`, `.section.bin`, `.hash-mismatch.bin` | Optional default-login-address stub, appended to the snapshot cave; source and assembled code for both placements |
| `make_client_026g.py` | Makes the players' overlay folder or ZIP: patched exe, Rust BugTrap DLL, certificate, start file and README |
| `play_skhid.sh`, `LINUX.txt` | Linux Wine/DXVK launcher and instructions, included in the generated folder and ZIP |
| `stale_dll_026g.asm`, `stale_dll_026g.section.bin` | The code of patch 25 (the sign-in reports a stale `shell_32.dll` in `%TEMP%`, a file one known cheat leaves behind) and that code assembled for 0xE00 of `.srp`; `patch_026g.py` reads the .bin |
| `check_stale_dll_026g.py` | Runs patch 25's check on made-up machines, loaded as is and rebased (needs Unicorn) |
| `mount_patches_026g.asm`, `mount_patches_026g.section.bin` | The code of patch 27 (the game mounts each `resources.db.patch_<n>` over `resources.db`) and that code assembled for 0x400 of `.srp`, its state at 0x800 of `.srpd`; `patch_026g.py` reads the .bin. Assembling the source again gives the same bytes |
| `check_mount_patches_026g.py` | Runs patch 27's two hooks and its code on made-up game folders, loaded as is and rebased (needs Unicorn) |
| `forward_scroll_026g.asm`, `forward_scroll_026g.section.bin` | The code of patch 28 (the transparent effects scroll as in 0.29, `scene_slomo` bound to 1.0) and that code assembled for 0xB00 of `.srp`; `patch_026g.py` reads the .bin. Assembling the source again gives the same bytes |
| `replay_link_026g.asm`, `replay_link_026g.section.bin` | The source of patch 29's code ("Watch replay": the lobby's link starts the updater) and that code assembled, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `check_replay_link_026g.py` | Runs the original and patch 29's link callback on made-up links (needs Unicorn) |
| `shared_upgrades_026g.asm`, `shared_upgrades_026g.section.bin` | The source of patch 30's code (one upgrade on several items: taking it off names the item) and that code assembled for 0x700 of `.srp`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `check_shared_upgrades_026g.py` | Runs the original's and patch 30's upgrade-window drop and the `83 46 01` / `02` answers on made-up inventories with one upgrade on several items (needs Unicorn) |
| `kit_change_026g.asm`, `kit_change_026g.section.bin` | The source of patch 31's code (a kit chosen before the match starts is played at once) and that code assembled for 0xEC0 of `.srp`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `warmup_look_026g.asm`, `warmup_look_026g.section.bin` | The source of patch 31's camera before the start (while the kits are shown, a drag turns the warmup camera and the wheel moves it, as the lobby's character view) and that code assembled for 0xD40 of `.srp`, its state at 0xE20 of `.srpd`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `warmup_keys_026g.asm`, `warmup_keys_026g.section.bin` | The same patch's buttons: one held from before the start (a drag of its camera) is not the player's until it is let go, and that code assembled for 0x9A8 of `.srp`, its state at 0xE30 of `.srpd`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `warmup_limits_026g.asm`, `warmup_limits_026g.section.bin` | The same camera's end of a drag and its limits (the lobby camera's: distance 1.5 to 4.6, pitch within 60 degrees) and that code assembled for 0xA98 of `.srp`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `read_order_026g.asm`, `read_order_026g.bin` | The source of patch 33 (the order of the archive reads is a strict weak ordering) and that code assembled at 0x10BCC0, the 0x1A0 bytes of `hdd_manager_sorting_predicate` it replaces, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `check_read_order_026g.py` | Runs the original's and patch 33's predicate, with the game's own path, string and offset helpers, and the game's own sort on made-up read queries over several archives (needs Unicorn) |
| `suicide_key_026g.asm`, `suicide_key_026g.section.bin` | The source of patch 34's code (the commit_suicide key kills one's own player) and that code assembled for 0x980 of `.srp`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `spot_026g.asm`, `spot_026g.section.bin` | The source of patch 38's code (a spot's hit on a battery's body spots the battery) and that code assembled for 0x7C0 of `.srp`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `check_spot_026g.py` | Runs the original's and patch 38's input apply (the spot and its wait, the player's tick's countdown) with the game's own spottable code of players and batteries on made-up ray hits (needs Unicorn) |
| `check_suicide_key_026g.py` | Runs the original's and patch 34's last action call of the input builder, with the game's own event buffer code, on made-up input handlers (needs Unicorn) |
| `check_tab_list_026g.py` | Runs the original's and patch 39's refresh of the scoreboard's rows (update_ui 0x23B421..0x23B477), with the game's own list code of initialize_match made again, on made-up huds whose places enter and leave (needs Unicorn) |
| `check_kit_change_026g.py` | Runs the original and patch 31's respawn rule tick, with the places' tick and the game's own spawn, the view of a kit change, the hud's update of the local player, mouse moves and the mouse's buttons through the game world's input handler and the warmup mode's step on made-up objects (needs Unicorn) |
| `check_baked_texture_026g.py` | Runs the original's and patch 35's baked-texture list of the camouflage/decal baker (the look-up, the push and check_loaded_baked_texture) on made-up loads, among them those of the crash of 2026-10-17 (needs Unicorn) |
| `weapon_preview_026g.asm`, `weapon_preview_026g.section.bin` | The source of patch 32's code (a weapon's upgrade window shows the weapon) and that code assembled for 0x2E50 of `.srp`, its state at 0xE00 of `.srpd`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `weapon_draw_026g.asm`, `weapon_draw_026g.section.bin` | The source of patch 32's draw (a weapon's window draws the player as the lobby does) and that code assembled for 0xD20 of `.srp`, which `patch_026g.py` reads. Assembling the source again gives the same bytes |
| `check_weapon_preview_026g.py` | Runs the original's and patch 32's upgrade window start and close (with the lobby camera's own setter), `setup_profile`'s entry, the lobby camera's three distance clamps and the player's decal pose on a made-up lobby, loaded as is and rebased (needs Unicorn) |
| `check_forward_scroll_026g.py` | Runs the original's and the build's forward-effect compile, with patch 28's two hooks, on made-up materials and compares what each sends; runs the third hook against the game's own binding of `scene_time`; loaded as is and rebased (needs Unicorn). With `--rev12=` it compares the build with revision 12 byte by byte |
| `selftest_026g.py` | Builds every variant and checks it (sections, disassembly of every hook, checksum, signature, and what the late join adds to the build without it); needs the original exe and nasm |
| `run_battery_rule.ps1` | Runs the battery rule's handler (patch 9) out of any 0.26g exe on a made-up rule object, in the 32-bit Windows PowerShell: what an exe does with a put that is played again after a rollback. Starts nothing of the game |
| `survarium_login_server.crt` | The login certificate (the same file as `../certs/survarium_login_server.crt`) |
| `launcher/` | The game's launcher made to work again, and to keep these files current: its patch tool, the one piece of new code, the self-test and its own README. Its updater is `../../client/updater` |

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
survarium_skhid.exe -localization=english -server=<host>:<login port> -mt_allocator=768 -minimum_resources_memory_size=768
```

Or let `make_client_026g.py` do all of it, without touching the game folder.
`bugtrap.dll` and `updater_skhid.exe` build reproducibly: the same commit and
toolchain give the same files in any folder, so a release builds them afresh
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
which starts `launcher_skhid.exe`: it looks for a newer ZIP on that site, installs it, and its
PLAY button starts the game in the language chosen in it. The same input gives the
same ZIP byte for byte, which the updater relies on. Back up the original `bugtrap.dll` and login certificate before
installing. `--check` does every check and writes nothing. Existing ZIPs are
never overwritten. `--bugtrap=<path>` selects another built DLL; by default
the builder reads `client/bugtrap/target/i686-pc-windows-msvc/release/BugTrap.dll`.

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

On Linux, install Wine with support for 32-bit Windows applications and
Winetricks, then run `bash play_skhid.sh` from `game/binaries/x86`. The launcher
creates its own Wine prefix, installs DXVK on first use, and preserves saved
game settings. Full builds use the embedded server; edit `server=` at the top
of the script for an override. Certificate-only builds fill in that value.
Both launchers use the builder's `--localization` setting. See `LINUX.txt` for
details, including the English-keyboard-layout startup workaround.

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

`login_hint_026g.py` replaces the old registration button with plain text:

- English: "Account created on first sign-in"
- Ukrainian: "Акаунт створюється під час першого входу"
- Russian: "Аккаунт создаётся при первом входе"
- German: "Konto wird beim ersten Login erstellt"
- Polish: "Konto powstaje przy pierwszym logowaniu"

The hint wraps inside the login panel. The old button is hidden and disabled,
removed from keyboard focus, and its browser callback is erased. All five game
languages have a translation. The language comes from the native client's
localization data, so it follows `-localization`.

Build a new resource archive with Python 3, a JDK and JPEXS FFDec 26.0.0:

```sh
python login_hint_026g.py /original/game/resources.db /output/game/resources.db \
  --ffdec=/path/to/ffdec.jar
```

`--java` and `--javac` can specify the JDK executables. The original archive is
only read; an existing output is refused. Install the output as the client's
`game/resources.db` while the game is closed, keeping the original for rollback.
This archive is a separate output from the small `make_client_026g.py` overlay;
include it when preparing a complete client distribution.

The patch verifies the original build 2777 login movie's SHA-256 and edits one
ActionScript block. All other movie tags and existing unrelated methods remain
byte-identical. Recompressing the movie leaves enough room in its original
archive entry; padding preserves the entry size and offsets, with four ignored
trailer bytes retaining the original content CRC. Both the signed file table
and the resource checksum validation remain intact. No archive signature bypass
is added. The sources are in `login_hint/`;
game assets are generated locally and are not checked in.

## Field-of-view slider

`fov_slider_026g.py` puts a "FOV:" slider under gamma in Settings →
Graphics. The exe side is patch 22 (patch revision 11): the game's video
option 7, a "FOV" slider labelled `st_fov_option` in all five languages, is
bound to `uro_fov`, which the camera follows. `Settings.as` maps the option
to `_slider_fov`, but the stock menu places no slider for it; this build
adds one to `main_menu.swf` in a new resources.db:

```sh
python fov_slider_026g.py /original/game/resources.db /output/game/resources.db \
  --ffdec=/path/to/ffdec.jar
```

It needs a JDK and JPEXS FFDec (tested with 25.1.3), as the login hint
does. The input is only read; an existing output is refused. It compiles
`fov_slider/SrpFovSlider.as` into `main_menu.swf` and calls it twice in
`Settings.create_content`. The first call, before the options are bound,
creates the slider on the Graphics frame, hidden, so the game binds option
7 to it like its other controls (label, range, value, apply). The second
call, before the final return, runs after the game has set the label, as
the login hint's calls run after the stock code. It shows the slider, makes
it opaque (Scaleform leaves a slider made by script at alpha 0), adds the
colon gamma has ("FOV:"), places it under gamma and moves the preset-button
panel down to make room. Under Scaleform `ENTER_FRAME` never reached the
slider, so nothing runs per frame. As with the login hint, one ActionScript
block changes, every other tag and unrelated method stays byte-identical,
and the archive entry keeps its size and CRC, so the signed file table is
untouched. The input may already carry the login hint.

With an exe before revision 11 the slider shows and does nothing: its
option 7 names a console variable that does not exist. Without this
archive, revision 11 still has the setting (`uro_fov 80` in the console or
`user.cfg`), just no slider. `test_fov_slider.py` checks the build's
refusals without game files. Checked live in the lobby and in a match on a
local server (with an earlier form of patch 22): the slider shows "FOV:"
under gamma, changes the view and is saved to `user.cfg`. In the players'
download from revision 12 the movie ships in `resources.db.patch_12`.

## Lobby exchange, silver and gold both ways

`exchange_026g.py` turns the lobby's gold-for-silver window into an exchange
both ways. The gold box at the top of the lobby ("Get Gold", which asked to
open the online store) opens it to buy gold; the silver box opens it to sell
gold. The window has:

- **Pay**: an amount, the coin and its name, the balance;
- a **v^** button that swaps the two currencies (the old Receive amount
  becomes the new Pay amount);
- **Receive**, the same way;
- the rate under them, Pay first: "100 Silver = 1 Gold" or "1 Gold = 100 Silver";
- "Not enough silver" / "Not enough gold" in red, with the Exchange button
  greyed out, when the Pay amount is more than the balance (since the
  exchange update below, a typed amount becomes the most the balance pays
  for; the note is left for the amount the window opens or swaps with).

Typing in either field updates the other. Silver goes by whole gold: a silver
amount that is not a multiple of the rate is evened out when the field is
left, and on Exchange. The texts are in all five game languages; the language
is the one of the client's own "Cancel" (`st_cancel`; in German the game's
file says "Abrechnen"). "Silver" is the game's own word, read live
(`regular_money`, `st_regular_money`); "Gold" is the game's `st_premium_money_name`,
copied, as the movie has no key for it; the rest are the window's own words,
the game having none for them.

It is the stock window (`MessageMoneyExchange`, symbol `SrvMessageExchangeSkinned`)
with the stock pieces moved: its 9-slice background made 300 x 300, its buttons
and title. The panels are drawn as the stock stepper's dark box, the coins are
`CoinsIcon2`, the swap is a `SrvButtonScale` (the skin of EXCHANGE / CANCEL)
with two arrows drawn on it. The texts take the font of the stock title
(bold Calibri from the font library, embedded): the movie's own plain
"Calibri" has two glyphs, and text in it shows as squares. Five methods
change, in two ABC blocks: the window's constructor (builds the layout, so the
size is final when `Message` centres it; the swap button is added in
`configUI`, after that, or the window opens in the corner), `configUI` and
`onMessageAccept`, and `StatusPanel.onPremiumMoneyAddClick` /
`onRegulatMoneyAddClick`. The exe is not
changed: the window calls the game's `exchange_premium_money`, which sends `47 04`
with the u32 it is given. Gold for silver sends the gold, as the stock window
did; silver for gold sends `0x40000000` + the gold (`SrpExchange.BUY_GOLD`),
which the server reads as `BuyPremium` (`requests::BUY_PREMIUM`). So the server
must be released first; an older server refuses such an amount as more gold
than the account has.

The input is a lobby region with the matches list (sha256 `8a98cf0d…`, live
since 14 Oct, or `48c3439f…`, with the JOIN reasons and the match level; the
exchange changes other classes than the list's), not the retail movie, either as a whole `resources.db` or as
the extracted `lobby_menu.swf` file. The output is of the
same kind:

```sh
python exchange_026g.py <lobby_menu.swf entry> <new lobby_menu.swf> --ffdec=/path/to/ffdec.jar
```

The checks are those of the login hint: the input's sha256, every other method
and tag byte for byte, the entry's size and CRC kept. The build is
deterministic. Tests: `python -m unittest test_exchange`.

Checked in a local lobby under Wine (2026-10-15, against this branch's
server): the window opens centred, shows both directions with their balances
and rate, the swap button works, and gold bought with silver reaches the top
bar. Not in release overlays yet.

## Get Gold opens the exchange

`buy_gold_026g.py` makes every "Get Gold" of the lobby open the exchange
window above, to buy gold with silver, instead of the online store (a browser
window on a dead site). The rules: `docs/notes/buy-gold.md`. The
"Insufficient Gold Rubles" dialog's button calls `goToStore` of the window that
showed it; the bodies of all ten (`ItemCard`, `StatusPanel`, `MessageAsk`,
`MessageCraftItem`, `MessagePostPremium`, `MessagePremium`,
`MessagePremiumDiscount`, `MessagePremiumProfile`, `MessageRepair`,
`MessageRerollItem`) and the premium window's gold button
(`MessagePremium.onPremiumMoneyAddClick`, which asked first) become
`SrpBuyGold.open()`. It shows the exchange with `showMessage`'s third
and fifth arguments set, so it opens over a window still open instead of
waiting behind it, and that window stays visible under it. The
build refuses a movie where one of them is missing or does not lead to the
store, and checks that no method pushes `go_to_store` afterwards. The exe and
the server do not change.

The input is the live lobby movie (`flash_movies/lobby_menu.swf` of
`resources.db.patch_24`, sha256 `3ca94f2a…`); the output is a new movie for a
new `resources.db.patch_<n>`, as for the rooms:

```sh
# development only (a step of build_lobby_026g.py, which builds the movie that ships)
python buy_gold_026g.py <lobby_menu.swf> <new lobby_menu.swf> --ffdec=/path/to/ffdec.jar --step-only
```

The build is deterministic (sha256 `b7cf70a8…` with FFDec 25.1.3). Tests:
`python -m unittest test_buy_gold`. Not in a release yet.

## Exchange window updates

`exchange_update_026g.py` brings a change of `exchange/SrpExchange.as` to a
live lobby movie, which has the window already: FFDec compiles the class anew
over the one in the movie (`exchange/ReplaceExchange.java`). Only the window's
ABC block changes; every method of every other class and every other
script's initializer, found by name (the block's bodies may be renumbered),
is checked byte for byte, and every constant they may point at keeps its
index and value. A method defined twice in the source is refused (FFDec keeps
the last).

The first such change (2026-10-20): an amount more than the Pay balance pays
for becomes, as it is typed, the most it does, in the field typed in. Buying
gold with 5,500 silver: 10,000 in Pay becomes 5,500, 100 in Receive becomes
55. Selling gold works the same way with the gold balance. Silver is still
whole gold: with 5,550 silver, anything over 5,500 becomes 5,500.

```sh
# development only (a step of build_lobby_026g.py, which builds the movie that ships)
python exchange_update_026g.py <lobby_menu.swf> <new lobby_menu.swf> --ffdec=/path/to/ffdec.jar --step-only
```

The input is the movie with Get Gold (`b7cf70a8…`); the build is
deterministic (`f900cc8b…` with FFDec 25.1.3). Tests:
`python -m unittest test_exchange_update`. Not in a release yet.

## Buy for modified items

`buy_modified_026g.py`: right-click on an owned item with modifications lists
Buy, which buys a stock one (the rules: `docs/notes/buy-modified.md`). The
menu (`InventoryList.showContextMenu`) listed Buy for an item that is not a
stack only when `!itemData.rank`; an owned item's rank is its number of
modifications (the server's `roll_modifications`), and a dictionary item's
is its rarity. The test becomes `!rank || itemAvailable`, the method's own
"owned and enabled" (`descriptor <= 100 && enabled`): after `getproperty
rank`, `convert_b; getlocal <itemAvailable>; not; bitand` (no jump added;
the register from the method's debug name), and the `not` that followed
stays. Shop, bundle and bundle-content entries are never `itemAvailable`, so
they keep the old rule. `buyElement` and `try_buy_item` go by the dictionary
id and the shop's
price and seller, so nothing else changes; the server gives a new item
without modifications.

A buy on an owned item is not asked "This item is already purchased by you.
Would you like to buy anyway?" any more. `LobbyMenu.try_buy_item`, which
every buy goes through, asked it for a non-stack item the account has unless
the buy came from a bundle (`sourceSlot != 300`); it asks it now only for a
buy from the shop's list (`sourceSlot == 200`): `pushshort 300; equals;
not` becomes `pushshort 200; equals; nop`. A buy on an owned item comes
with that item's own slot (100 or less), the victory screen's treasure maps
with 200 as before.

The input is the movie of the exchange update (`f900cc8b…`); the build is
deterministic (`45452c50…` with FFDec 25.1.3):

```sh
# development only (a step of build_lobby_026g.py, which builds the movie that ships)
python buy_modified_026g.py <lobby_menu.swf> <new lobby_menu.swf> --ffdec=/path/to/ffdec.jar --step-only
```

Tests: `python -m unittest test_buy_modified`. Not in a release yet.

## The craft window: Buy and Sell, Continue and Exit

`craft_menu_026g.py`: right-click on an item in the craft window's list
("Upgrade") gives Buy and Sell only, and its result offers Continue and Exit (the rules: `docs/notes/craft-menu.md`).
The window locks the inventory list (`lockControls(true)`), and
`InventoryList.showContextMenu` ended at once on a locked list. Its read of
the lock, `this._inventoryLocked`, becomes `SrpCraftMenu.locked(this._inventoryLocked,
this, activeItem)`: five instructions after the `getproperty`, no jump added.
While the list is locked and a `MessageCraftItem` is on the lobby, the helper
shows its own menu (Buy by the list's rule; Sell for an owned item, not the
item being upgraded or the donor the window holds) and returns true, so the
method ends; otherwise it returns the lock as it was. Buy goes as the item
card's (the dictionary id, the shop's price, the shop's seller); Sell through
the list's public `sellItem`.

After an upgrade the window's result had one button, Ready, which closed it.
`MessageCraftItem.fillCraftedItem` now ends with `SrpCraftMenu.result(this,
result)`: the window's Cancel shows again as Exit (the lobby already closes
the window on it, as on Ready), and the Ready button becomes Continue, side
by side. Continue closes the window as Exit does, puts the main item (as the
inventory has it now) on the item card (`setPrimaryItem`) and sends the
list's `craft_item` event, which opens the Upgrade window for it again with
an empty donor slot; an item with three modifications, or not craftable,
just closes. The words are the game's: `btn_exit_label` and
`btn_go_to_online_store` ("Continue").

The input is the movie of Buy on owned items (`45452c50…`); the build is
deterministic (`1091a19d…` with FFDec 25.1.3):

```sh
# development only (a step of build_lobby_026g.py, which builds the movie that ships)
python craft_menu_026g.py <lobby_menu.swf> <new lobby_menu.swf> --ffdec=/path/to/ffdec.jar --step-only
```

Tests: `python -m unittest test_craft_menu`. Not in a release yet.

## Weapon scope context menu

`scope_context_menu_026g.py` adds the existing localized **Optics** action to a
compatible weapon's right-click menu, in the inventory or an equipped slot. It
opens the scope editor already shipped in revision 19. Armor still opens its
camouflage/decal editor. See [weapon_scopes.md §5](../docs/notes/weapon_scopes.md#5-weapon-right-click-menu)
for the build command and validation. The output is a `lobby_menu.swf` replacement
for the next resource patch (built on `1091a19d…`, the craft window's movie: `2e4cb7d2…`,
deterministic with FFDec 25.1.3); this does not change the executable or reserve a
release revision.

## Building the lobby movie (one driver)

The lobby movie is one file (`flash_movies/lobby_menu.swf`), and a patch that
carries it replaces the players' movie whole. So it is built one way only:
`build_lobby_026g.py` starts from revision 23's movie (`flash_movies/lobby_menu.swf`
of `resources.db.patch_23`, sha256 `12f0ee1d…`) and applies every lobby change
the repository holds, in order, as `patch_026g.py` does for the exe:

1. the rooms window (`rooms_026g.py`, today's `rooms/SrpRooms.as`, #212 in it);
2. Get Gold opens the exchange (`buy_gold_026g.py`, #213);
3. the exchange window's amounts (`exchange_update_026g.py`, #214);
4. Buy on owned items (`buy_modified_026g.py`, #215);
5. the craft window's menu and buttons (`craft_menu_026g.py`, #216);
6. Optics in a weapon's menu (`scope_context_menu_026g.py`, #217).

A change of a step's source is built in by building again; a new lobby change
is a new entry of `STEPS`. Never ship a movie made by running one step alone
on some movie: it would lack the steps not run. Each step checks its own work
(every method it does not change byte for byte); the steps' own lists of known
inputs name the movies they were first built on, and the driver checks the
base instead.

```sh
python build_lobby_026g.py <patch_23's lobby_menu.swf> <new lobby_menu.swf> --ffdec=/path/to/ffdec.jar
```

Each step's own script refuses to write a movie unless given `--step-only`
(a development build of that one step). The driver's result must be
`RESULT_SHA256`, the movie last checked: a changed source or FFDec gives
another, refused until `RESULT_SHA256` is set to it on purpose (`--any-result`
for development). The login hint and the exchange's first build are frozen in
the base (a change of theirs needs a new base); the exchange's later changes
are a step.

Since the rooms' window under a match's results (2026-10-20): `RESULT_SHA256` is
`57f73166…` (the owner checked `d41b9655…` in the game; then #219's review fixes); with #211 (Leave room / Leave queue, the yellow tab) `a56cd0ed…` (the owner checked `d7907536…`; then the review fixes); with #209 (why a waiting member cannot go into its room's match, `J;-1;<why>`) `57258f9e…`, not yet checked in the game; with the remembered create window (the last room made by hand, `S1;F`) `88fa657a…`, not yet checked in the game. The first build of 2026-10-20 (`main` with
#212): sha256 `8841ba5c…`, deterministic
with FFDec 25.1.3. Its 881 scripts decompile byte for byte to those of the
movie the owner tested with #212 (`ed14345f…`, made the long way: the rooms
class compiled over Optics' movie); the bytecode is laid out differently (the
rooms class compiled first), the non-script tags are the base's in both (every
step keeps them). Tests: `python -m unittest test_build_lobby`. Not in a
release yet.

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
  `73 0F` → `EB 0F`, the same one-byte patch used by `play_skhid.sh`.
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
- option 7 names "uro_fov" for "fov" (0x262930, `mov eax,offset`); the
  instruction's relocation stays active, an address for an address;
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
behind a wall (containers, traps). The code (`pickup_below_026g.asm`) is
at `.srp`+800h; `../docs/notes/client_patch.md` section 3.23 has the details.

**Revisions.** Every game runs every player's use: a game of 11 takes a
battery an older game leaves, so they part there; a server asks for 11
(patch 21 already made it ask).

Checked: `selftest_026g.py` check 24 (the sites, the code and where it
leaves to, the battery's vtables), the relocation and byte checks of the
late join; `check_pickup_below_026g.py` runs the original and the build's
scan on made-up hits (it says nothing about what the real collision world
returns). Run in a game (2026-10-15, two games on Keln): a battery stuck
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
  the weapon: height 1.05, distance 0.85, the side angle the original's.
  Any other item (armour; a weapon only in the stash; a weapon 2 behind a
  weapon 1) gets the original's values and nothing else;
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
  three nops; the least is 0.6 from a weapon's window until the camera is
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
shown. The byte comparison with the build of `main` 5fba57f: 514 bytes
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

**The patch.** Only in the builds with the late join; `suicide_key_026g.asm`,
40 bytes at `.srp`+980h (after the pickup code, before the seek code), one
site: the builder's last action call (spot, `call 0x242460` at 0x242196)
calls it first. It asks `action_present` for action 1Fh with the same
arguments and sets bit 24 when the key went down (one input, as spot), then
goes on to the original call. K while alive in a match: the player dies at
the input's tick, in every game.

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
first page); `check_suicide_key_026g.py` runs the original's and the
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

## Not ported, not done

- The 0.21d mode 3 battery patch is not needed: 0.26g serializes the
  next-battery time. (Patch 9 above is another defect, 0.26g's own.)
- A defect of the client itself is known and not patched: an input with the
  "previous weapon" bit from a player who holds something other than its two
  weapons can crash every client of a match
  (`../docs/OPEN-ISSUES.md`, 26-24).
