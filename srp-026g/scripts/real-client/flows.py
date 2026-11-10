#!/usr/bin/env python3
"""Real-client test flows of the Watch window and the replay viewer, on real game windows.

    python flows.py <flow>... --game <folder of survarium_skhid.exe> --server-log <file> --host <address> --shots <folder>

A flow prints each step as it is done and stops, with a screenshot, at the first step that does not happen.
Coordinates are client-area pixels of a 1600x1200 game window (`r_resolution` in user.cfg).

The local test accounts come from the environment: SRP_TEST_ACCOUNTS=name:password,name:password.

Start from a freshly started server, and after a flow that failed. A game that is killed leaves its account's room
and match on the server: the room's vote runs on, its match starts, and the account is put back in that match when
it signs in. A flow that passes leaves its match first, so flows can follow each other.
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import time

from driver import LEFT_DOWN, LEFT_UP, Again, Game, Run, Stop, mouse, send

PASSWORDS = dict(pair.split(":", 1) for pair in os.environ.get("SRP_TEST_ACCOUNTS", "").split(",") if pair)
WATCH_MENU = (1099, 52)
PLAY = (799, 44)
TAB_LIVE = (665, 139)
TAB_REPLAYS = (940, 139)
LIVE_FRIENDS = (265, 195)
REPLAYS_FRIENDS = (238, 195)
REPLAYS_EVERYONE = (340, 195)
CLOSE_WATCH = (1490, 137)
JOIN_ROOM = (917, 1134)
CARD_WATCH = (1396, 268)  # the first card's Watch button
CARD_STEP = 82  # the height of a card
BACK = (288, 28)
ROOMS_MENU = (1018, 52)
RANKING_MENU = (1182, 52)
RANKING_ROW = (638, 492)  # the second row of the Ranking's list
RANKING_MATCHES = (700, 527)  # "Matches" in its right-click menu, the second entry
QUEUE_SPECTATE = (1031, 1136)
SEARCH_BOX = (1290, 195)
MAGNIFIER = (1480, 194)  # the search box's
CARD_CODE = (383, 296)  # the first Replays card's match code
TOP_MENU = {"character": (441, 40), "equipment": (547, 40), "shop": (653, 40), "clan": (934, 40), "rooms": ROOMS_MENU,
            "ranking": (1183, 40)}
MOMENTS = {"all": (1065, 1161), "mine": (1110, 1161), "followed": (1172, 1161)}
SEEK_BAR_Y, SEEK_BAR_X = 1146, (640, 960)
PAUSED_MOST = 5 * 60  # playback.rs
PROFILE_2 = (794, 1129)
BLACKOUT = 6  # the mode without a HUD, and so without the kit bar
FRIENDS = (108, 1183)
FRIEND_ROW = (110, 876)  # the first friend in the friends panel
FRIEND_SPECTATE = (150, 907)  # its right-click menu's Spectate, for a friend in a match
# The results screen's row, centred: the match's code, Watch replay, Save replay, Close.
RESULTS_CODE = (610, 881)
RESULTS_SAVE = (812, 881)
RESULTS_CLOSE = (920, 881)
LEAVE_MATCH = (799, 593)
VIEWER_PLATES, VIEWER_PLATE_Y = (999, 1156, 1310), 901  # the kit plates in a replay's viewer
PLAYER_ROW_X = 1495  # a viewer's player list
PLAYER_ROWS = (("ReplayTester", 560), ("SecondTester", 583))
LIVE_DELAY = 30  # SRP_LIVE_WATCH_DELAY_S's default
DAILY_CLOSE = (1032, 539)  # the X of the first sign-in of a day's reward box
# A room's three map cards: where to sample each mode label's box, and where to click to vote for the card.
CARD_LABELS = (132, 598, 1064)
CARD_LABEL_Y = 337
CARD_VOTES = ((338, 440), (804, 440), (1270, 440))

began = time.time()


def window_open(picture) -> bool:
    """A lobby window (Watch, Rooms) is up: its title band, one flat dark colour on every window. The body is no sign: a
    long list fills it, and the floor of a dark match can look like it."""
    band = picture[125:155, 1100:1450]
    return 10 < band.mean() < 22 and band.std() < 3  # not 0: the black of a loading screen is flat too


def lobby_clear(picture) -> bool:
    return not window_open(picture)


def third_row_text(picture) -> tuple[int, int]:
    """The width and the left edge of the bright text along the row of the menu's third button."""
    row = picture[586:603, 715:885].min(axis=2) > 150
    columns = row.any(axis=0).nonzero()[0]
    return (int(columns.max() - columns.min() + 1), int(columns.min() + 715)) if len(columns) else (0, 0)


def match_menu_open(picture) -> bool:
    width, left = third_row_text(picture)
    return 66 <= width <= 76 and 758 <= left <= 768


def leave_box_open(picture) -> bool:
    """The box's second line of text runs along the same row, wider."""
    width, left = third_row_text(picture)
    return 112 <= width <= 134 and 730 <= left <= 745


def daily_box_open(picture) -> bool:
    """The reward box of an account's first sign-in of a day: modal, so a press anywhere else does nothing. Its title
    band (left of the title, whose length is the account's) is darker than a window's and its footer nearly black; it
    is only ever over the lobby, whose red PLAY shows dimmed behind it, so a dark place of a match is no box."""
    title, footer = picture[530:548, 556:600], picture[652:670, 552:572]
    if not (8 < title.mean() < 19 and title.std() < 6 and 1 < footer.mean() < 5 and footer.std() < 2):
        return False
    play = picture[15:45, 735:865].astype(int)
    return bool(((play[:, :, 0] > 40) & (play[:, :, 0] > 2 * play[:, :, 1])).mean() > 0.25)


def close_daily_box(game: Game) -> None:
    if daily_box_open(game.picture()):
        game.shot(f"daily-box-{game.account}")
        game.click(*DAILY_CLOSE, 8)
        game.wait_screen(lambda picture: not daily_box_open(picture), "the daily reward box closed", 3)
        say(f"{game.account} closed the daily reward box")


def kit_bar_shown(picture) -> bool:
    """A player's kit chooser: a few near-white pixels of each plate's name. The plates are see-through, so what is behind
    them says nothing; a bright scene fills the row with many more pixels than a name has."""
    return all(3 <= (picture[1122:1136, x:x + 60].min(axis=2) > 190).sum() <= 60 for x in (610, 765, 920))


def viewer_kits_shown(picture) -> bool:
    """A replay viewer's kit plates, by each plate's name as in kit_bar_shown. Without the server's note of the kits (a
    server older than the client's) the viewer plays on with no plates at all, and nothing else on the screen says so."""
    return all(3 <= (picture[VIEWER_PLATE_Y - 7:VIEWER_PLATE_Y + 7, x - 30:x + 30].min(axis=2) > 190).sum() <= 60
               for x in VIEWER_PLATES)


def play_ready(picture) -> bool:
    """The lobby's PLAY button, lit: a match's screen has no such red there."""
    button = picture[15:45, 735:865].astype(int)
    return ((button[:, :, 0] > 150) & (button[:, :, 1] < 90)).mean() > 0.25  # the longer word of another language covers more of it


def results_open(picture) -> bool:
    """The results screen: the flat dark strip of its panel, under the rewards."""
    strip = picture[790:850, 850:1250]
    return strip.mean() < 40 and strip.std() < 10 and not window_open(picture)


def in_room(picture) -> bool:
    """The PLAY button reads LEAVE ROOM: its longer white text lifts the button's green."""
    return picture[15:45, 735:865, 1].mean() > 95


def label_widths(picture) -> list[int]:
    """How wide each map card's mode label is: its box is one flat colour, and Blackout's name is the longest."""
    widths = []
    for x in CARD_LABELS:
        row = picture[CARD_LABEL_Y, x:x + 414].astype(int)
        differs = (abs(row - row[0]).max(axis=1) > 70).nonzero()[0]
        widths.append(int(differs[0]) if len(differs) else 414)
    return widths


def vote_away_from_blackout(host: Game) -> None:
    widths = label_widths(host.picture())
    end = time.time() + 3
    while len(set(widths)) == 1 and time.time() < end:  # the cards are drawn after the window
        time.sleep(0.1)
        widths = label_widths(host.picture())
    choice = widths.index(min(widths))
    host.click(*CARD_VOTES[choice])
    say(f"{host.account} voted for card {choice + 1}, label widths {widths}")


def leave_match(game: Game) -> None:
    """Leave by the game's own menu: a game only killed is put back in its place when the account signs in again."""
    game.key("esc")
    # The third row of the lobby's own menu is Quit to Windows, at the same place: only "Leave match" may be clicked.
    game.wait_screen(match_menu_open, "the match's menu, with Leave match", 3)
    game.shot(f"leave-{game.account}-1-esc")
    game.click(*LEAVE_MATCH)
    game.wait_screen(leave_box_open, "the box asking to leave the match", 3)
    game.shot(f"leave-{game.account}-2-box")
    game.key("enter")


def end_match(run: Run, a: Game, b: Game, begun: bool = False) -> None:
    """Both leave once play has begun, and the server has closed the match: the next run finds nothing left of it."""
    if not begun:
        a.expect_server(r"play begins", 40)
    mark = run.server.mark()
    leave_match(a)
    leave_match(b)
    a.expect_server(r"is over: \d+ seats freed", 30, mark)
    say("both left; the match is closed")


def say(text: str) -> None:
    print(f"{time.time() - began:7.1f} s  {text}", flush=True)


def sign_in(run: Run, account: str) -> Game:
    if account not in PASSWORDS:
        raise Stop(f"{account} is not in SRP_TEST_ACCOUNTS")
    game = run.launch(account, PASSWORDS[account])
    game.sign_in()
    say(f"{account} signed in")
    game.dialogs.append((daily_box_open, close_daily_box))  # it comes up some seconds after the lobby, at any step
    leave_old_room(game)
    return game


def leave_old_room(game: Game) -> None:
    """A room outlives a killed game, and the server tells the account about it a few seconds after it signs in; a new
    day's reward box comes the same way."""
    end = time.time() + 6
    while time.time() < end:
        close_daily_box(game)
        if in_room(game.picture()):
            mark = game.run.server.mark()
            game.click(*PLAY, 6)
            game.expect_server(rf"{game.account}: left room \d+", 5, mark)
            game.wait_screen(lambda picture: not in_room(picture), "the PLAY button again", 5)
            say(f"{game.account} left a room from an earlier run")
            return
        time.sleep(0.1)


def open_replays(game: Game) -> None:
    game.click(*WATCH_MENU)
    game.click(*TAB_REPLAYS)


def watch_card(game: Game, row: int = 0) -> str:
    """Press a card's Watch; the replay session's id, once the game is watching."""
    mark = game.run.server.mark()
    game.click(CARD_WATCH[0], CARD_WATCH[1] + row * CARD_STEP)
    game.expect_server(r"spectator 0x[0-9a-f]+ watches", 20, mark)
    say(f"{game.account} is watching")
    return ""


def room_match(host: Game, guest: Game) -> int:
    """A real match of two: the host's PLAY makes a room, the guest joins it from the list."""
    mark = host.run.server.mark()
    close_daily_box(host)
    close_daily_box(guest)
    host.click(*PLAY, 6)
    host.expect_server(r"made room \d+", 20, mark)
    host.wait_screen(window_open, "the room")
    vote_away_from_blackout(host)
    guest.click(*PLAY, 6)
    guest.wait_screen(window_open, "the Rooms list")
    end = time.time() + 2  # PLAY joins a room with a place by itself, or opens the list to choose from
    while not in_room(guest.picture()) and time.time() < end:
        time.sleep(0.1)
    if not in_room(guest.picture()):
        guest.click(*JOIN_ROOM, 8)
    guest.wait_screen(in_room, "the guest in the room", 6)
    say(f"{guest.account} joined {host.account}'s room")
    made = host.expect_server(r"Match 0x[0-9a-f]+ made: map \d+ \([^,]+, mode \d+\)", 120, mark)
    say(made)
    host.expect_server(rf"{host.account} goes to match", 10, mark)
    host.expect_server(r"player \d entered at", 30, mark)
    say("the match has begun: countdown")
    return int(re.search(r"mode (\d+)", made).group(1))


def kit_after_replay(run: Run) -> None:
    """The fix under test: after a replay left by Back, the next real match is an ordinary one, and its kits
    take the mouse."""
    a, b = sign_in(run, "ReplayTester"), sign_in(run, "SecondTester")
    open_replays(a)
    watch_card(a, 2)
    a.cursor_on()
    a.shot("kit-viewer-1-cursor-on")
    mark = run.server.mark()
    a.click(*BACK, 10)
    a.shot("kit-viewer-2-back-clicked")
    a.expect_server(r": Leave at \d+ ms", 6, mark)
    say("left the replay by clicking Back")
    a.expect_server(r"session of match 0x[0-9a-f]+ is over", 20, mark)
    a.wait_screen(window_open, "the Watch window again, after the replay", steady=1.5)
    a.shot("kit-0-back-in-watch")
    a.click(*CLOSE_WATCH)
    a.wait_screen(lobby_clear, "the lobby, with the Watch window closed", 5)
    say("the Watch window came back and closed")
    if room_match(a, b) == BLACKOUT:
        end_match(run, a, b)
        raise Again("the room drew Blackout, which has no kit bar")
    a.shot("kit-1-next-match-before")
    a.wait_screen(kit_bar_shown, "a player's kit bar in the countdown", 10)
    a.click(*PROFILE_2, 8)
    a.shot("kit-2-after-click")
    say("clicked the second kit")
    end_match(run, a, b)


def card_listed(picture) -> bool:
    """The first card's red Watch button is drawn."""
    button = picture[258:278, 1340:1450].astype(int)
    return ((button[:, :, 0] > 70) & (button[:, :, 0] > 2.5 * button[:, :, 1])).mean() > 0.3


def live(run: Run) -> None:
    """A match in progress, as seen from the lobby of a friend of one of its players: the badge, the Live list and
    its Friends chip, watching it and coming back."""
    a, b, c = sign_in(run, "ReplayTester"), sign_in(run, "SecondTester"), sign_in(run, "ThirdTester")
    room_match(b, c)
    a.expect_server(r"play begins", 40)
    begun = time.time()
    a.focus()
    # the badge counts what can be watched now, yellow for a friend's match; its list is asked every 30 s
    a.wait_screen(badge_yellow, "the live badge, yellow for a friend's match", 45)
    a.move(*WATCH_MENU)
    a.wait_screen(badge_tip_shown, "the badge's tip under the Watch tab", 3)
    a.shot("live-1-lobby-badge")
    a.click(*WATCH_MENU)
    a.wait_screen(window_open, "the Watch window")
    a.click(*TAB_LIVE)
    a.wait_screen(card_listed, "a card on the Live tab")
    a.shot("live-2-list")
    say("the Live tab lists the match")
    a.click(*LIVE_FRIENDS)
    a.wait_screen(card_listed, "the friend's match under Friends")
    a.shot("live-3-friends-chip")
    say("the Friends chip lists the friend's match")
    watch_card(a)
    for frame in range(1, 5):
        time.sleep(1.5)
        a.shot(f"live-4-watching-{frame}")
    a.key("2")  # a spectator has no kits: the game's own key must not bring up or pick one
    time.sleep(0.5)
    a.shot("live-5-after-kit-key")
    # The spectator is 30 s behind: only past that is the followed player spawned, and its kit used.
    time.sleep(max(0, begun + LIVE_DELAY + 8 - time.time()))
    a.shot("live-7-past-the-start")
    leave_match(a)  # on a live match, unlike a replay, Esc is the game's own menu
    a.wait_screen(window_open, "the Watch window, after watching", 25, steady=1.5)
    a.shot("live-6-back-in-window")
    say("left the live match and came back to the window")
    a.click(*CLOSE_WATCH)
    a.wait_screen(lobby_clear, "the lobby, with the Watch window closed", 5)
    end_match(run, b, c, begun=True)


def follow(run: Run) -> None:
    """Following a friend from the friends panel, and Save replay on the results screen. One spectate to a game's
    session: Esc, Leave match and another match crash the game on rev53 (#1513, patch 113)."""
    a, b, c = sign_in(run, "ReplayTester"), sign_in(run, "SecondTester"), sign_in(run, "ThirdTester")
    room_match(b, c)
    a.expect_server(r"play begins", 40)
    begun = time.time()
    a.click(*WATCH_MENU)
    a.wait_screen(window_open, "the Watch window")
    a.click(*CLOSE_WATCH)
    a.wait_screen(lobby_clear, "the lobby", 5)
    settle_focus(a)
    a.click(*FRIENDS, 8)  # once: it toggles the panel
    time.sleep(1.5)  # the list is filled when the server has answered
    a.shot("live-7-friends-panel")
    mark = run.server.mark()
    a.right_click(*FRIEND_ROW, 8)
    time.sleep(0.5)
    a.shot("live-8-friend-menu")
    a.click(*FRIEND_SPECTATE, 6)
    asked = a.expect_server(r"asked to watch the replay of match 0x[0-9a-f]+ \([^\n]*", 15, mark)
    a.expect_server(r"spectator 0x[0-9a-f]+ watches", 20, mark)
    say(f"following the friend: {asked[:110]}")
    for frame in range(1, 3):
        time.sleep(2)
        a.shot(f"live-9-following-{frame}")
    # 30 s behind: past the start the followed friend has spawned, so the one kit it used is shown, and no other.
    time.sleep(max(0, begun + LIVE_DELAY + 8 - time.time()))
    a.shot("live-9-following-past-the-start")
    leave_match(a)
    a.wait_screen(lambda picture: window_open(picture) or play_ready(picture), "the lobby, after following", 25, steady=1.5)
    a.shot("live-10-back-from-following")
    say("left the friend's match")

    end_match(run, b, c, begun=True)


def save(run: Run) -> None:
    """Save replay on the results screen: the file arrives in the game's replays folder."""
    a, b = sign_in(run, "ReplayTester"), sign_in(run, "SecondTester")
    room_match(a, b)
    replays = run.game_dir.parents[1] / "replays"
    before = set(replays.glob("*")) if replays.exists() else set()
    end_match(run, a, b)
    for second in range(1, 7):
        time.sleep(1)
        a.shot(f"save-0-after-leaving-{second}")
    a.wait_screen(results_open, "the results screen", 20, steady=1)
    a.shot("save-1-results")
    results_code(a)
    mark = run.server.mark()
    a.click(*RESULTS_SAVE, 6)
    a.expect_server(r"asked to save the replay of match 0x[0-9a-f]+", 10, mark)
    end = time.time() + 40
    while not (set(replays.glob("*")) - before if replays.exists() else set()):
        if time.time() > end:
            raise a.fail("no new file in the replays folder within 40 s")
        time.sleep(0.25)
    new = sorted(path.name for path in set(replays.glob("*")) - before)
    time.sleep(0.5)
    a.shot("save-2-saved")
    say(f"saved from the results screen: {new}")


def results_code(game: Game) -> None:
    """Pressing the results screen's match code selects it: Ctrl+C copies it, typing changes nothing."""
    saved = get_clipboard()
    try:
        set_clipboard("nothing-yet")
        game.click(*RESULTS_CODE, 10)
        time.sleep(0.5)
        game.shot("save-1-code-selected")
        game.type("zz")
        time.sleep(0.5)
        game.click(*RESULTS_CODE, 10)
        time.sleep(0.4)
        game.key_chord("ctrl", "c")
        time.sleep(0.6)
        copied = get_clipboard()
        if not re.fullmatch(r"#[0-9a-f]{8}", copied):  # after the typing above: had it changed the code, it would show
            raise game.fail(f"Ctrl+C did not copy the results screen's match code: {copied!r}")
        game.shot("save-1-code-copied")
        say(f"copied {copied} from the results screen")
    finally:
        set_clipboard(saved or " ")


def kit_view(run: Run) -> None:
    """A replay's viewer sees, read-only, the kits the player in view used: the one it started with and any it picked.
    Shots of each player in view; what they list is compared with the kits picked here."""
    a, b = sign_in(run, "ReplayTester"), sign_in(run, "SecondTester")
    if room_match(a, b) == BLACKOUT:
        end_match(run, a, b)
        raise Again("the room drew Blackout, which has no kit bar")
    a.shot("kits-1-countdown")
    a.wait_screen(kit_bar_shown, "a player's kit bar in the countdown", 10)
    a.click(*PROFILE_2, 8)
    a.shot("kits-2-picked-second")
    a.expect_server(r"play begins", 40)
    time.sleep(25)  # long enough a replay to look at both players in it
    end_match(run, a, b, begun=True)
    a.wait_screen(results_open, "the results screen", 20, steady=1)
    a.click(*RESULTS_CLOSE, 6)
    a.wait_screen(play_ready, "the lobby, after the results", 20, steady=1)
    open_replays(a)
    a.wait_screen(card_listed, "a card on the Replays tab")
    mark = run.server.mark()
    watch_card(a)
    time.sleep(2)
    a.wait_screen(viewer_kits_shown, "the viewer's kit plates", 15)
    a.shot("kits-3-viewer-first-player")
    a.cursor_on()
    for row, (name, y) in enumerate(PLAYER_ROWS, 4):  # the player list on the right: what follows a place
        a.click(PLAYER_ROW_X, y, 10)
        time.sleep(1.5)
        a.shot(f"kits-{row}-viewer-{name}")
    for plate, x in enumerate(VIEWER_PLATES, 1):  # read-only, but with the Ctrl cursor up a plate still lists its kit's items
        a.move(x, VIEWER_PLATE_Y, 20)
        time.sleep(0.6)
        a.shot(f"kits-6-viewer-hover-{plate}")
    a.click(*BACK, 10)
    a.expect_server(r": Leave at \d+ ms", 6, mark)
    a.expect_server(r"session of match 0x[0-9a-f]+ is over", 20, mark)
    a.wait_screen(window_open, "the Watch window, after the replay", 20, steady=1.5)
    a.click(*CLOSE_WATCH, 6)
    a.wait_screen(lobby_clear, "the lobby", 5)
    say("watched the replay's kit view and left it")


def first_click(run: Run) -> None:
    """The first click on Back after Ctrl brings up the viewer's cursor must register. Regression flow: it fails while
    the viewer swallows that click."""
    a = sign_in(run, "ReplayTester")
    a.wait_screen(play_ready, "a lobby that has settled", 20, steady=3)
    misses = quits = 0
    for cycle in range(1, 13):
        open_replays(a)
        a.wait_screen(card_listed, "a card on the Replays tab")
        mark = run.server.mark()
        a.click(CARD_WATCH[0], CARD_WATCH[1])
        started = run.wait(run.server, r"spectator 0x[0-9a-f]+ watches|game came back from match", 20, None, mark)
        if "came back" in started:  # the game quit as it connected (known): pressing Watch again has to work at once
            quits += 1
            say(f"cycle {cycle}: the game quit as it connected; pressing Watch again")
            a.wait_screen(card_listed, "a card on the Replays tab, after the quit", 10)
            mark = run.server.mark()
            a.click(CARD_WATCH[0], CARD_WATCH[1])
            a.expect_server(r"spectator 0x[0-9a-f]+ watches", 20, mark)
        a.cursor_on()
        a.click(*BACK, 10)
        try:
            run.wait(run.server, r": Leave at \d+ ms", 3, None, mark)
        except Stop:
            misses += 1
            a.click(*BACK, 10)
            a.expect_server(r": Leave at \d+ ms", 6, mark)
        a.expect_server(r"session of match 0x[0-9a-f]+ is over", 20, mark)
        a.wait_screen(window_open, "the Watch window, after the replay", 20, steady=1.5)
        a.click(*CLOSE_WATCH, 6)
        a.wait_screen(lobby_clear, "the lobby", 5)
        say(f"cycle {cycle}: {misses} missed so far, {quits} quit and were pressed again")
    if misses:
        raise Stop(f"{misses} of 12 first clicks on Back did nothing")


def watch_open(picture) -> bool:
    """The Watch window itself, not another window: the gold line under its selected tab."""
    def gold(line) -> bool:
        return ((line[:, :, 0] > 150) & (line[:, :, 1] > 100) & (line[:, :, 2] < 110)).mean() > 0.5
    return gold(picture[156:161, 535:800]) or gold(picture[156:161, 808:1075])


BADGE = (slice(8, 36), slice(1100, 1160))  # where the Watch tab's badge is: its plate, dot, count and friend's mark


def badge_shown(picture) -> bool:
    """The live badge on the Watch menu: its red dot."""
    area = picture[BADGE].astype(int)
    return bool(((area[:, :, 0] > 170) & (area[:, :, 1] < 110) & (area[:, :, 2] < 100)).sum() >= 20)


def badge_yellow(picture) -> bool:
    """The badge's yellow person: a friend plays in a match that can be watched."""
    area = picture[BADGE].astype(int)
    return bool(((area[:, :, 0] > 200) & (area[:, :, 1] > 160) & (area[:, :, 2] < 120)).sum() >= 15)


def badge_tip_shown(picture) -> bool:
    """The box under the Watch tab that says what the badge counts: near black, with light words in it."""
    box = picture[78:100, 1040:1160].astype(int).sum(axis=2)
    return bool((box < 60).mean() > 0.5 and (box > 500).mean() > 0.02)


def absent(run: Run, pattern: str, seconds: float, mark: int, what: str) -> None:
    """Fails if `pattern` shows in the server's log within `seconds`."""
    end = time.time() + seconds
    while True:
        found = re.search(pattern, run.server.since(mark))
        if found:
            raise Stop(f"{what}: {found.group(0)[:120]}")
        if time.time() >= end:
            return
        time.sleep(0.2)


def settle_focus(game: Game) -> None:
    """This game in front, and the one that was in a match has let the mouse out of its window."""
    game.focus()
    time.sleep(1.2)


def to_room_list(game: Game) -> None:
    """The Rooms window, the only room selected as it opens."""
    settle_focus(game)
    game.click(*ROOMS_MENU)
    game.wait_screen(window_open, "the Rooms window")
    time.sleep(1.5)


def after_results(game: Game) -> None:
    game.wait_screen(results_open, "the results screen", 25, steady=1)
    game.click(*RESULTS_CLOSE, 6)
    game.wait_screen(play_ready, "the lobby, after the results", 20, steady=1)
    leave_old_room(game)


def queue_spectate(run: Run) -> None:
    """Queue and spectate, which needs a full public room: the server's public rooms must be 1 v 1 for this flow (two
    accounts fill one). It watches behind the match by the Live delay; when a place frees it is let go and seated."""
    a, b, c = sign_in(run, "ReplayTester"), sign_in(run, "SecondTester"), sign_in(run, "ThirdTester")
    room_match(b, c)
    to_room_list(a)
    a.shot("queue-1-full-room")
    mark = run.server.mark()
    a.click(*QUEUE_SPECTATE, 8)
    a.expect_server(r"ReplayTester: watches its queue's room's match 0x[0-9a-f]+", 10, mark)
    behind = a.expect_server(r"a spectator \(session 0x[0-9a-f]+\) connected from [^,]+, \d+ s behind", 10, mark)
    if f"{LIVE_DELAY} s behind" not in behind:
        raise a.fail(f"not behind by the Live delay: {behind}")
    say(behind)
    a.expect_server(r"spectator 0x[0-9a-f]+ has loaded", 30, mark)
    time.sleep(2)
    a.shot("queue-2-waiting-for-the-delayed-clock")
    a.expect_server(r"spectator 0x[0-9a-f]+ starts from the snapshot", 45, mark)
    a.expect_server(r"spectator 0x[0-9a-f]+ watches", 10, mark)
    say("watching, behind the match")
    time.sleep(3)
    a.shot("queue-3-watching")
    mark = run.server.mark()
    leave_match(c)  # a place frees
    a.expect_server(r"ReplayTester: watches match \S+ no more: a place came", 15, mark)
    a.expect_server(r"spectator 0x[0-9a-f]+ is let go", 5, mark)
    a.expect_server(r"ReplayTester goes to match 0x[0-9a-f]+", 15, mark)
    a.expect_server(r"new place \d caught up", 30, mark)
    say("a place came: let go and seated")
    time.sleep(4)
    a.shot("queue-4-seated")
    end_match(run, a, b, begun=True)


def paused(run: Run) -> None:
    """A viewer plays on past its memory report; a replay left paused ends at 5 minutes, a seek restarts that count, and
    the game returns to the Watch window on the farewell. Takes about 9 minutes."""
    a = sign_in(run, "ReplayTester")
    a.wait_screen(play_ready, "a lobby that has settled", 20, steady=3)
    open_replays(a)
    a.wait_screen(card_listed, "a card on the Replays tab")
    mark = run.server.mark()
    a.click(*CARD_WATCH, 8)
    a.expect_server(r"spectator 0x[0-9a-f]+ watches", 20, mark)
    time.sleep(8)  # past the moment its game sends a memory report, which the server drops: the viewer plays on
    absent(run, r"session of match 0x[0-9a-f]+ is over", 0, mark, "the viewer ended by itself")
    a.key("space")
    a.expect_server(r"Pause\(true\)", 5, mark)
    paused_at = time.time()
    say("paused")
    while time.time() - paused_at < PAUSED_MOST - 60:
        time.sleep(30)
        say(f"paused for {time.time() - paused_at:.0f} s")
        absent(run, r"paused too long", 0, mark, "ended before the 5 minutes")
    a.key("left")  # a seek restarts the count
    sought = time.time()
    a.expect_server(r"Seek", 8, mark)
    say("sought: the count starts again")
    while time.time() - sought < PAUSED_MOST - 15:  # the last look comes before the limit, not on it
        time.sleep(min(30, PAUSED_MOST - 15 - (time.time() - sought)))
        say(f"{time.time() - sought:.0f} s since the seek")
        absent(run, r"paused too long", 0, mark, "the seek did not restart the count")
    ended = a.expect_server(r"paused too long, its session goes to someone else", 45, mark)
    say(f"at {time.time() - sought:.0f} s since the seek: {ended}")
    a.expect_server(r"session of match 0x[0-9a-f]+ is over", 30, mark)
    a.wait_screen(watch_open, "the Watch window, after the paused replay ended", 30, steady=1.5)
    a.shot("paused-back-in-watch")
    a.click(*CLOSE_WATCH, 6)
    a.wait_screen(lobby_clear, "the lobby", 5)


def open_friend_menu(a: Game) -> None:
    settle_focus(a)
    a.click(*FRIENDS, 8)
    time.sleep(1.5)
    a.right_click(*FRIEND_ROW, 8)
    time.sleep(0.5)


def friend_in_countdown(run: Run) -> None:
    """A friend's Spectate pressed in the countdown waits for play to begin, then watches."""
    a, b, c = sign_in(run, "ReplayTester"), sign_in(run, "SecondTester"), sign_in(run, "ThirdTester")
    mark = run.server.mark()
    room_match(b, c)
    if re.search(r"play begins", run.server.since(mark)):
        raise Again("play began before the menu could be opened")
    open_friend_menu(a)
    a.click(*FRIEND_SPECTATE, 6)
    say("pressed Spectate in the countdown")
    a.expect_server(r"play begins", 40, mark)
    begun = run.server.mark()
    a.expect_server(r"ReplayTester asked to watch the replay of match 0x[0-9a-f]+", 20, mark)
    if re.search(r"ReplayTester asked to watch", run.server.since(mark)[:run.server.since(mark).find("play begins")]):
        raise a.fail("the watch was asked before play began")
    a.expect_server(r"spectator 0x[0-9a-f]+ watches", 45, begun)
    time.sleep(2)
    a.shot("friend-countdown-watching")
    say("watching the friend after the countdown")
    leave_match(a)
    a.wait_screen(lambda picture: window_open(picture) or play_ready(picture), "the lobby, after following", 25, steady=1.5)
    end_match(run, b, c, begun=True)


def hold(game: Game, x: int, y: int, seconds: float) -> None:
    """The left button held on (x, y) for `seconds`, then released there."""
    game.move(x, y, 10)
    game.focus()
    game._aim_windows_cursor(x, y)
    time.sleep(0.15)
    send(mouse(flags=LEFT_DOWN))
    time.sleep(seconds)
    send(mouse(flags=LEFT_UP))


def wheel(game: Game, notches: int) -> None:
    """The mouse wheel, `notches` down."""
    event = mouse(flags=0x0800)
    event.u.mi.mouseData = (-120 * notches) & 0xFFFFFFFF
    game.focus()
    send(event)


def windows_close_watch(run: Run) -> None:
    """Any lobby window opening closes the Watch window."""
    a = sign_in(run, "ReplayTester")
    a.wait_screen(play_ready, "a lobby that has settled", 20, steady=3)
    for name, place in TOP_MENU.items():
        a.click(*WATCH_MENU)
        a.wait_screen(watch_open, f"the Watch window, before {name}", 6)
        a.click(*place, 8)
        time.sleep(2)
        a.shot(f"windows-{name}")
        if watch_open(a.picture()):
            raise a.fail(f"the Watch window stayed open over {name}")
        say(f"{name} closed the Watch window")
        a.click(*CLOSE_WATCH, 30)
        time.sleep(1)


def card_press(run: Run) -> None:
    """The live badge; a card keeps a press held through the Live list's refreshes (every 5 s); the Replays list keeps its
    scroll through its refresh."""
    a, b, c = sign_in(run, "ReplayTester"), sign_in(run, "SecondTester"), sign_in(run, "ThirdTester")
    a.wait_screen(play_ready, "a lobby that has settled", 20, steady=2)
    if badge_shown(a.picture()):
        raise a.fail("the live badge shows with no match running")
    open_replays(a)
    a.click(*REPLAYS_EVERYONE)
    a.wait_screen(card_listed, "a card under Everyone")
    time.sleep(1)
    a.move(800, 700, 20)
    wheel(a, 3)
    time.sleep(1)

    def rows(picture):
        return picture[240:1000, 110:1300].astype(int)
    before = rows(a.picture())
    a.shot("press-1-scrolled")
    time.sleep(33)  # past the Replays list's refresh of 30 s
    change = abs(rows(a.picture()) - before).mean()
    a.shot("press-2-after-refresh")
    if change > 6:
        raise a.fail(f"the list moved through its refresh ({change:.1f})")
    say(f"the list kept its scroll through a refresh ({change:.1f})")
    a.click(*CLOSE_WATCH)
    a.wait_screen(lobby_clear, "the lobby", 5)
    room_match(b, c)
    a.expect_server(r"play begins", 40)
    a.wait_screen(badge_shown, "the live badge", 40)
    a.shot("press-3-badge")
    say("the live badge shows with a match running")
    a.click(*WATCH_MENU)
    a.wait_screen(watch_open, "the Watch window")
    a.click(*TAB_LIVE)
    a.wait_screen(card_listed, "a card on the Live tab")
    mark = run.server.mark()
    hold(a, CARD_WATCH[0], CARD_WATCH[1], 12)  # through two refreshes of the list
    a.expect_server(r"spectator 0x[0-9a-f]+ watches", 40, mark)
    say("a press held through the list's refreshes still started the watch")
    leave_match(a)
    a.wait_screen(watch_open, "the Watch window, after watching", 25, steady=1.5)
    a.click(*CLOSE_WATCH)
    a.wait_screen(lobby_clear, "the lobby", 5)
    end_match(run, b, c, begun=True)


def get_clipboard() -> str:
    return subprocess.run(["powershell", "-NoProfile", "-Command", "Get-Clipboard -Raw"], capture_output=True, text=True, check=False).stdout.strip()


def set_clipboard(text: str) -> None:
    for attempt in range(6):  # another program can hold the clipboard for a moment
        done = subprocess.run(["powershell", "-NoProfile", "-Command", "Set-Clipboard -Value ([Console]::In.ReadToEnd())"], input=text, text=True,
                              capture_output=True)
        if done.returncode == 0:
            return
        time.sleep(0.5)
    raise RuntimeError(f"the clipboard could not be set: {done.stderr.strip()[:200]}")


def card_code(run: Run) -> None:
    """Pressing a card's code selects it: Ctrl+C copies it, typing changes nothing."""
    a = sign_in(run, "ReplayTester")
    a.wait_screen(play_ready, "a lobby that has settled", 20, steady=3)
    open_replays(a)
    a.wait_screen(card_listed, "a card on the Replays tab")
    time.sleep(1)
    saved = get_clipboard()
    try:
        set_clipboard("nothing-yet")
        a.click(*CARD_CODE, 10)
        time.sleep(0.5)
        a.shot("code-1-selected")
        a.type("zz")
        time.sleep(0.5)
        a.click(*CARD_CODE, 10)
        time.sleep(0.4)
        a.key_chord("ctrl", "c")
        time.sleep(0.6)
        copied = get_clipboard()
        if not re.fullmatch(r"#?[0-9a-f]{8}", copied):  # after the typing above: had it changed the code, it would show
            raise a.fail(f"Ctrl+C did not copy the card's code: {copied!r}")
        say(f"copied {copied}")
    finally:
        set_clipboard(saved or " ")
    a.click(*CLOSE_WATCH)
    a.wait_screen(lobby_clear, "the lobby", 5)


def box_up(picture) -> bool:
    """A message box is up: the window behind it is dark, the middle of the screen is not."""
    return picture[260:480, 120:1400].mean() < 12 and picture[500:700, 685:915].mean() > 15


def message_box(game: Game, shot: str, seconds: float = 10) -> None:
    """A message box the game puts up (the lobby behind it goes dark), read in a shot and dismissed."""
    dark = box_up
    game.wait_screen(dark, f"a message box ({shot})", seconds, steady=0.3)
    game.shot(shot)
    game.key("enter")
    game.wait_screen(lambda picture: not dark(picture), "the message box closed", 5)


def type_in_search(game: Game, text: str) -> None:
    """`text` alone in the Watch window's search box, which keeps what was typed last."""
    game.click(*SEARCH_BOX, 10)
    time.sleep(0.2)
    for _ in range(20):
        game.key("backspace")
    game.type(text)


def code_submit(run: Run) -> None:
    """A code with no recording, sent by the magnifier and by Enter: either way a box stays up to say it was refused."""
    a = sign_in(run, "ThirdTester")
    a.wait_screen(play_ready, "a lobby that has settled", 20, steady=3)
    open_replays(a)
    a.wait_screen(card_listed, "a card on the Replays tab")
    gone = []
    for number, how in enumerate(("magnifier", "enter"), 1):
        code = f"deadbf{number:02x}"
        type_in_search(a, code)
        mark = run.server.mark()
        started = time.monotonic()
        if how == "enter":
            a.key("enter")
        else:
            a.click(*MAGNIFIER)
        seen = []
        while time.monotonic() - started < 6:
            seen.append(f"{time.monotonic() - started:.1f}:{'box' if box_up(a.picture()) else '-'}")
            time.sleep(0.2)
        a.expect_server(rf"ThirdTester asked to watch the replay of match 0x{code}", 2, mark)
        say(f"{how}: {' '.join(seen)}")
        a.shot(f"code-submit-{how}")
        if box_up(a.picture()):
            a.key("enter")
            a.wait_screen(lambda picture: not box_up(picture), "the message box closed", 5)
        else:
            gone.append(how)
        time.sleep(0.5)
    if gone:
        raise a.fail(f"no box left after a refused code sent by: {', '.join(gone)}")
    saved = get_clipboard()  # a real code, sent by Enter, is watched
    try:
        a.click(*CARD_CODE, 10)
        time.sleep(0.4)
        a.key_chord("ctrl", "c")
        time.sleep(0.6)
        code = get_clipboard().lstrip("#")
    finally:
        set_clipboard(saved or " ")
    if not re.fullmatch(r"[0-9a-f]{8}", code):
        raise a.fail(f"no code copied from the card: {code!r}")
    type_in_search(a, code)
    mark = run.server.mark()
    a.key("enter")
    a.expect_server(r"spectator 0x[0-9a-f]+ watches", 25, mark)
    say(f"the real code {code} sent by Enter is watched")


def unknown_codes(run: Run) -> None:
    """A code with no recording is refused and does not bring the Watch window back later; past 12 unknown ids in an hour a
    real card is refused too. Leaves the account limited in the server's memory: run it last."""
    a, b = sign_in(run, "ThirdTester"), sign_in(run, "SecondTester")
    a.wait_screen(play_ready, "a lobby that has settled", 20, steady=3)
    open_replays(a)
    a.wait_screen(card_listed, "a card on the Replays tab")
    for number in range(1, 13):
        code = f"deadbe{number:02x}"
        type_in_search(a, code)
        mark = run.server.mark()
        a.key("enter")
        a.expect_server(rf"ThirdTester asked to watch the replay of match 0x{code}", 6, mark)
        message_box(a, f"unknown-{number:02d}")
        say(f"unknown code {number}")
    mark = run.server.mark()
    a.click(*TAB_REPLAYS)
    time.sleep(1.5)
    a.wait_screen(card_listed, "a card on the Replays tab")
    a.click(*CARD_WATCH, 8)
    message_box(a, "unknown-13-real-card")
    absent(run, r"ThirdTester watches the replay", 3, mark, "a limited account watched a replay")
    say("past the limit a real card is refused too")
    a.click(*CLOSE_WATCH, 8)
    a.wait_screen(lobby_clear, "the lobby", 8)
    room_match(a, b)
    end_match(run, a, b)
    after_results(a)
    time.sleep(2)
    a.shot("unknown-no-return")
    if watch_open(a.picture()):
        raise a.fail("the Watch window came back after a refused ask")
    say("the refused ask did not bring the Watch window back")


def moments(run: Run) -> None:
    """A replay with events: the Moments choices act on the press, and a tick on the bar has a tooltip. The events come from
    SRP_TEST_SEED_EVENTS, a command that writes them into the test database. With SRP_TEST_VIEW_LANGUAGE the viewing game, and so the tooltip, is in that language."""
    a, b = sign_in(run, "ReplayTester"), sign_in(run, "SecondTester")
    if room_match(a, b) == BLACKOUT:
        end_match(run, a, b)
        raise Again("the room drew Blackout")
    a.expect_server(r"play begins", 40)
    time.sleep(30)  # a stretch of play for the seeded events to lie in
    end_match(run, a, b, begun=True)
    after_results(a)
    seed = os.environ.get("SRP_TEST_SEED_EVENTS")  # puts the events into the test database: two idle games score none
    if seed:
        time.sleep(1)
        done = subprocess.run(seed, shell=True, capture_output=True, text=True, check=False)
        say(f"seeded: {done.stdout.strip()[:100]} {done.stderr.strip()[:100]}")
    language = os.environ.get("SRP_TEST_VIEW_LANGUAGE", "english")
    if language != "english":  # the match is played in English, whose buttons the flows know: only the viewer's game changes
        a.close()
        os.environ["SRP_TEST_LANGUAGE"] = language
        time.sleep(3)
        a = sign_in(run, "ReplayTester")
        a.wait_screen(play_ready, "a lobby that has settled", 20, steady=3)
    open_replays(a)
    a.wait_screen(card_listed, "a card on the Replays tab")
    mark = run.server.mark()
    a.click(*CARD_WATCH, 8)
    a.expect_server(r"ReplayTester asked to watch the replay of match 0x[0-9a-f]+ \(ReplayWatchAsk \{ seconds: 0, place: Some\(0\)", 6, mark)  # its own seat
    a.expect_server(r"spectator 0x[0-9a-f]+ watches", 20, mark)
    time.sleep(2)
    a.cursor_on()
    time.sleep(0.5)
    a.shot("moments-1-viewer")

    def word(picture, name):
        x, y = MOMENTS[name]
        return picture[y - 9:y + 9, x - 22:x + 22].astype(int).mean()
    a.click(*MOMENTS["all"], 8)
    time.sleep(0.6)
    all_lit = word(a.picture(), "all")
    a.click(*MOMENTS["mine"], 8)
    time.sleep(0.6)
    picture = a.picture()
    a.shot("moments-2-mine")
    if not word(picture, "mine") > word(picture, "all"):
        raise a.fail("the Moments choice did not move to Mine")
    a.click(*MOMENTS["all"], 8)
    time.sleep(0.6)
    if abs(word(a.picture(), "all") - all_lit) >= 12:
        raise a.fail("the Moments choice did not return to All")
    say("the Moments choices act on the press")
    a.move(800, 900, 30)
    time.sleep(0.4)

    def tip(picture):
        return picture[1085:1140, 560:1040].astype(int)
    base = tip(a.picture())
    for x in range(SEEK_BAR_X[0], SEEK_BAR_X[1], 6):
        a._place(x, SEEK_BAR_Y)
        time.sleep(0.12)
        if abs(tip(a.picture()) - base).mean() > 3:
            a.shot("moments-3-tooltip")
            say(f"a tooltip at x={x}")
            break
    else:
        a.shot("moments-3-no-tooltip")
        raise a.fail("no tick on the bar showed a tooltip")
    a.click(*BACK, 10)
    a.expect_server(r": Leave at \d+ ms", 6, mark)
    a.expect_server(r"session of match 0x[0-9a-f]+ is over", 20, mark)
    a.wait_screen(window_open, "the Watch window, after the replay", 20, steady=1.5)
    a.click(*CLOSE_WATCH, 6)
    a.wait_screen(lobby_clear, "the lobby", 5)


def loose_search(run: Run) -> None:
    """A player's name typed with a letter missing finds the player: its matches on the Replays tab."""
    a = sign_in(run, "ReplayTester")
    a.wait_screen(play_ready, "a lobby that has settled", 20, steady=3)
    open_replays(a)
    a.wait_screen(card_listed, "a card on the Replays tab")
    type_in_search(a, "secnd")
    mark = run.server.mark()
    a.key("enter")
    a.expect_server(r"ReplayTester searched players for `secnd`: \d+ found, the best SecondTester", 6, mark)
    a.wait_screen(card_listed, "the matches of the player found", 10)
    a.shot("loose-search-found")
    say("a name typed loosely found the player's matches")
    a.click(*CLOSE_WATCH)
    a.wait_screen(lobby_clear, "the lobby", 5)


def called_back_line(picture) -> bool:
    """The hud's line of a room's call back: a dark box with yellow words, under the game's own lines at the top of the
    screen, in the middle (CalledBackNotice.TOP, at the 1600x1200 window.s scale)."""
    box = picture[118:172, 640:960].astype(int)
    yellow = (box[:, :, 0] > 200) & (box[:, :, 1] > 160) & (box[:, :, 2] < 120)
    return bool(yellow.mean() > 0.01)


def room_notice(picture) -> bool:
    """The room's "Your room is ready": a dark box with a gold edge and light words, under the window's title, in the middle."""
    box = picture[150:215, 600:1000].astype(int)
    gold = (box[:, :, 0] > 170) & (box[:, :, 1] > 120) & (box[:, :, 1] < 200) & (box[:, :, 2] < 120)
    return bool(gold.sum() >= 60)


def called_back(run: Run) -> None:
    """A player alone in its room watches a replay; a second player comes, the room can play, and the first is
    brought back from the replay to its room, which shows itself; the room's match is then made with both."""
    a, b = sign_in(run, "ReplayTester"), sign_in(run, "SecondTester")
    mark = run.server.mark()
    a.click(*PLAY, 6)
    a.expect_server(r"made room \d+", 20, mark)
    a.wait_screen(window_open, "the room")
    open_replays(a)
    a.wait_screen(card_listed, "a card on the Replays tab")
    watch_card(a)
    a.expect_server(r"Replay 0x[0-9a-f]+: account \d+ at \S+ watches match", 40, mark)
    a.wait_screen(viewer_kits_shown, "the replay's viewer", 30)
    a.shot("called-back-1-watching")
    say("alone in its room, watching a replay")
    b.click(*PLAY, 6)
    a.expect_server(r"ReplayTester: called back to room \d+ from the replay it watched", 20, mark)
    a.expect_server(r"its player's room calls it back", 10, mark)
    a.wait_screen(called_back_line, "the line that says so over the replay, with its seconds", 4)
    a.shot("called-back-1b-countdown")
    a.expect_server(r"let go, back to its player's room", 10, mark)
    a.wait_screen(lambda picture: window_open(picture) and not watch_open(picture) and room_notice(picture),
                  "its room, not the Watch window, with its notice, back from the replay", 40, steady=0.5)
    a.shot("called-back-2-room")
    say("brought back to its room")
    made = a.expect_server(r"Match 0x[0-9a-f]+ made: map \d+ \([^,]+, mode \d+\)", 120, mark)
    say(made)
    a.expect_server(r"ReplayTester goes to match", 15, mark)
    a.expect_server(r"SecondTester goes to match", 15, mark)
    end_match(run, a, b)


def ranking_listed(picture) -> bool:
    """The Ranking window with rows: light text where its second row's name is."""
    name = picture[484:500, 596:690].astype(int).sum(axis=2)
    return bool((name > 450).mean() > 0.05)


def ranking_menu_open(picture) -> bool:
    """A right-click menu under the Ranking's second row: near black, with its entries' light words."""
    menu = picture[515:660, 645:805].astype(int).sum(axis=2)
    return bool((menu < 60).mean() > 0.7 and (menu > 400).mean() > 0.01)


def ranking_matches(run: Run) -> None:
    """A Ranking row's right-click menu has Matches: the Watch window on Replays, that player's name searched."""
    a = sign_in(run, "ReplayTester")
    a.wait_screen(play_ready, "a lobby that has settled", 20, steady=3)
    a.click(*RANKING_MENU)
    a.wait_screen(ranking_listed, "the Ranking with its rows", 8, steady=0.5)
    a.shot("ranking-1-window")
    a.right_click(*RANKING_ROW, 10)
    a.wait_screen(ranking_menu_open, "the row's right-click menu", 3)
    a.shot("ranking-2-menu")
    a.click(*RANKING_MATCHES, 10)
    a.wait_screen(window_open, "the Watch window, from the Ranking's menu", 6)
    a.wait_screen(card_listed, "the player's matches on the Replays tab", 10)
    a.shot("ranking-3-matches")
    say("Matches of the Ranking's menu opened the Watch window on the player's matches")
    a.click(*CLOSE_WATCH)
    a.wait_screen(lambda picture: not window_open(picture), "the Watch window closed", 5)


FLOWS = {"called_back": called_back, "ranking_matches": ranking_matches, "loose_search": loose_search, "queue_spectate": queue_spectate, "paused": paused,
         "friend_in_countdown": friend_in_countdown,
         "windows_close_watch": windows_close_watch, "card_press": card_press, "card_code": card_code, "code_submit": code_submit,
         "unknown_codes": unknown_codes, "moments": moments, "kit": kit_after_replay, "kit_view": kit_view, "live": live, "follow": follow, "save": save, "first_click": first_click}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("flows", nargs="+", choices=sorted(FLOWS))
    parser.add_argument("--game", required=True)
    parser.add_argument("--server-log", required=True)
    parser.add_argument("--host", required=True)
    parser.add_argument("--shots", required=True)
    args = parser.parse_args()
    failed = False
    for name in args.flows:
        for attempt in range(1, 5):
            say(f"=== {name}, attempt {attempt}")
            with Run(args.game, args.server_log, args.host, args.shots) as run:
                try:
                    FLOWS[name](run)
                    say(f"=== {name}: PASS")
                    break
                except Again as again:
                    say(f"starting over: {again}")
                except Stop as stop:
                    say(f"=== {name}: FAILED: {stop}")
                    failed = True
                    break
        else:
            say(f"=== {name}: FAILED: drew nothing testable in 4 attempts")
            failed = True
        if failed:  # what a failed flow leaves on the server is the next one's trouble
            break
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
