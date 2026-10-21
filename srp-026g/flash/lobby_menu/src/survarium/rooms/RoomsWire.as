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
        // kept by the server (the owner's of 2026-10-20).
        public static const SETTINGS:int = FIRST + 9;
        // The room's chat tab is up: its last lines, and its new ones from
        // now on (answered with the state; social.md 9.8).
        public static const CHAT:int = FIRST + 30;
        // The asker's friends who are members of a room (`S1;Y`).
        public static const FRIEND_ROOMS:int = FIRST + 40;
        public static const QUEUE:int = FIRST + 200000;
        public static const VOTE:int = FIRST + 3;
        public static const POOL:int = FIRST + 10;
        public static const KICK:int = FIRST + 100;
        public static const PIN:int = FIRST + 10000;
        public static const JOIN:int = FIRST + 100000;
        // Who is in a room, for anyone (`S1;G`; a room with a PIN too).
        public static const MEMBERS:int = FIRST + 300000;
        public static const BITS:int = FIRST + 1000000;
        public static const CREATE:int = FIRST + 10000000;
        public static const LAST:int = FIRST + 19999999;
        public static const WAITS:Array = [15, 20, 25, 30, 35, 40, 45, 50, 55, 60];
        // The vote's own time before the wait (srp's rooms::VOTE).
        public static const VOTE_SECS:int = 15;

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
