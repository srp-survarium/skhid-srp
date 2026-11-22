package survarium.rooms
{
    import flash.external.ExternalInterface;

    // The rooms' wire (srp's own; lobby.md 2.16.3, contract S1). A request
    // is the game's ranking request (`request_players_range`) at a start in
    // the rooms' range; the server answers every one with ranking rows
    // (`91`) at the start asked, which PlayerRanking.fillPlayers hands to the
    // window first (RoomsWindow.take). A row's nickname carries the answer:
    // `S1;<kind>;...`, its fields split at ";".
    public class RoomsWire
    {
        public static const FIRST:int = 2060000000;
        public static const LIST:int = FIRST;
        public static const STATE:int = FIRST + 1;
        public static const LEAVE:int = FIRST + 2;
        public static const PLAY:int = FIRST + 6;
        public static const UNQUEUE:int = FIRST + 7;
        public static const JOIN_MATCH:int = FIRST + 8;
        // The create window's start: the player's last room made by hand,
        // kept by the server (the owner's of 2026-10-20); asked once as the
        // lobby comes up (RoomsWindow._form).
        public static const SETTINGS:int = FIRST + 9;
        // The room's chat tab is up: its last lines, and its new ones from
        // now on (answered with the state; social.md 9.8).
        public static const CHAT:int = FIRST + 30;
        // The asker's friends who are members of a room (`S1;Y`).
        public static const FRIEND_ROOMS:int = FIRST + 40;
        // A private room's owner starts its match now (answered with the
        // state; the owner's of 2026-10-26: no countdown in a private room).
        public static const START:int = FIRST + 41;
        // The owner of a private room moves a member to the other team:
        // MOVE + place + MEMBERS_MAX * (the room's serial % KICK_CHECKS), as a
        // kick (answered with the state, whose `A;<sides>` row has the teams).
        public static const MOVE:int = FIRST + 20000;
        public static const QUEUE:int = FIRST + 200000;
        public static const VOTE:int = FIRST + 3;
        public static const POOL:int = FIRST + 10;
        public static const KICK:int = FIRST + 100;
        public static const PIN:int = FIRST + 10000;
        public static const JOIN:int = FIRST + 100000;
        // Who is in a room, for anyone (`S1;G`; a room with a PIN too).
        public static const MEMBERS:int = FIRST + 300000;
        // Whether a join offers a kit preset (`S1;O`; presets.md): + key, 0 for
        // the room Play would take.
        public static const OFFER:int = FIRST + 400000;
        // "Switch and enter": PRESET + (preset - 1) * PRESET_STEP + key (0: Play).
        public static const PRESET:int = FIRST + 500000;
        public static const PRESET_STEP:int = 100000;
        // "Queue and spectate": WATCH + key, a full room's queue as QUEUE's, its
        // match watched meanwhile (queue-spectate.md; the owner's of 2026-11-05).
        public static const WATCH:int = FIRST + 11000000;
        public static const LAST:int = FIRST + 19999999;
        // The rows asked of the list (the server's LIST_ROWS, 31 rooms, and
        // the header) and of a pool page (POOL_PAGE_ROWS, 30, and the header).
        public static const LIST_ASK_ROWS:int = 32;
        public static const POOL_ASK_ROWS:int = 31;
        // A room's most members (two sides of 10) and a max level of "any".
        public static const MEMBERS_MAX:int = 20;
        public static const LEVEL_ANY:int = 0;
        // The form's pool rows (`E;<chunk>;<bits>`) of BITS_PER_CHUNK entries; a create names
        // POOL_BITS entries at most. 256: every playable map (219 in the live data).
        public static const BITS_PER_CHUNK:int = 16;
        public static const POOL_BITS:int = 256;
        // A PIN's digits (PIN_MAX 9999).
        public static const PIN_DIGITS:int = 4;
        // The vote's cards: 0 to LAST_CARD (the server's CARDS, 3).
        public static const LAST_CARD:int = 2;
        // A kick: KICK + place + MEMBERS_MAX * (the room's serial % KICK_CHECKS).
        public static const KICK_CHECKS:int = 495;

        // A private room's create in one request (the server's RoomCreate; the owner's of
        // 2026-11-20: "a single very fast request"): `S1C;<P or B><PIN>;<pool>`, B with bots, the
        // pool entries `picked` (their indexes) in hex, character k entries 4k to 4k + 3, entry
        // 4k + b by bit b. Until then a PIN, a bits request a 16 entries and the create, one after
        // another on the game's 500 ms lobby tick: 4 to 8 s.
        public static function createText(pin:String, bots:Boolean, picked:Array):String
        {
            var nibbles:Array = [];
            for each (var index:int in picked)
            {
                if (index < 0 || index >= POOL_BITS)
                    continue;
                var k:int = index >> 2;
                while (nibbles.length <= k)
                    nibbles.push(0);
                nibbles[k] |= 1 << (index & 3);
            }
            var pool:String = "";
            for (var n:int = 0; n < nibbles.length; n++)
                pool += int(nibbles[n]).toString(16);
            return "S1C;" + (bots ? "B" : "P") + pin + ";" + pool;
        }

        // A create's text goes as the game's player search (`find_players`, sent as it is, the
        // lobby.md 2.16.3 create in one request): the server answers it with the state.
        public static function sendCreate(text:String):void
        {
            try
            {
                ExternalInterface.call("find_players", text);
            }
            catch (e:Error)
            {
            }
        }

        // Whether pool entry `index` is in by a form's `bits` (chunk n's bits at bits[n]).
        public static function bitIn(bits:Array, index:int):Boolean
        {
            return (uint(bits[int(index / BITS_PER_CHUNK)]) & (1 << (index % BITS_PER_CHUNK))) != 0;
        }

        public static function send(start:int, count:int):void
        {
            try
            {
                ExternalInterface.call("request_players_range", start, count);
            }
            catch (e:Error)
            {
            }
        }

        // Ranking rows of the rooms' range: the window's answers.
        public static function owns(rows:Array):Boolean
        {
            if (rows == null || rows.length == 0 || rows[0] == null)
                return false;
            var place:Number = Number(rows[0].place);
            return !(place < FIRST || place > LAST);
        }

        public static function fields(row:Object):Array
        {
            if (row == null || row.nickname == null)
                return [];
            return String(row.nickname).split(";");
        }
    }
}
