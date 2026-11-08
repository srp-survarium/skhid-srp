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
CHIP_FRIENDS = (240, 195)
CHIP_EVERYONE = (340, 195)
CLOSE_WATCH = (1490, 137)
JOIN_ROOM = (917, 1134)
CARD_WATCH = (1396, 268)  # the first card's Watch button
CARD_STEP = 82  # the height of a card
BACK = (288, 28)
PROFILE_2 = (794, 1129)
BLACKOUT = 6  # the mode without a HUD, and so without the kit bar
LEAVE_MATCH = (799, 593)
# A room's three map cards: where to sample each mode label's box, and where to click to vote for the card.
CARD_LABELS = (132, 598, 1064)
CARD_LABEL_Y = 337
CARD_VOTES = ((338, 440), (804, 440), (1270, 440))

began = time.time()


class Again(Exception):
    """The run drew something it cannot test: start over."""


def window_open(picture) -> bool:
    """A lobby window (Watch, Rooms) is up: its dark panel covers the room's bright floor."""
    return picture[1000:1100, 130:600].mean() < 45


def lobby_clear(picture) -> bool:
    return not window_open(picture)


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
    game.click(*LEAVE_MATCH)
    game.key("enter")


def end_match(run: Run, a: Game, b: Game) -> None:
    """Both leave once play has begun, and the server has closed the match: the next run finds nothing left of it."""
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
    """A room outlives a killed game, and the server tells the account about it a few seconds after it signs in."""
    end = time.time() + 3.5
    while time.time() < end:
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
    host.click(*PLAY, 6)
    host.expect_server(r"made room \d+", 20, mark)
    host.wait_screen(window_open, "the room")
    vote_away_from_blackout(host)
    guest.click(*PLAY, 6)
    guest.wait_screen(window_open, "the Rooms list")
    guest.click(*JOIN_ROOM, 8)
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
    a.wait_screen(window_open, "the Watch window again, after the replay")
    a.shot("kit-0-back-in-watch")
    a.click(*CLOSE_WATCH)
    a.wait_screen(lobby_clear, "the lobby, with the Watch window closed", 5)
    say("the Watch window came back and closed")
    if room_match(a, b) == BLACKOUT:
        end_match(run, a, b)
        raise Again("the room drew Blackout, which has no kit bar")
    a.shot("kit-1-next-match-before")
    a.click(*PROFILE_2, 8)
    a.shot("kit-2-after-click")
    say("clicked the second kit")
    end_match(run, a, b)


def live(run: Run) -> None:
    """A match in progress, as seen from the lobby: the badge, the Live list, and watching it."""
    a, b, c = sign_in(run, "ReplayTester"), sign_in(run, "SecondTester"), sign_in(run, "ThirdTester")
    room_match(b, c)
    a.expect_server(r"play begins", 40)
    a.focus()
    a.shot("live-1-lobby-badge")
    a.click(*WATCH_MENU)
    a.click(*TAB_LIVE)
    a.shot("live-2-list")
    say("Live list open")
    watch_card(a)
    a.shot("live-3-watching")
    mark = run.server.mark()
    a.key("esc")
    a.expect_server(r"game left|Leave at", 10, mark)
    a.shot("live-4-back-in-window")
    say("left the live match")


FLOWS = {"kit": kit_after_replay, "live": live}


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
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
