# Skhid (Схід): a private server for Survarium 0.69d

**Skhid (Схід)** lets you run your own server for **Survarium 0.69d**, the last version of the game on Steam (February
2022). Players use their own, unmodified game and connect to your server. Online PvP and PvE matches, AI players, the
lobby, progression, the store, clans, friends and chat run on your server.

The Skhid server program is called `srp`; its settings file is `srp.toml`.

This project contains **no game files and no game code**. The server runs the game's own world code (start worlds,
match ends, AI players) from the `survarium.exe` of **your own** game install and builds its game tables (items,
prices, quests, maps) from that install's files, so the server computer needs the game installed too.

## Which guide do I need?

* **I want to play on a Skhid server:** [Player guide](docs/guide-player.md). Easiest: with Survarium in your Steam
  library, run `steam-play-setup.cmd` once and press Play ([Steam page](docs/notes/steam.md)). Without Steam, run
  `skhid-setup.cmd` once; it puts a start file on your desktop.
* **I want to run a Skhid server:** [Server guide](docs/guide-server.md): install, set up, let friends in, settings,
  backups, updates, troubleshooting.

## What a server needs

* A computer with **Linux** (64-bit), or **Windows 10 / 11 with WSL 2** (the Linux built into Windows).
* **Survarium 0.69d** installed on that computer: the last Steam build, unmodified. Other builds are refused with a
  message.
* **Memory:** about 25 MB idle; each map in use about 0.35-0.56 GB, shared by all matches on it; each running match
  about 60-150 MB more. About 1.5 GB covers two maps and several matches; the default allows up to eight maps loaded at
  once (up to about 5 GB; set `max_zygotes` lower on a small machine). A few GB of disk for the database and logs.
* A release package for Linux x86-64 (glibc 2.34 or newer: Ubuntu 22.04, Debian 12 or newer, WSL's Ubuntu), or the
  source to build yourself (about 10 minutes). The server guide shows both.

## Quick start for server operators

Download `skhid-<version>-linux-x86_64.tar.gz` and its `.sha256` file from the
[releases page](https://github.com/srp-survarium/skhid-srp/releases) into your home folder. Then, in a Linux terminal
(on Windows: the Ubuntu window of WSL 2):

```sh
cd ~
sha256sum -c skhid-<version>-linux-x86_64.tar.gz.sha256
mkdir -p ~/srp-069d && tar -xzf skhid-<version>-linux-x86_64.tar.gz -C ~/srp-069d --strip-components=1
cd ~/srp-069d
scripts/srp-host.sh setup --game "/mnt/c/Program Files (x86)/Steam/steamapps/common/Survarium"
scripts/srp-host.sh selftest
scripts/srp-host.sh run
```

1. The first three lines check the download (`OK`) and unpack it into `~/srp-069d`. To build from source instead, see
   [server guide, section 3](docs/guide-server.md#3-get-the-server).
2. `setup` checks the computer and the game, builds the server's game tables from your game install (a few seconds,
   about 2 MB in `data/`; run `setup` again after a game or server update), writes `srp.toml`, creates the database
   and the login key, and prints the setup lines for your players. `--game` is your game folder; on WSL a Windows drive `C:` is `/mnt/c`. Without
   `--game` it looks in the usual Steam folders.
3. `selftest` checks that the game's world code and its AI players run on your computer (about a minute). You should
   see `OK: ...`.
4. `run` starts the server. Keep the window open; Ctrl+C stops it.

Give your players your server address and the setup scripts (`scripts/steam-play` for Steam's Play button,
`scripts/player` for the start file); they follow the [player guide](docs/guide-player.md). For players on other PCs or
from the internet, see [Letting players in](docs/guide-server.md#7-letting-players-in).

## Known limits

* **No anti-cheat.** The game's BattlEye cannot be used with a private server. The server decides money, items, prices,
  rewards and match results, so cheats on those do nothing; aim or wall cheats are not detected.
* **Steam mode** (Steam's Play button) works only for players who already have Survarium in their Steam library. In
  Steam mode the game sends the password in clear text, and there is no "remember me".
* **Some game modes need real players.** AI players fill Battery, Team Deathmatch, Research and Slaughter after a short
  wait, but not Search and Destroy (5 versus 5, its maps have no paths for AI players) or Arms Race (all versus all,
  2 to 8 players). Those two start only with enough real players; Arms Race is never ranked.
* Not yet tried live: a native Linux game folder.

Details: [player guide](docs/guide-player.md#known-limits), [server guide](docs/guide-server.md#15-known-limits-and-faq).

## More

* Settings reference: [config/srp.example.toml](config/srp.example.toml) and [docs/notes/config.md](docs/notes/config.md).
* How the parts work (for developers): [servers/srp/README.md](servers/srp/README.md) and the notes in
  [docs/notes](docs/notes).

Skhid is not affiliated with the makers of Survarium. You need your own copy of the game.
