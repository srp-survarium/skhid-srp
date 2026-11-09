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
import sys
import time

from driver import Game, Run, Stop

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
PROFILE_2 = (794, 1129)
BLACKOUT = 6  # the mode without a HUD, and so without the kit bar
FRIENDS = (108, 1183)
FRIEND_ROW = (110, 876)  # the first friend in the friends panel
FRIEND_SPECTATE = (150, 907)  # its right-click menu's Spectate, for a friend in a match
RESULTS_SAVE = (704, 881)  # "Save replay"; the floppy at the right edge is the stock screen's own, disabled
RESULTS_CLOSE = (812, 881)
LEAVE_MATCH = (799, 593)
VIEWER_PLATES, VIEWER_PLATE_Y = (1160, 1314, 1470), 972  # the kit plates in a replay's viewer
PLAYER_ROW_X = 1495  # a viewer's player list
PLAYER_ROWS = (("ReplayTester", 560), ("SecondTester", 583))
LIVE_DELAY = 30  # SRP_LIVE_WATCH_DELAY_S's default
DAILY_CLOSE = (1032, 539)  # the X of the first sign-in of a day's reward box
# A room's three map cards: where to sample each mode label's box, and where to click to vote for the card.
CARD_LABELS = (132, 598, 1064)
CARD_LABEL_Y = 337
CARD_VOTES = ((338, 440), (804, 440), (1270, 440))

began = time.time()


class Again(Exception):
    """The run drew something it cannot test: start over."""


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
    band is darker than a window's and its footer nearly black."""
    title, footer = picture[530:548, 556:680], picture[652:670, 552:572]
    return bool(8 < title.mean() < 19 and title.std() < 6 and 1 < footer.mean() < 5 and footer.std() < 2)


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


def play_ready(picture) -> bool:
    """The lobby's PLAY button, lit: a match's screen has no such red there."""
    button = picture[15:45, 735:865].astype(int)
    return ((button[:, :, 0] > 150) & (button[:, :, 1] < 90)).mean() > 0.4


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


FLOWS = {"kit": kit_after_replay, "kit_view": kit_view, "live": live, "follow": follow, "save": save, "first_click": first_click}


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
