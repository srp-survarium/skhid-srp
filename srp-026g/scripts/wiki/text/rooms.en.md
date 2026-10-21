# Rooms
section: mechanics
summary: How matches are made: rooms, votes, late joins.

Every PvP match is a room's. Play vs Bots and clan matches stay as they were.

## Making one

- Team size {{rooms.min}} v {{rooms.min}} to {{rooms.max}} v {{rooms.max}}; players to start: {{rooms.start_min}} v {{rooms.start_min}} to {{rooms.start_max}} v {{rooms.start_max}}.
- Max level: an equipment cap (none, or up to {{rooms.level_max}}), never below the maker's own gear.
- A wait of {{rooms.wait_min}} to {{rooms.wait_max}} before a match, an optional 4-digit PIN, a pool of maps and [modes](modes).

## Play

- Play puts you straight into the open room nearest your level, then the fullest; none: a new room.
- You stay in your room from match to match until you leave, are kicked (PIN rooms only) or the room closes.
- Lost your game: your place is kept about 90 s; Play takes you back to the same seat.

## Before each match

- Once enough players are in: a vote on 3 cards (map, weather, mode), open until the match starts. Most votes wins; a tie is drawn.
- A kit that can't play (no weapon, too little ammo) or gear over the max level: you are marked red and left out.
- Sides are drawn as even as can be, squads together.

## Late joins

- Coming during a match: you see the room first, then go in 15 s later, or at once with **Join match**.
- You join the side with fewer players; then the side you played on before; then the weaker one.
- No late joins in the last 30 s, while another player joins, while a game is loading, into a full match, near a maintenance or as the match ends: the room says why.
- A late joiner starts with its first [kit](kits).
- Leaving a match and coming back is a fresh start.

## Switch and queue

- **Switch room**: the open room of the lowest max level lower than yours that takes your gear.
- A full room offers **Queue**: you go in when a place frees.
