# Player guide: playing Survarium 0.69d on a Skhid (Схід) server

Skhid (Схід) is a private server for **Survarium 0.69d**, the last version of the game that was on Steam (February 2022).
Someone runs a Skhid server (your **server operator**); you play on it with your own game. Nothing in the game is
changed or patched.

**What you need**

* A Windows PC with **Survarium 0.69d** installed, as Steam installed it, without changes. Other versions do not work.
* From your server operator: the **server address** (for example `203.0.113.5` or `play.example.org`), the small setup
  scripts (see below), and the account rules of that server (normally: you make up your account name and password at
  your first sign-in).

**What you will end up with:** one setup run, and after that the game starts on the Skhid server with one click: from
Steam's **Play** button, or from a start file `Skhid.cmd` on your desktop. You sign in and play.

Contents: [Easiest: Steam](#easiest-steam) · [Without Steam: the start file](#without-steam-the-start-file) ·
[Signing in](#signing-in) · [Language](#language) · [Known limits](#known-limits) · [Troubleshooting](#troubleshooting) ·
[Appendix: making the start file by hand](#appendix-making-the-start-file-by-hand)

## Easiest: Steam

If Survarium is **in your Steam library**, use Steam's own **Play** button (**Steam mode**). You also get the Steam
overlay, "playing Survarium" on your Steam profile and Steam achievements on your real Steam account.

1. Get the four files `steam-play-setup.cmd`, `steam-play-setup.ps1`, `steam-play-undo.cmd` and `steam-play-undo.ps1`
   from your server operator (the server's `scripts/steam-play` folder). Put them together in one folder.
2. Double-click **`steam-play-setup.cmd`**, type the server address and press **Enter**.
   You should see `Written: ...\Survarium-Steam\launcher.cfg` and `Server check: ... answers. Good.`
3. In Steam, select **Survarium** and press **Play**. Sign in with your Skhid account name and password (not your Steam
   password).

All the details, what you see at each step and the Steam troubleshooting are on the [Steam page](notes/steam.md). Steam
mode has a few extra limits ([Known limits](#known-limits)).

## Without Steam: the start file

No Survarium in your Steam library, or you prefer to start without Steam? A setup script writes a start file
`Skhid.cmd` on your desktop. It works without Steam running.

1. **Get the setup files.** Your server operator gives you `skhid-setup.cmd`, `skhid-setup.ps1`, `skhid-undo.cmd` and
   `skhid-undo.ps1` (the server's `scripts/player` folder). Put them together in one folder.
2. **Double-click `skhid-setup.cmd`.** If Windows shows "Windows protected your PC", click **More info** -> **Run
   anyway**. (The scripts are plain text; you can open them in Notepad and read them.)
3. A window asks:

   ```
   Server address (from your server operator, e.g. 203.0.113.5):
   ```

   Type the server address and press **Enter**.
4. It offers the game language:

   ```
   Game languages: english, russian, ukrainian, german, french, polish, spanish, portuguese, turkish, chinese
   Language (press Enter for english):
   ```

   Press **Enter** for English, or type another language from the list and press **Enter**.
5. The script looks for your game. You should see `Game found: ...\game\binaries\x64\survarium.exe`.
   If it does not find the game, it asks for the game folder: in Steam, right-click **Survarium** -> **Manage** ->
   **Browse local files**, click the address bar of the folder window that opens, copy the text (Ctrl+C), paste it into
   the script's window (right-click) and press **Enter**.
6. The script asks the server for its start data (its address and port). You should see
   `Server answers: login port 65000.` and then
   `Written: C:\Users\<you>\Desktop\Skhid.cmd` with the three lines it wrote.
7. Press any key to close the window. You should see a new icon named `Skhid` (or `Skhid.cmd`) on your desktop.
8. **Double-click `Skhid`.** A black window flashes for a moment and the game starts, without the Survarium launcher
   window and without BattlEye: no BattlEye window or message appears ([Known limits](#known-limits)).
9. **Sign in** at the game's sign-in screen ([Signing in](#signing-in)).

The script changes nothing in the game folder and downloads no programs: it reads three plain text values from the
server, checks them, and writes only `Skhid.cmd`. To remove the start file, double-click **`skhid-undo.cmd`** (you
should see `Deleted: ...\Skhid.cmd`); it deletes only a `Skhid.cmd` that the setup wrote.

**Other options.** Open the folder with the scripts, click the address bar, type `cmd` and press Enter. In the black
window type, for example:

```
skhid-setup.cmd -Server 203.0.113.5 -Language russian -Game "D:\Games\Survarium"
```

* `-Language` the game language, `-Game` your game folder if the script does not find it.
* `-Folder` another folder for `Skhid.cmd` instead of the desktop (use the same `-Folder` with `skhid-undo.cmd`).
* `-UpdatePort <number>` only if your operator gave you another port than 65001.
* Running the setup again replaces its own `Skhid.cmd` (a new server address or a new language).

## Signing in

At the game's sign-in screen type an **account name** and a **password**.

* **The first time:** a new name creates your account with that password. There is no separate sign-up screen. (Some
  servers turn this off; then the operator creates your account and tells you the name and password.)
* The game then asks for your **nickname** (the name other players see) and starts the tutorial, as for a new player in
  the original game.
* **After that:** sign in with the same name and password. "Remember me" works.
* **Forgot your password?** There is no reset inside the game. Ask your server operator to set a new one.

## Language

The game languages are `english`, `russian`, `ukrainian`, `german`, `french`, `polish`, `spanish`, `portuguese`,
`turkish` and `chinese`.

* **Start file:** run `skhid-setup.cmd` again and pick another language, or change the word after `-localization=` in
  `Skhid.cmd` (right-click -> **Edit**; on Windows 11: **Show more options** -> **Edit**).
* **Steam mode:** the game uses the language set in Steam (right-click Survarium -> **Properties** -> **General** ->
  **Language**). Only if you want a different one, run `steam-play-setup.cmd -Server <server address> -Language russian`
  (any language from the list); run it again without `-Language` to go back to Steam's language.

## Known limits

* **No anti-cheat.** Survarium came with BattlEye, an anti-cheat that cannot work with a private server (it needs a
  license and servers of its own). So the game starts without it. The server checks money, items, rewards and match
  results itself, so cheats that change those do nothing; aim or wall cheats are not detected. Report cheaters to your
  server operator. The game shows no BattlEye window or message (tried with Steam's Play button).
* **Steam mode needs Survarium in your Steam library.** Survarium is no longer on the Steam store (since June 2022).
  Only Steam accounts that added it before still have it. Without it, use the start file.
* **Steam mode sends your password in clear text** from the game's Steam "Import" panel to the server (the game does it
  that way; the normal start sends it protected). Use a password you use nowhere else.
* **No "remember me" in Steam mode:** you type the password at every start.
* **Search and Destroy and Arms Race need real players.** The other modes get AI players when too few people are
  online; these two don't, so they start only when enough players search for them. Tick another mode too if you don't
  want to wait.
* **Only what 0.69d itself had.** Services of the original game such as payments or account linking with Google, VK or
  e-mail do not exist on Skhid. Gold comes only from the game's own rewards.

## Troubleshooting

| What you see | Why | What to do |
|---|---|---|
| `NOT DONE: no start data from <address>:65001 (...)` | Wrong address, the server is off, or port 65001 is blocked | Check the address. Ask your operator whether the server runs and whether TCP port 65001 is open. If your operator's server is older and has no start data, use the [appendix](#appendix-making-the-start-file-by-hand). |
| `NOT DONE: ... answered with something that is not Skhid start data` | The address or port belongs to something else | Check the address and `-UpdatePort` with your operator. |
| `NOT DONE: the server's start data has format ...` | The server is newer than your setup files | Get the current `skhid-setup` files from your operator. |
| `NOT DONE: no survarium.exe in '...'` | The folder you gave is not the game folder | Give the `Survarium` folder that Steam's **Browse local files** opens (step 5). |
| `NOT DONE: ...\Skhid.cmd exists and was not written by this script` | You already have a `Skhid.cmd`, for example one you made by hand | Look at it first. To replace it, run `skhid-setup.cmd -Force` from a black window ([Other options](#without-steam-the-start-file)), or delete or rename it. |
| `NOT DONE: the game folder '...' has letters a start file cannot hold` | The game folder's name has letters the black window's text cannot show | Use Steam mode, or install the game in a folder with Latin letters only. |
| `Note: the server gives its address as ...` | The server knows itself under another address than the one you typed | Nothing, if the game connects. If it cannot connect, run the setup again with `-LoginServer <that address>`. |
| Double-clicking `Skhid.cmd` opens Notepad, or nothing happens | (Hand-made file) it was saved as `Skhid.cmd.txt` | Save again with **Save as type: All files** ([appendix](#appendix-making-the-start-file-by-hand), step 5). |
| Windows says it cannot find `survarium.exe` | The game folder moved, or the first line of a hand-made file is wrong | Run `skhid-setup.cmd` again. |
| The game cannot connect, or the sign-in times out | Wrong address or port, or the server is off | Check the address with your operator; ask whether the server runs. |
| The game says the name or password is wrong, for a brand new name | The server does not create accounts at sign-in | Ask the operator for an account. |
| The game says the name or password is wrong, for your own account | Typo, or a forgotten password | After 5 wrong tries in a minute the server ignores that name from your PC for a minute. Wait, then try again, or ask the operator for a new password. |
| Signed in, but the lobby never loads | A firewall or the network blocks the lobby | Tell the operator: the lobby port is not reachable from your PC. |
| The game says you are already online | Your account is signed in elsewhere, or the last session has not ended yet | Wait a little and try again. |
| A match search takes long | Few players are searching | AI players fill the match after a wait the operator sets. |
| The game is in the wrong language | Start file: `-localization=`. Steam mode: Steam's language setting, or `-Language` in the setup | Change it ([Language](#language)). |
| Steam mode: "Steam initialization failed" | Steam is not running, or this Steam account does not have Survarium | [Steam page, troubleshooting](notes/steam.md#troubleshooting). |
| Steam mode: "invalid user or password" for about 4 seconds | Normal: the Import panel always opens with it | Type your name and password and press Import. |
| Steam: the launcher's Play button stays off | The setup script was not run, or the server's update check cannot be reached | [Steam page, troubleshooting](notes/steam.md#troubleshooting). |

## Appendix: making the start file by hand

Use this only if the setup script cannot run on your PC (for example, PowerShell is blocked) or your server has no start
data. You need the **launch line** from your server operator: the command that starts the game and tells it which
server to use. It looks like this (yours has other values):

```
survarium.exe -product_name=Survarium-Steam -server=<server address>:65000 -localization=english
```

* `-server=` is the server's address and port.
* `-localization=` is the game's language ([Language](#language)).

The line has no password key: the server sends the key that protects your password at every sign-in, and the game
keeps it. The game must start in its own folder, so the line goes into a small start file that changes to that folder
first:

1. **Find your game folder.** In Steam, right-click **Survarium** -> **Manage** -> **Browse local files**. A folder
   opens; inside it open `game`, then `binaries`, then `x64`. You should see `survarium.exe` there.
2. **Copy that folder's path.** Click the address bar at the top of the folder window and copy the text (Ctrl+C). For a
   usual Steam install it is:

   ```
   C:\Program Files (x86)\Steam\steamapps\common\Survarium\game\binaries\x64
   ```

3. **Open Notepad** (Start menu, type `Notepad`).
4. **Type these two lines** into Notepad. In the first line, put your folder path from step 2 between the quotes. In the
   second line, after `start "" `, paste the launch line from your operator:

   ```
   cd /d "C:\Program Files (x86)\Steam\steamapps\common\Survarium\game\binaries\x64"
   start "" survarium.exe -product_name=Survarium-Steam -server=<server address>:65000 -localization=english
   ```

   The second line must stay one line.
5. **Save it as a start file.** In Notepad: **File** -> **Save as**. Choose **Desktop** on the left. Set **Save as type**
   to **All files (\*.\*)**. Type the file name `Skhid.cmd` and click **Save**.
   You should see a new icon named `Skhid` (or `Skhid.cmd`) on your desktop.
6. **Double-click `Skhid.cmd`** and [sign in](#signing-in).

If Windows shows "Windows protected your PC" the first time, click **More info** -> **Run anyway**: the file only
contains the two lines you typed. A hand-made `Skhid.cmd` has no marker line, so `skhid-setup.cmd` will not overwrite it
without `-Force` and `skhid-undo.cmd` will not delete it.
