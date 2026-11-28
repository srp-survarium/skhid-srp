# Server guide: running a Skhid (Схід) server for Survarium 0.69d

Skhid (Схід) is a private server for Survarium 0.69d. Its program is called `srp`: in this guide "the `srp` program" or
"srp" is the Skhid server, and its settings file is `srp.toml`.

This guide takes you from an empty computer to a Skhid server your friends can play on. You do not need to be a
programmer: copy the commands one by one. Commands in grey boxes are typed in a **Linux terminal** (on Windows: the
**Ubuntu** window of WSL 2) unless the box says **PowerShell**.

**What you need**

* **A computer that stays on** while people play: Linux (64-bit, x86-64), or Windows 10 / 11 with WSL 2.
* **Survarium 0.69d installed on that computer**, the last Steam build, unmodified (about 24 GB). The server needs the
  game files because it runs the game's own world code from your `survarium.exe` (start worlds, match ends, AI players).
* **Memory:** about 0.2-0.25 GB while idle, then about 0.06-0.15 GB per map in use (a PvE mission map about
  0.2-0.26 GB, the largest map about 0.34 GB) and about 0.04-0.14 GB per running match (measured in test runs; 0.14 GB
  is a 12-player match with 11 AI players; 12 human players were not measured). How much the server may use is yours
  to set with `max_zygotes` and `max_matches`: see "Memory: how many maps and matches" in section `[world_host]` below;
  `srp setup` prints a suggestion for the machine it runs on.
* **Disk:** a few GB for the database and logs.
* **For players outside your home:** a router where you can forward ports, or a rented server (VPS) with a public IP
  address.

**What you will end up with:** the Skhid server running on your computer, a **server address** and two small setup
scripts to give your players, and the knowledge to change settings, keep the server running, back it up and update it.

> Steps marked **(to be confirmed)** have not yet been run on a real setup from start to end: a native Linux install
> among them. Please report what you see.

Contents:

* [Words used in this guide](#words-used-in-this-guide)
* [Quick start](#quick-start)

1. [Install on Windows (WSL 2)](#1-install-on-windows-wsl-2)
2. [Install on Linux](#2-install-on-linux)
3. [Get the server](#3-get-the-server)
4. [Set up: `srp setup`](#4-set-up-srp-setup)
5. [Self-test, start and stop](#5-self-test-start-and-stop)
6. [Try it yourself](#6-try-it-yourself)
7. [Letting players in](#7-letting-players-in)
8. [Configuring the server (`srp.toml`)](#8-configuring-the-server-srptoml)
9. [Events, ranked seasons, store, login calendar, server list](#9-events-ranked-seasons-store-login-calendar-server-list)
10. [Accounts and bans](#10-accounts-and-bans)
11. [Keeping it running](#11-keeping-it-running)
12. [Updating](#12-updating)
13. [Backups](#13-backups)
14. [Logs and troubleshooting](#14-logs-and-troubleshooting)
15. [Known limits and FAQ](#15-known-limits-and-faq)

---

## Words used in this guide

| Word | Meaning |
|---|---|
| **WSL 2** | "Windows Subsystem for Linux": Linux built into Windows 10 / 11. On Windows the server runs inside it. |
| **Terminal** | A window where you type commands. On Windows with WSL: the **Ubuntu** app. |
| **IP address** | A computer's number on a network, like `192.168.1.20` (home network) or `203.0.113.5` (internet). |
| **Port** | A numbered "door" of a program on a computer. Skhid uses four: 65000, 65001, 1235 and 1236. |
| **TCP / UDP** | The two kinds of network traffic. Firewalls and routers ask which one a port uses. |
| **Firewall** | A filter on a computer or router that blocks ports until you allow them. |
| **Port forwarding** | A router setting that sends traffic from the internet on a port to one computer at home. |
| **Config file** | `srp.toml`, a text file with all of the server's settings. `srp setup` writes it. |
| **Login key** | The file `login_key.pem`, the server's secret key. It protects players' passwords on their way to the server. The server sends its public half to the game at every sign-in, so players never set it up. |
| **Launch line** | The command players start the game with. It holds your server's address, port and the game language. |
| **World host** | A helper program (`world-host`) that srp starts and stops by itself. It runs the game's world code for matches. |

## Quick start

The whole path in short; each step links to its details.

1. **Windows:** [install WSL 2](#1-install-on-windows-wsl-2). **Linux:** [check the basics](#2-install-on-linux).
2. [Get the server](#3-get-the-server): download a release package (a few minutes), or build it from source (about
   5-10 minutes). Both put it in the folder `~/srp-069d`.
3. Run the setup; it checks everything and writes the config file and the login key (use your own game folder; on
   Linux, for example, `~/.local/share/Steam/steamapps/common/Survarium`):

   ```sh
   cd ~/srp-069d
   scripts/srp-host.sh setup --game "/mnt/c/Program Files (x86)/Steam/steamapps/common/Survarium"
   ```

4. Check the game's world code once, then start the server:

   ```sh
   scripts/srp-host.sh selftest
   scripts/srp-host.sh run
   ```

5. [Start the game yourself](#6-try-it-yourself) with the player setup script the setup names.
6. [Let your friends in](#7-letting-players-in): address, firewall, port forwarding.

---

## 1. Install on Windows (WSL 2)

The server runs inside WSL 2; the game files stay where Steam put them.

1. Open **PowerShell as administrator**: Start menu, type `PowerShell`, right-click it, **Run as administrator**.
2. Install WSL with Ubuntu:

   ```powershell
   wsl --install -d Ubuntu
   ```

3. Restart Windows when it asks. After the restart an **Ubuntu** window opens and asks for a new Linux user name and
   password. Choose any; they are only for Linux. You should then see a prompt ending in `$`.
4. Check that it is WSL **2** (PowerShell):

   ```powershell
   wsl -l -v
   ```

   You should see `Ubuntu` with `2` in the `VERSION` column. If it says `1`, run `wsl --set-version Ubuntu 2`.

From now on, "Linux terminal" means the **Ubuntu** app from the Start menu. Your Windows drives are inside it as
`/mnt/c`, `/mnt/d`, ...: the usual Steam folder `C:\Program Files (x86)\Steam\steamapps\common\Survarium` is
`/mnt/c/Program Files (x86)/Steam/steamapps/common/Survarium`.

**Memory for WSL (optional):** WSL gets half of your PC's memory by default. To give it a fixed amount, create the file
`C:\Users\<you>\.wslconfig` with Notepad:

```ini
[wsl2]
memory=8GB
```

Then run `wsl --shutdown` in PowerShell; the next Ubuntu window uses the new size.

Go on with [3. Get the server](#3-get-the-server).

## 2. Install on Linux

Any current 64-bit distribution works; the commands in this guide are for Ubuntu / Debian.

The server needs the **game folder**: install Survarium with Steam on Linux
(`~/.local/share/Steam/steamapps/common/Survarium/game`), or copy the `game` folder from a Windows PC. Keep the game's
own layout (`binaries/x64/survarium.exe`, the `resources.NN.db` files, the `resources` folder). The game never runs as a
game on the server; only its files are used.

A native Linux game folder works **(to be confirmed)**: srp shows it to the game's code as drive `G:`.

Go on with [3. Get the server](#3-get-the-server).

## 3. Get the server

Two ways; both end with the server in the folder `~/srp-069d`. A release package is the easy way; build from source
when you want to change the code or run the newest version.

### Download a release (recommended)

The [releases page](https://github.com/srp-survarium/skhid-srp/releases) has two files for each version:
`skhid-<version>-linux-x86_64.tar.gz` (the server) and `skhid-<version>-linux-x86_64.tar.gz.sha256` (its checksum).
In the commands below put the version you download in place of `<version>`. In the Linux terminal:

1. Download both files into your home folder: copy each file's link on the releases page and paste it after `curl -LO`:

   ```sh
   cd ~
   curl -LO <link of the .tar.gz file>
   curl -LO <link of the .sha256 file>
   ```

   On WSL you can also download them with your Windows browser and copy them over:
   `cp /mnt/c/Users/<you>/Downloads/skhid-* ~/`.

2. Check the download:

   ```sh
   sha256sum -c skhid-<version>-linux-x86_64.tar.gz.sha256
   ```

   You should see `skhid-<version>-linux-x86_64.tar.gz: OK`. On `FAILED`, download the file again.

3. Unpack it into `~/srp-069d`:

   ```sh
   mkdir -p ~/srp-069d
   tar -xzf skhid-<version>-linux-x86_64.tar.gz -C ~/srp-069d --strip-components=1
   cd ~/srp-069d
   ```

   The folder now holds the programs `srp` and `world-host`, the `scripts` folder, the example settings
   `config/srp.example.toml` and these guides. Keep `srp` and `world-host` side by side: srp starts the world host from
   next to itself.

The release programs need a 64-bit Linux with glibc 2.34 or newer: Ubuntu 22.04, Debian 12 or newer (WSL's Ubuntu
too). On an older system, build from source.

Go on with [4. Set up](#4-set-up-srp-setup).

### Build from source

About 5-10 minutes, once. In the Linux terminal:

1. Install the build tools:

   ```sh
   sudo apt update
   sudo apt install -y build-essential git curl
   ```

2. Install Rust (the programming language srp is written in):

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
   source "$HOME/.cargo/env"
   ```

3. Download the source. The server is the folder `srp-069d` of the repository; the second line makes it reachable as
   `~/srp-069d`, the folder this guide uses:

   ```sh
   git clone https://github.com/srp-survarium/skhid-srp.git ~/skhid-srp
   ln -s ~/skhid-srp/srp-069d ~/srp-069d
   cd ~/srp-069d
   ```

4. Build the server and the world host:

   ```sh
   cargo build --release -p srp -p world-host
   ```

   You should see `Finished` at the end. The programs are now `target/release/srp` and `target/release/world-host`.
   Keep them in the same folder: srp starts the world host from next to itself.

On WSL keep the source folder inside Linux (like `~/skhid-srp`), not under `/mnt/c`: building there is much faster.

The build uses the Rust version named in `rust-toolchain.toml`; rustup downloads it by itself on the first build.

To make a release package yourself: `scripts/release.sh` builds both programs and writes
`dist/skhid-<version>-linux-x86_64.tar.gz` with its `.sha256` file. Build packages for other people on an older
distribution (Ubuntu 22.04 or Debian 12): the programs then run on that one and on everything newer.

### For contributors

* **Tests:** `cargo test --workspace -- --test-threads=2`. Tests that need the game tables read them from the folder
  named in `SRP_TEST_DATA` (build it once: `target/release/srp extract --game <game folder> --out <folder>`), else from
  `data/` in the source folder (ignored by git). Without tables those tests pass at once with a note;
  `SRP_REQUIRE_TEST_DATA=1` makes them fail instead, use it before a pull request:

  ```sh
  SRP_TEST_DATA=~/srp-tables SRP_REQUIRE_TEST_DATA=1 cargo test --workspace -- --test-threads=2
  ```

  A few game-data tests also compare with reference values of an earlier implementation (`expected_069d.json`,
  `skills_expected_069d.json` in the same folder). Those files hold game data and are not published; the tests skip
  without them.
* **Before a commit:** run `scripts/check-public.sh` (`--summary` adds counts per category and folder). It lists
  everything that must not go into the public repository: folders of your own computer, e-mail and IP addresses
  outside the example / documentation ranges, private keys, committed game data, decision notes and disassembly in
  comments. It exits 1 when it finds something; pass files or folders to check only those.
* **Comments and notes:** say in your own words what the game does and what our code does about it. Keep exe
  addresses as short references (`0x20034CA80`, `+0x2E0`) and the tags PROVEN / LIKELY / CHOICE / OPEN; the longer
  rules and their sources go to `docs/notes/`.

## 4. Set up: `srp setup`

The setup checks this computer, builds the game tables from your game install, writes the config file, creates the
database and the login key, and prints the launch line. Run it in the folder where the server should keep its files. This guide uses `~/srp-069d`: the config file, the
database, the login key and the logs all go there.

1. Go to the folder:

   ```sh
   cd ~/srp-069d
   ```

2. Run the setup with your game folder:

   ```sh
   scripts/srp-host.sh setup --game "/mnt/c/Program Files (x86)/Steam/steamapps/common/Survarium"
   ```

   `--game` takes the Steam `Survarium` folder, the `game` folder inside it, or `survarium.exe` itself. Without
   `--game` the setup looks in the usual Steam folders (also extra Steam library folders).

3. Read the result. Each line starts with `ok`, `WARNING` or `PROBLEM`; a `PROBLEM` line is followed by `->` and what to
   do. When everything is fine you should see:

   ```
   Setup complete. Check the game's world code once (about a minute), then start the server:
   ```

   followed by `Players set up their game once with one of the scripts ...`: the Steam setup line
   (`steam-play-setup.cmd -Server ...`), the start file line (`skhid-setup.cmd -Server ...`) and the **launch line**
   for a start file made by hand. You do not need to copy them now: `scripts/srp-host.sh check` prints them again.

What the setup checks, in order:

| Line | What it means |
|---|---|
| system | Linux or WSL 2. WSL 1 is refused (the world host needs WSL 2). |
| game folder | where `binaries/x64/survarium.exe` was found |
| survarium.exe | the exact 0.69d build (13,386,392 bytes); any other `survarium.exe` is refused |
| game resources | the game's `resources.NN.db` files can be read |
| world host program | `world-host` next to `srp` |
| config | `srp.toml` written (first run) or updated; your other edits are kept |
| game tables | the server's tables (items, shop, quests, drops, maps, bots: 32 files, about 2 MB) built from your game's own files into `data/` next to the config; takes a few seconds. Nothing is downloaded and nothing is written into the game folder. |
| pre-warm lists | the folder `data/prewarm`: every match on a map adds the resources it loaded to that map's list, so later matches on that map start faster and share them; a map without a list yet uses the list of another variant of the same map and mode, else the part every map of its kind has in common; that common part is loaded once for all maps |
| ports free | the four ports are free ([7. Letting players in](#7-letting-players-in)) |
| memory, disk | free memory for the maps loaded at once; free disk for the database and logs |
| database | `srp-069d.sqlite` created on the first run, with the number of accounts |
| login key | `login_key.pem` created on the first run. **Keep it secret**: it decrypts the players' passwords. |

Options:

* `--address <IP>`: the IPv4 address players connect to ([Which address](#which-address)). Without it, the first setup
  uses this computer's own network address.
* Another config file than `./srp.toml`: name it right after `setup`, like
  `scripts/srp-host.sh setup other.toml --game ...`; the other commands take it the same way (`scripts/srp-host.sh run
  other.toml`).

You can run `srp setup` again at any time; it keeps what is already right and builds the game tables again. Stop the
server first, otherwise the ports line says they are in use. **After a game update or a new srp release, run `srp
setup` again** so the tables match. `srp` refuses to start without them (`no game tables in ...: run srp setup`).

## 5. Self-test, start and stop

All commands run in `~/srp-069d`.

1. **Self-test** (once after the setup, and after updates). It checks that the game's world code runs on this computer
   (about a minute, about 0.6 GB memory; it does not touch a running server):

   ```sh
   scripts/srp-host.sh selftest
   ```

   It starts a world host of its own, builds the start world of one map and plays the first 30 seconds of a match with
   five AI players on it (the 20-second countdown, then 10 seconds of play; faster than real time). You should see
   `OK: the world host runs the game's world code and its AI players; online matches can start.` If not, the `FAILED`
   line names the part that failed ([troubleshooting](#14-logs-and-troubleshooting)).

2. **Start the server:**

   ```sh
   scripts/srp-host.sh run
   ```

   The log scrolls in the window. You should see lines like:

   ```
   players start the game with: survarium.exe -product_name=Survarium-Steam -server=... -localization=english
   login on tcp 0.0.0.0:65000, lobby on udp 0.0.0.0:1235 (client is told 192.168.1.20:1235)
   match server on udp 0.0.0.0:1236 (clients are told 192.168.1.20)
   ```

   Keep this window open while people play.

3. **Stop the server:** press **Ctrl+C** in that window. The world host stops with it.

Other commands:

```sh
scripts/srp-host.sh check     # print the settings, the launch line and the players' setup lines; "ready" at the end
scripts/srp-host.sh status    # is it running?
scripts/srp-host.sh start     # Linux only: run in the background (log in logs/srp.log)
scripts/srp-host.sh log       # follow the log (Ctrl+C stops following, not the server)
scripts/srp-host.sh stop      # stop a server started with start
```

The first match on a map waits a few seconds while that map loads (on a test PC about 1 s for a PvP map and 3 s for
a PvE mission, plus about 3 s right after the server started); no map is loaded at start (`prestart = []`).

## 6. Try it yourself

Before you invite anyone, start the game on the server PC (or another PC at home) as a player would:

1. Copy the four files of the `scripts/player` folder to that PC (on WSL the folder is also reachable from Windows, for
   example `\\wsl$\Ubuntu\home\<you>\...`).
2. Follow the [player guide, Without Steam](guide-player.md#without-steam-the-start-file) with the address
   `scripts/srp-host.sh check` shows in its `start file` line. You should see `Server answers: login port 65000.`
   and `Written: ...\Skhid.cmd`; the server log shows `player start data /skhid/client.txt sent to ...`.
3. Sign in with a new name and password. You should see the nickname screen; in the server log a `login of ...` line
   appears.

If this works on the server PC but not on other PCs, the address or the network is the problem: see the next section.

## 7. Letting players in

### The ports

| Port | Kind | What |
|---|---|---|
| 65000 | TCP | login (`-server=<server address>:65000` in the launch line) |
| 1235 | UDP | lobby |
| 1236 | UDP | matches |
| 65001 | TCP | update check for the Steam Play button ([steam.md](notes/steam.md)); it also serves the start data (`/skhid/client.txt`: login address and port) that players' `skhid-setup.cmd` fetches once |

All four can be changed in `[network]` ([section 8](#network-addresses-and-ports)).

### Which address

The server tells the players' games where the lobby and the matches are, so it must know the address **players** use.
Choose one:

* **Only players on the server PC itself:** the address the first setup chose works. (On WSL it is the Linux side's own
  address, `172.x.x.x`, which Windows on the same PC reaches. It can change after a Windows restart:
  [troubleshooting](#14-logs-and-troubleshooting).)
* **Players in your home network:** the server PC's home network address, like `192.168.1.20`. Find it with
  `hostname -I` (Linux) or `ipconfig` (Windows, "IPv4 Address"). On WSL see [WSL and other PCs](#wsl-and-other-pcs-to-be-confirmed).
* **Players from the internet:** your public IP address (your router shows it, or any "what is my IP" website), plus
  [port forwarding](#port-forwarding-on-the-router).

Set it (stop the server first):

```sh
scripts/srp-host.sh setup --address 192.168.1.20
```

You should see `updated  config ...: network` and the new launch line. The setup makes every service listen on all
network connections of the PC (`0.0.0.0`) and tells players `192.168.1.20`. **Give your players the new address**; they
run their setup script again with it.

A host name (for example from a dynamic DNS service) can be written by hand into `match_public`
([section 8](#network-addresses-and-ports)); the lobby address must be an IPv4 address.

### Firewall on Linux

If you use `ufw`:

```sh
sudo ufw allow 65000/tcp
sudo ufw allow 65001/tcp
sudo ufw allow 1235/udp
sudo ufw allow 1236/udp
```

On a VPS also open the four ports in the provider's firewall panel.

### WSL and other PCs (to be confirmed)

In WSL's default network mode only the Windows PC itself reaches the server. Windows' port forwarding
(`netsh interface portproxy`) handles TCP only, and the lobby and matches use UDP. So for players on other PCs switch WSL
to **mirrored** network mode (Windows 11 22H2 or newer):

1. Add to `C:\Users\<you>\.wslconfig` (Notepad):

   ```ini
   [wsl2]
   networkingMode=mirrored
   ```

2. Allow the ports through the WSL firewall (PowerShell as administrator):

   ```powershell
   New-NetFirewallHyperVRule -Name srp-login -DisplayName "srp login" -Direction Inbound -VMCreatorId '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' -Protocol TCP -LocalPorts 65000,65001
   New-NetFirewallHyperVRule -Name srp-game -DisplayName "srp lobby and matches" -Direction Inbound -VMCreatorId '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' -Protocol UDP -LocalPorts 1235,1236
   ```

3. Run `wsl --shutdown` in PowerShell and open Ubuntu again.
4. In `~/srp-069d` run `scripts/srp-host.sh setup --address <the Windows PC's home network address>`.

In mirrored mode Linux and Windows share the PC's addresses, so the home network address works for players on the
server PC too, and it does not change at every restart. Windows Defender Firewall may also ask once to allow the
connection.

### Port forwarding on the router

For players from the internet, open your router's settings (often "Port forwarding" or "Virtual server") and forward
TCP 65000, TCP 65001, UDP 1235 and UDP 1236 to the server PC's home network address. Then run
`scripts/srp-host.sh setup --address <your public IP>`.

Players inside your own home network then also use the public IP. That needs a router that supports it ("NAT loopback"
or "hairpin NAT"); otherwise they cannot connect from inside (**to be confirmed** per router).

## 8. Configuring the server (`srp.toml`)

`srp.toml` is a text file in `~/srp-069d`. Open it with any editor, for example in the terminal:

```sh
nano srp.toml
```

(In `nano`: Ctrl+O then Enter saves, Ctrl+X quits.)

* Every setting is explained in the file itself. Every value in it is the default, except the lines `srp setup` filled
  in (the network addresses and the game folder).
* **After a change, restart srp** (Ctrl+C, then `scripts/srp-host.sh run`).
* A setting with a typo is refused at start with its name, so a mistake never silently falls back to a default.
* Lines starting with `#:` are examples: remove the `#: ` to use them.
* Relative paths are relative to the folder of `srp.toml`. Times are UTC: `2026-12-24T12:00:00Z`, or just `2026-12-24`.

The full list with every setting: [config/srp.example.toml](../config/srp.example.toml). The most useful ones:

### `[network]`: addresses and ports

As `srp setup --address 203.0.113.5` writes them:

```toml
[network]
login = "0.0.0.0:65000"           # TCP, where the login listens (players: -server=<server address>:65000)
lobby = "0.0.0.0:1235"            # UDP, the lobby
lobby_public = "203.0.113.5:1235" # IPv4 address:port players reach the lobby at ("" = the lobby address)
match = "0.0.0.0:1236"            # UDP, the match server
match_public = "203.0.113.5"      # host name or IPv4 players reach the matches at ("" = the match IP)
update = ""                       # TCP, the Steam launcher's update check ("" = the login IP, port 65001)
```

`0.0.0.0` means "all network connections of this PC" and is right for any server others play on. `127.0.0.1` (the
default in the example file) means "this computer only".

### `[files]`

```toml
[files]
database = "srp-069d.sqlite"   # the accounts database, created at the first start
login_key = "login_key.pem"    # the login key (secret); made when missing
data_dir = "data"              # the game tables `srp setup` builds from your game install
run_dir = "run"                # files srp makes for the world host
log_dir = "logs"               # logs (section 14)
```

**A new login key** (changed or lost) needs nothing from players: the server sends its public half at every sign-in.
Keep `login_key.pem` secret and back it up (section 13): it decrypts the players' passwords.

### `[accounts]`

```toml
[accounts]
auto_register = true     # a new name is created by its first sign-in; false = only accounts you create (section 10)
client_version = ""      # "" = any game version string
#: start_money = { silver = 1000000, gold = 10000, parts = 1000 }   # money for NEW accounts (testing, events)
```

### `[world_host]`

```toml
[world_host]
exe = "/mnt/c/Program Files (x86)/Steam/steamapps/common/Survarium/game/binaries/x64/survarium.exe"
max_matches = 4      # matches at once per map (up to twice as many while the machine has 1.2 GB available)
max_zygotes = 8      # maps loaded at once (0.06-0.35 GB each, shared by its matches), a map still unloading included
idle_secs = 600      # a map without matches for this long is unloaded
prestart = []        # maps loaded at start ([] = maps load when a match needs them)
```

`exe` is written by `srp setup`. The other path settings normally stay empty (`prewarm_dir` "" = `data/prewarm`).

`max_zygotes` is a hard limit: a match for a map that is not loaded unloads the least recently used map that runs no
match (`prestart` maps last) and waits until it is gone. A map that runs matches is never unloaded; while every loaded map
runs matches, the new map loads one over the limit if the machine has 1.4 GB available (the next new map unloads back
down to the limit), else the new match waits up to 20 s for one to finish and is then refused. A map is never loaded
twice. The queue does not send players into such a match: srp reads the world host's room about every 0.5 s, and when no
new map can load it starts the match on a loaded map of the group's modes that takes one more, or keeps the group
waiting in the queue until one does (see "Limiting the maps loaded" below).

On a PC with little memory: the `max_zygotes` that `srp setup` prints (`1` = one map loaded at a time; a map that is not
loaded needs a few seconds more before its first match) and no `prestart`.

#### Memory: how many maps and matches

The numbers below are measured in test runs and played matches, except where they say "estimate" (no test run had 12
human players); your own server logs the real ones (see "Reading the real numbers").

**What takes memory**

| Part | Memory | How many |
|---|---|---|
| srp (login, lobby, match relay) | about 40 MB, plus about 5 MB per running match | one |
| World host base process (the game code's start-up, shared by every map) | about 0.12 GB, about 0.1 GB of it shared with every map; about 0.17 GB once it also holds the resources common to the maps' lists (see below) | one (replaced every 6 hours; for a while the old one stays with the maps it started) |
| A loaded map | PvP about 0.06-0.1 GB (0.11-0.15 GB while the maps have no common lists yet); the largest map, `level_11` (its three modes), about 0.28-0.31 GB (it loads in about 5 s); PvE mission about 0.2 GB (0.26 GB without common lists) | at most `max_zygotes` (one more only while 1.4 GB is free) |
| A running match | PvP with 11 AI players filling 12 slots (the default `bots_per_team = 6`) about 0.13-0.14 GB; with 15 AI players in 16 slots (`bots_per_team = 8`) 0.1-0.12 GB measured with level-1 equipment (a match grows with the equipment it carries); with 2 players and no AI players about 40-45 MB; PvE (17 AI players) about 0.1-0.12 GB | at most `max_matches` per map (up to twice that only while 1.2 GB is free) |

Maps differ a lot: the level itself is 31-98 MB on the usual PvP maps, 185-192 MB on a PvE mission and 281-283 MB on
`level_11` (as a loaded map 308 MB, 277 MB with the common part in the base process); the rest of a map is its list of the equipment and animations its matches load (about 38-54 MB on a PvP
map, 65 MB on a mission), which its matches then share. Most of that list is the same on every map: once two or more
maps have recorded lists, the world host loads the common part once in its base process (about 47 MB there) and each
map holds about 51 MB less (measured: a PvP map 112 -> 61 MB, a PvE mission 252 -> 200 MB). A map whose matches have
not recorded a list yet starts from the part every map of its kind shares: its first match takes about 70 MB instead
of 39 MB with two players, and from its second match on the loaded map has taken in what the first one recorded.

**What counts as a map:** every map, mode and variant the game data has is a map of its own (about 110: for example
`level_02_day_br_v1` ... `_v4` are four, `level_02_day_tdm` and `level_02_day_ctp` two more, each PvE mission one). Two
matches share a loaded map only when they are on exactly the same one. AI players cost about as much as human players
(their equipment): a 12-player match with 11 AI players measured 126-131 MB at its start and at most 140.5 MB, on a
map started from the common part of the lists.

**The most it can use** with your settings (every loaded map full):

    about 0.35 GB + max_zygotes x (map + max_matches x match)

with each map and match of the largest kind (0.35 GB a map, 0.165 GB a match; the largest measured match is 0.14 GB):
`srp setup` counts this way. Keep that under the machine's memory minus what the system and other programs need (at
least 0.5 GB on a server of its own; on a PC with WSL, the memory WSL has). The over-the-limit rules only use memory that is free at that moment, so
they never push the server past this.

**By players online.** Each row is the number of players online at the same time; the other columns follow from it.
Players do not split into full matches: queues fill unevenly, small groups get AI players, some play a PvE mission
alone or in two. A match costs about the same whatever the number of people in it (AI players carry equipment too; a
PvE mission always has its 17 AI players), so what counts is the number of matches. Assumed for the table, at the
busiest moment: every online player is in a match, 3 players per match on average, a quarter of the matches are PvE
missions (6 mission maps, so several missions share one).

**Maps loaded** assumes that every PvP match is on a map of its own. A loaded map is one map in one mode and one
variant (`level_02_day_br_v3` is not `level_02_day_br_v4`, nor `level_02_day_tdm`): two matches share it only when the
queue gave both exactly that one. The queue rotates through about 100 such PvP maps and draws the variant, so at a
given moment few matches share one; this column is the upper end. The table has no limit on the maps loaded; the next
table shows a limit.

| Online | Matches at once (PvP + PvE) | Maps loaded | PvE missions per mission map | Memory, usual | Memory, worst case | Server memory | `max_zygotes` | CPU |
|---|---|---|---|---|---|---|---|---|
| 10 | 4 (3 + 1) | 4 | 1 | 1.2 GB | 2.1 GB | 4 GB | 5 | 0.6 core |
| 20 | 7 (5 + 2) | 7 | 1 | 2.0 GB | 3.4 GB | 4-6 GB | 8 | 1.1 cores |
| 50 | 17 (13 + 4) | 17 | 1 | 4.3 GB | 7.8 GB | 10-12 GB | 19 | 2.5 cores |
| 100 | 34 (26 + 8) | 32 | 2 | 8.0 GB | 14.5 GB | 16 GB | 34 | 5.1 cores |
| 200 | 67 (50 + 17) | 56 | 3 | 14.2 GB | 25.9 GB | 32 GB | 58 | 10 cores |

* Memory, usual = 0.25 GB + PvP maps x 0.085 GB + mission maps x 0.2 GB + PvP matches x 0.13 GB + PvE matches x
  0.12 GB: the measured sizes once the maps have recorded their lists (the common part then sits in the base
  process); a PvP match counts as one filled with AI players (with the default settings a match short of players gets
  them after `bot_wait_secs`: 11 AI players in 12 slots measured 0.13-0.14 GB; two players without AI players take
  41-43 MB). A new server's first matches, before the lists exist, take more. Worst case = 0.35 GB + PvP maps x
  0.25 GB + mission maps x 0.35 GB + PvP matches x 0.165 GB + PvE matches x 0.16 GB: room for maps and matches larger
  than any measured so far.
* Server memory = the worst case plus about 0.5 GB for the system, rounded up to a usual size.
* `max_zygotes` = the maps loaded plus one or two kept loaded for the next matches (no limit in practice).
* `max_matches = 4` covers every row (missions per mission map stay under it); raise it only for many PvE players.
* CPU: about 0.15 of a server core per match filled with AI players, for a server core three times slower than a test
  PC (estimate for 11 AI players: a 16-slot match with 15 AI players measured about 5.7 % of the test PC's core for
  the match and its relay, 0.17 taken three times slower; the AI players' own thinking is under 1 %, the cost is the
  world of 12-16 players); a two-player match without AI players about 0.05.
* The biggest part is one map per PvP match. Fewer maps in the queue's rotation, or more players per match, lower it:
  with 6 players per match the matches and maps halve.

**Limiting the maps loaded.** A lower `max_zygotes` saves the memory of the maps, not of the matches: the matches
still run, more of them on each loaded map. srp reads the world host's room about every 0.5 s. While a new map can load
(under the limit, an idle map to unload, or one over the limit while 1.4 GB is free), the queue picks the map by its
rotation as always. When none can, the queue starts the match on a loaded map of the group's ticked modes that takes one
more match: up to `max_matches` per map, and up to twice that while 1.2 GB is free, so fewer groups wait. A group waits
in the queue only when no loaded map of its modes has a free match slot, and starts as soon as one has or a map can load
again. PvE missions do the same with their two mazes. This covers the PvP queue (with its AI-player fill) and PvE
missions; custom rooms and clan battles play the map their players chose and do not check the room. Without a status
from the world host (none read yet, or the last one older than 5 s) the queue picks as it did before. The mission maps
stay loaded (their missions share them); the rest of the limit goes to PvP maps. Same assumptions as the
table above (3 players per match, a quarter PvE), at the busiest moment, with `max_matches = 4`:

| Online | `max_zygotes` | Maps for missions + PvP | PvP matches running | Matches per PvP map | PvE missions | PvP matches waiting in the queue | Memory, usual | Memory, worst case | Server memory |
|---|---|---|---|---|---|---|---|---|---|
| 10 | no limit (5) | 1 + 3 | 3 | 1 | 1 | 0 | 1.2 GB | 2.1 GB | 3 GB |
| 10 | 3 | 1 + 2 | 3 | 2 | 1 | 0 | 1.1 GB | 1.9 GB | 3 GB |
| 10 | 2 | 1 + 1 | 3 | 3 | 1 | 0 | 1.0 GB | 1.6 GB | 3 GB |
| 20 | no limit (8) | 2 + 5 | 5 | 1 | 2 | 0 | 2.0 GB | 3.4 GB | 4 GB |
| 20 | 5 | 2 + 3 | 5 | 2 | 2 | 0 | 1.8 GB | 2.9 GB | 4 GB |
| 20 | 3 | 2 + 1 | 4 | 4 | 2 | 1 | 1.5 GB | 2.3 GB | 3 GB |
| 50 | no limit (19) | 4 + 13 | 13 | 1 | 4 | 0 | 4.3 GB | 7.8 GB | 10 GB |
| 50 | 10 | 4 + 6 | 13 | 3 | 4 | 0 | 3.7 GB | 6.0 GB | 8 GB |
| 50 | 6 | 4 + 2 | 8 | 4 | 4 | 5 | 2.7 GB | 4.2 GB | 6 GB |
| 100 | no limit (34) | 6 + 26 | 26 | 1 | 8 | 0 | 8.0 GB | 14.5 GB | 16 GB |
| 100 | 20 | 6 + 14 | 26 | 2 | 8 | 0 | 7.0 GB | 11.5 GB | 12 GB |
| 100 | 12 | 6 + 6 | 24 | 4 | 8 | 2 | 6.0 GB | 9.2 GB | 10-12 GB |
| 200 | no limit (58) | 6 + 50 | 50 | 1 | 17 | 0 | 14.2 GB | 25.9 GB | 32 GB |
| 200 | 32 | 6 + 26 | 50 | 2 | 17 | 0 | 12.2 GB | 19.9 GB | 24 GB |
| 200 | 16 | 6 + 10 | 40 | 4 | 17 | 10 | 9.5 GB | 14.3 GB | 16 GB |

Memory with a limit, usual: 0.25 GB + mission maps x 0.2 GB + PvP maps x 0.085 GB + PvP matches running x 0.13 GB +
PvE missions x 0.12 GB (a PvP match counts as one filled with AI players); worst case: 0.35 GB + mission maps x 0.35 GB + PvP maps x 0.25 GB + PvP matches running x
0.165 GB + PvE missions x 0.16 GB; with the PvP matches running = the PvP matches, at most PvP maps x 4.
Server memory = the worst case plus 0.5 GB, rounded up. The waiting column counts at `max_matches = 4` per map; with
1.2 GB free a map takes up to 8, so fewer wait. It is the worst moment: a match that ends frees its slot for the next
group, and an idle map can be unloaded for another one.

* **Variety narrows, then comes back.** While the memory limits the maps, the queue's matches are on the loaded maps
  only. It takes turns among them: the mode and the map whose last match started the longest ago (a freshly loaded map
  at most once before the others get their turn again), and the variants of a loaded map in turn too. As soon as a map
  can load again, the next match follows the rotation over every map.
* **The oldest loaded map is rotated out.** While no map can load, the world host picks one busy map to drain: the one
  loaded the longest, not a `prestart` map. The queue starts no new match on it, nor on an idle map, as long as a busy
  loaded map fits the group (a group that fits only there still plays there). Its matches end and it becomes idle. As
  soon as an idle map can be unloaded for a new one, the queue avoids the maps that load would unload, so the next match
  that needs a new map loads the rotation's next map and the idle map goes; then the next map drains, one at a time. A
  map that stays busy for 20 minutes (custom rooms, clan battles, missions with nowhere else to go keep it busy) is passed
  over and the next one drains. So under a steady full load the loaded maps keep changing, at the pace of the matches
  that end.
* **One rare mode waits longer.** A group that ticked only one mode nobody else plays may find no loaded map of it, and
  none can load until a map is idle.
* A limit is worth it when the memory is short; with enough memory leave `max_zygotes` above the maps you see loaded at
  your busiest time (`front:` lines of `logs/world-host/front.log`). `srp.log` shows each narrowed choice (`queue: no new
  map can load: narrowed to ...`, with the candidates, the map being rotated out and why one was picked) and each held
  group once (`queue: held, ...`).

**Suggested settings by memory** (`max_matches = 4`, every loaded map full, about 0.5 GB left for the system):

| Memory for the server | Worst case: every map of the largest kind | Usual PvP maps (measured sizes) |
|---|---|---|
| 2 GB | `max_zygotes = 1` | `max_zygotes = 3` with `max_matches = 2` (or 1 map with 4) |
| 4 GB | `max_zygotes = 3` | `max_zygotes = 5` |
| 8 GB | `max_zygotes = 7` | `max_zygotes = 12` |
| 16 GB | `max_zygotes = 15` | `max_zygotes = 25` |

`srp setup` prints both numbers for the memory it finds (`this memory fits N map(s) in the worst case ... or about M
usual PvP maps`); when fewer than 3 usual PvP maps fit, it also prints the usual fit with `max_matches = 2` (`; with
max_matches = 2 about K usual PvP maps`): on a small machine the same memory then holds more maps with fewer matches
each, so the queue has more variety. Worst case: 0.35 GB + `max_zygotes` x (0.35 GB + `max_matches` x 0.165 GB), safe whatever the
players choose. Usual PvP maps: 0.35 GB + `max_zygotes` x (0.085 GB + `max_matches` x 0.13 GB), PvP maps of the
measured size once their lists' common part is in the base process, with matches filled with AI players (11 AI
players in 12 slots: 0.13-0.14 GB). A value between the two is a reasonable start; the
over-the-limit rules only use memory that is free at that moment. The large maps are few: the six PvE missions
(0.2-0.26 GB each) and `level_11`'s three modes (0.28-0.31 GB each).

More maps loaded means fewer matches that wait for a map to load (a few seconds each); more matches per map only helps
when many players queue for the same map. With few players, lower `max_matches` frees nothing: a match that does not run
costs nothing.

**Same-page merging (optional, saves memory):** the world host marks its memory for the Linux kernel's same-page
merging (KSM). With it switched on, the kernel keeps one copy of pages that are byte-identical in several matches or
maps (for example the same AI players' equipment in every PvE mission of one map) until one of them changes the page.
Results do not change. It is off until you switch it on, as root:

    echo 1 > /sys/kernel/mm/ksm/run

(to keep it after a restart, put that line in a start-up script, for example a systemd unit or `/etc/rc.local`). It
costs some CPU for the kernel's `ksmd` thread: `pages_to_scan` and `sleep_millisecs` in the same folder set how fast it
scans; the defaults are gentle. `/sys/kernel/mm/ksm/pages_sharing` shows how many pages it saves (times 4 kB). The
world host's log says at start whether its memory takes part (`memory merging (KSM): ...`). On WSL the kernel has KSM
too. The tables above do not count on it: whatever it saves comes on top. Measured in test runs: about 27 MB for the
base process and two idle maps, about 10 MB per loaded map and per running match on any map, and about 30-40 MB per
PvE mission when several run on one mission map (five missions on one map: about 185 MB). Its CPU cost on a one-core
server has not been measured yet: compare the server's CPU with it on and off.

**Reading the real numbers:** every process of the world host writes `MEM [...] smaps_rollup:` lines to its log. The
memory a process uses alone is its `Private_Dirty`; `Rss` also counts the pages it shares and would count the shared
part once per map.
* A map: `MEM [zygote ready]` in `logs/world-host/<map>/z.../host.log`.
* A match: `MEM [child: end]` in `logs/world-host/<map>/z.../m<match>-<pid>/host.log` (at its end, its largest).
* The base: `MEM [base: at the cut]` in `logs/world-host/base/b.../host.log`.

Add them up as in the formula above and adjust `max_zygotes` / `max_matches`.

### `[matches]`

```toml
[matches]
min_players = 2          # players a queue group needs before an online match starts
bot_wait_secs = 30       # after this many seconds AI players fill the match (0 = never)
bots_per_team = 6        # players per team of a match AI players fill, 1..8 (also a custom room's AI maximum)
report_grace_secs = 20   # wait for the players' result reports before paying out
equipment_use_costs = [0, 0, 1000, 2000, 5000, 8000]   # silver a match costs by equipment level 0..5
match_silver_factors = [1.0, 1.0, 3.5, 7.0, 10.0, 14.0] # silver reward factor by level 0..5
```

With few players: `min_players = 1` and a short `bot_wait_secs` let one player start a match with AI players.

`bots_per_team` fills each team of such a match up to that many players. A map's own match size and its AI paths
keep it lower where needed: a 12-player map gets at most 6 per team, a 14-player map 7, and no map more than 8 (every
map the AI plays has 8 paths per team, and each AI player needs a path of its own). More AI players means more memory
and CPU per match: the sizes in [Memory: how many maps and matches](#memory-how-many-maps-and-matches) were measured
with at most 11 AI players in a 12-player match (6 per team, up to 0.14 GB); with 8 per team a match on a larger map
can hold 15 AI players, which no test run has measured, so plan for larger matches.

Game modes in the online queue: the modes of the game's own map list, among them Search and Destroy (random and ranked).
Search and Destroy matches are players only: its maps have no paths for AI players, so `bot_wait_secs` never fills
them with AI players ([notes/online.md](notes/online.md), section 7.4).
Arms Race (gun race, all versus all, 2 to 8 players) is in the queue too, players only and never ranked
([notes/online.md](notes/online.md), section 7.5).

### `[social]`

```toml
[social]
claims_per_day = 0        # complaints one player may file per day (0 = no limit)
compliments_per_day = 0
```

### `[league]`, `[store]`, `[[game_servers]]`, `[[login_calendar]]`, `[[events]]`, `[[faction_battles]]`

See the next section.

## 9. Events, ranked seasons, store, login calendar, server list

None of these needs a setting to work: without entries the built-in defaults apply. Entries replace the defaults.

### Events

The game files carry no dates, so an event runs only when you schedule it. **No entry = no event.** The only event the
0.69d data has is the New Year event (snowflakes, spruce trees and stars from PvP matches, exchanged in the event window
for New Year 2022 rewards; the first PvP win gives a winter hat). Copy this into `srp.toml` and change the dates:

```toml
[[events]]
event = "new_year"
start = 2026-12-24T12:00:00Z          # matches pay the event currencies from here ...
end = 2027-01-10T12:00:00Z            # ... until here
exchange_end = 2027-01-17T12:00:00Z   # the event window and the exchange stay open until here
header = "st_merry_christmas"         # window header (a game text key)
text = "st_global_event_currency2_descr"
```

Restart srp. The log (and `scripts/srp-host.sh check`) says `event new_year: ... (starts in N h)`, `(RUNNING now)` or
`over`, or `event: no event scheduled`. Several entries are allowed (this year and next) if they do not overlap. Ranked
matches are closed while an event runs (as in the game). Details: [notes/events.md](notes/events.md).

### Ranked (league) seasons

Without entries a new season starts on the 1st of every month (UTC). Your own seasons:

```toml
[league]
tokens = []          # season-end tokens per league 1..10 ([] = 25 x league^2)
[[league.seasons]]
number = 1
start = 2027-01-01T00:00:00Z
end = 2027-02-01T00:00:00Z
[[league.seasons]]
number = 2
start = 2027-02-01T00:00:00Z
end = 2027-03-01T00:00:00Z
```

### Store offers and premium

Built in: one "special offer" tab with the 7-day premium card and the Winner's Contract; premium for 3 days, a week, a
month or 3 months. To replace them, see the commented `[[store.tabs]]`, `[[store.offers]]` and `[[store.premium]]`
example in `config/srp.example.toml`. Texts are the game's own text keys (`st_...`), shown in each player's language.
An offer grants `items`, `premium` hours or the `winners_contract`. Gold comes only from the game's own sources on this
server (there is no payment). Details: [notes/store.md](notes/store.md).

### Daily login calendar

Built in: a 7-day cycle (energy drink, rations, radios, stash map, reputation, experience and silver boosters). Your own:

```toml
[[login_calendar]]
item = 340        # an item id of the game data
amount = 1
# ... one entry per day; star = true marks the big reward (day 7)
```

### Game servers list

The game's server window lists servers. Without entries: one entry "Skhid" on your match address. Your own (at most 8):

```toml
[[game_servers]]
host = ""          # "" = this server's match address
label = "My server"
x = 238            # the marker on the window's map
y = 47
```

### Faction battles

No entry = no battle. Example: `start`, `end` and `factions = [1, 2]` (two of the four factions) in a
`[[faction_battles]]` entry (commented in the example file).

## 10. Accounts and bans

**Open registration** (default, `auto_register = true`): the first sign-in with a new name creates the account with that
password. There is no separate sign-up screen.

**Closed registration** (`auto_register = false`): only accounts you create can sign in.

All commands below run in `~/srp-069d` and work while the server runs:

```sh
scripts/srp-host.sh account add <name> <password>        # a new account (same start as a first sign-in)
scripts/srp-host.sh account password <name> <password>   # set a new password (a player forgot theirs)
```

You should see `account "<name>" created ...` or `account "<name>" has a new password`. There is no password change
inside the game: `account password` is the way. Start the command with a space to keep the password out of your
terminal history (Ubuntu's terminal does that by default).

Bans and complaints:

```sh
scripts/srp-host.sh ban <name> ban <reason> <minutes>      # a ban; 0 minutes = permanent
scripts/srp-host.sh ban <name> suspend <reason> <minutes>  # a suspension
scripts/srp-host.sh unban <name>
scripts/srp-host.sh comments [name [count]]                # complaints and compliments players filed
```

`<reason>` is a number 0..15, one of the game's own ban reason texts (shown to the player at sign-in); 15 shows the
"soft ban" text. With another config file put `SRP_CONFIG=<file>` in front of the command.

A player can be signed in once at a time; a second sign-in with the same name is refused. After five wrong passwords
within a minute a name is locked for a minute from that address.

## 11. Keeping it running

* **WSL:** start with `scripts/srp-host.sh run` and **leave the Ubuntu window open**. Windows stops WSL, and the server
  with it, when no Ubuntu window or other WSL program is open; a background `start` stops too. Starting the server
  automatically with Windows is not covered yet (**to be confirmed**; for example a Windows scheduled task that runs
  `wsl -d Ubuntu -- bash -lc "cd ~/srp-069d && scripts/srp-host.sh run"`).
* **Linux:** `scripts/srp-host.sh start` runs the server in the background; it keeps running after you close the
  terminal. To start it after every reboot, add this line with `crontab -e` (**to be confirmed**):

  ```
  @reboot cd /home/<you>/srp-069d && scripts/srp-host.sh start
  ```

* Check now and then: `scripts/srp-host.sh status`, and free disk space (`df -h .`): the logs grow with every match.

## 12. Updating

1. Stop the server: Ctrl+C in its window, or `scripts/srp-host.sh stop`.
2. Make a backup of the database:

   ```sh
   cd ~/srp-069d
   cp srp-069d.sqlite srp-069d.sqlite.before-update
   ```

3. Get the new version:

   * **Release package:** download and check the new package as in [section 3](#download-a-release-recommended), then
     unpack it over the old one:

     ```sh
     tar -xzf ~/skhid-<new version>-linux-x86_64.tar.gz -C ~/srp-069d --strip-components=1
     ```

     The programs, scripts and guides are replaced; `srp.toml`, the database, the login key and the logs are not in
     the package and stay as they are.
   * **Built from source:**

     ```sh
     git pull
     cargo build --release -p srp -p world-host
     ```

4. Check again (your `srp.toml`, database and key stay as they are):

   ```sh
   scripts/srp-host.sh setup
   scripts/srp-host.sh selftest
   ```

5. Start: `scripts/srp-host.sh run`.

Read the release notes first: they say when a setting was added or renamed. Before the first public release the database
format may still change; if the new version refuses your database, the release notes say what to do.

## 13. Backups

Everything that matters is in three files in `~/srp-069d`: the database `srp-069d.sqlite`, the login key
`login_key.pem` and `srp.toml`.

* **Server stopped:** copy the three files somewhere safe.
* **Server running:** copy the database with SQLite's backup command (install it once with `sudo apt install sqlite3`):

  ```sh
  sqlite3 srp-069d.sqlite ".backup 'backup-$(date +%F).sqlite'"
  ```

  A plain `cp` of a running database can miss the latest changes (they are still in `srp-069d.sqlite-wal`).
* **Restore:** stop srp, put the backup in place of `srp-069d.sqlite` (delete `srp-069d.sqlite-wal` and
  `srp-069d.sqlite-shm` if present), start srp.

A daily backup at 5:00 with cron (`crontab -e`; create the `backups` folder first; old backups are not deleted
automatically; on WSL cron runs only while WSL runs):

```
0 5 * * * cd /home/<you>/srp-069d && sqlite3 srp-069d.sqlite ".backup 'backups/srp-$(date +\%F).sqlite'"
```

## 14. Logs and troubleshooting

Logs are in `~/srp-069d/logs/` (setting `files.log_dir`):

* `srp.log`: login, lobby and matches (with `run` also on the screen).
* `world-host.log`: the world host's own output.
* `world-host/`: one folder per map and match (the game's world code).

For more detail start with `RUST_LOG=debug scripts/srp-host.sh run`.

| What you see | Why | What to do |
|---|---|---|
| Setup: `PROBLEM ... is not the Survarium 0.69d build this server needs` | Another game version, or a changed `survarium.exe` | Use the last Steam build of 0.69d, unmodified. In Steam: Properties -> Installed files -> Verify integrity. |
| Setup: `no Survarium install found in the usual Steam folders` | The game is somewhere else | Give the folder: `srp setup --game "<folder>"` (WSL: drive `C:` is `/mnt/c`). |
| Setup or start: `... is in use: is srp already running ...` | srp already runs, or another program uses the port | `scripts/srp-host.sh status`; stop the old one (Ctrl+C in its window, or `scripts/srp-host.sh stop`), or choose another port in `[network]`. |
| `no world host program at ...` | `world-host` is not next to `srp` | Release: unpack the whole package. Source: `cargo build --release -p srp -p world-host`. |
| Self-test: `FAILED ...` | The world code cannot run | Read the named part and `logs/selftest/`; check free memory. |
| Self-test: `WARNING hardware breakpoints refused` | Linux refuses the performance events the world host reads deaths with; matches run, but without the kill log (kills per weapon, kill achievements) | `sudo sysctl kernel.perf_event_paranoid=2`; to keep it after a restart, put the line `kernel.perf_event_paranoid = 2` in a file `/etc/sysctl.d/60-skhid.conf`. |
| Players cannot connect at sign-in | Wrong address in their launch line, or TCP 65000 blocked | Check the address ([7](#which-address)), firewall and port forwarding; on WSL see [WSL and other PCs](#wsl-and-other-pcs-to-be-confirmed). |
| Players sign in, then the lobby never loads | UDP 1235 blocked, or `lobby_public` is an address players cannot reach | Firewall / port forwarding for UDP 1235; `srp setup --address <address players use>`. |
| The lobby works, matches do not start or time out | UDP 1236 blocked, `match_public` wrong, or the world host fails | Firewall / port forwarding for UDP 1236; `scripts/srp-host.sh selftest`; `logs/world-host.log`. |
| At start: `the world host did not answer on ... within 30s` | The world host could not start: short of memory, or the game's code failed | `logs/world-host.log`; `scripts/srp-host.sh selftest`; free memory or lower `max_zygotes`. |
| An online search finds nothing | Fewer than `min_players` searching | AI players join after `bot_wait_secs`; or lower `min_players`. |
| A player's `skhid-setup.cmd` says `no start data from ...` | TCP 65001 not reachable, or the server is off | Firewall / port forwarding for TCP 65001; the log shows `player start data ... sent to ...` when it works. |
| WSL: worked yesterday, today nobody connects | The WSL address changed after a Windows restart | Stop srp, find the new address with `hostname -I`, run `srp setup --address <new address>`, give players the new line; or switch to [mirrored mode](#wsl-and-other-pcs-to-be-confirmed). |
| WSL: the server stops when you close the window | Expected: WSL stops with its last window | Keep the Ubuntu window open ([11](#11-keeping-it-running)). |
| Steam players: the launcher's Play button stays off | TCP 65001 not reachable | Firewall / port forwarding for TCP 65001; their check shows in the log as `update check from ...` ([steam.md](notes/steam.md#for-server-operators)). |
| Steam players: "Steam initialization failed" | On the player's PC: Steam not running, or no Survarium in that Steam library | Nothing on the server; [steam.md, troubleshooting](notes/steam.md#troubleshooting). |
| Steam players: "invalid user or password" for about 4 seconds | Normal in Steam mode | Nothing; they type name and password and press Import. |
| A player forgot the password | | `scripts/srp-host.sh account password <name> <new password>` ([10](#10-accounts-and-bans)). |

## 15. Known limits and FAQ

**Known limits**

* **No anti-cheat.** The game ships with BattlEye, which cannot work with a private server (it needs a license and
  servers of its own). The server decides money, rewards, prices, items and match results, so cheats that change those
  on the player's PC do nothing. Aim or wall cheats are **not detected**. Use `srp comments` and `srp ban` for players
  who misbehave.
* **Steam mode only for players who already have Survarium in their Steam library**; it is no longer on the Steam
  store. Everyone else uses the start file of the player guide.
* **Steam mode sends the password in clear text** from the game's Import panel (the normal start sends it protected).
  Tell Steam players to use a password they use nowhere else.
* **No "remember me" in Steam mode.**
* **Some game modes are players only.** AI players fill Battery, Team Deathmatch, Research and Slaughter after
  `bot_wait_secs`, never Search and Destroy (its maps have no paths for AI players; 10 players, 5 versus 5) or Arms Race
  (all versus all, 2 to 8 players, never ranked). On a small server those two rarely start; players can tick other
  modes as well, and the queue then plays one of those ([8](#8-configuring-the-server-srptoml), `[matches]`).
* **Ranked:** when the ranked slot's mode can't start (for example Search and Destroy with too few players), ranked
  plays the next mode of the rotation instead.
* **Not yet tried live:** a native Linux game folder (**to be confirmed**).

**Can players start from Steam?** Yes, if Survarium is in their Steam library: [notes/steam.md](notes/steam.md). Your
part: TCP 65001 reachable ([7](#7-letting-players-in)). `scripts/srp-host.sh check` prints the setup line for them.

**Where are passwords stored?** The server stores only a strong one-way hash (Argon2) of what the game sends. The login
key protects the sign-in on the way; keep `login_key.pem` secret and out of shared folders.

**Do players need anything from the server?** Only your server address and the setup scripts (`scripts/steam-play` for
Steam mode, `scripts/player` for the start file; [player guide](guide-player.md)). Players use their own game; the
server uses its own copy for the world host.

**Does this project contain game files?** It contains no game program and no game code; the world host loads your
installed `survarium.exe`, and `srp setup` builds the game tables from your own install. The project ships no game
tables; only its own figures (shop prices from the fan wiki's price tables, a few prices of its own).
