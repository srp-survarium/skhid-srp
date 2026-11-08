package survarium.watch
{
   import survarium.controls.NotificationListScrollContent;

   // The lobby server's rows for the Watch window (WatchWindow), parsed: each a `91` row's name,
   // `;` between its fields (docs/notes/replay_watch.md). A row that is not one gives null.
   // - Live: `L0;<n>[;<delay s>]`, then n `L1` rows, then `F1` rows;
   //   `L1;<match 8 hex>;<map id>;<mode>;<phase>;<players a>;<players b>;<score a|->;<score b|->;<played s>;<spectators>;<watch>`
   //   (phase 0 made, 1 countdown, 2 in play, 3 over; watch 1 may, 0 full, 2 not watchable live);
   //   the roster: `L2;<match 8 hex>;<place>;<team 0|1>;<kills>;<deaths>;<name>`.
   // - Recent: `M0;<n>`, then n `M1` rows, then `F1` rows;
   //   `M1;<match 8 hex>;<map id>;<mode>;<score a|->;<score b|->;<players>;<length s>;<minutes since its end>;<replay>`.
   // - A player's matches: `P1;<n>;<nickname>`, then n of
   //   `P2;<match 8 hex>;<place>;<map id>;<mode>;<result 0 defeat|1 victory|2 draw>;<kills>;<deaths>;<minutes since its end>;<replay>`.
   // - A friend of the asker in a listed match: `F1;<match 8 hex>;<place>;<team 0|1>;<name>`.
   // `<replay>`: REPLAY_* below.
   public class WatchRows
   {
      public static const REPLAY_NOT_RECORDED:int = 0;

      // kept: made into a replay when first asked for
      public static const REPLAY_KEPT:int = 1;

      public static const REPLAY_READY:int = 2;

      public static const REPLAY_GONE:int = 3;

      // recorded by games of revisions the asker's does not play
      public static const REPLAY_OLD:int = 4;

      public static const PHASE_PLAYING:int = 2;

      public static const PHASE_OVER:int = 3;

      public static const WATCH_FULL:int = 0;

      public static const WATCH_MAY:int = 1;

      public static const WATCH_PRIVATE:int = 2;

      public function WatchRows()
      {
         super();
      }

      private static function hex(text:String) : Boolean
      {
         return /^[0-9a-f]{8}$/.test(text);
      }

      // A count of the rows (0 to 999999), -1 if the text is not one.
      public static function count(text:String) : int
      {
         return /^[0-9]{1,6}$/.test(text) ? int(text) : -1;
      }

      private static function counts(f:Array, at:Array) : Boolean
      {
         for each(var i:int in at)
         {
            if(count(f[i]) < 0)
            {
               return false;
            }
         }
         return true;
      }

      // A score's two fields, "a:b", "" for "-;-"; null if they are neither.
      private static function score(a:String, b:String) : String
      {
         if(a == "-" && b == "-")
         {
            return "";
         }
         return count(a) < 0 || count(b) < 0 ? null : a + " : " + b;
      }

      // The head of a page: [its tag, n, the rest] ("L0;3;30" -> ["L0", 3, ["30"]]), null if not one.
      public static function head(text:String, tag:String) : Array
      {
         var f:Array = text.split(";");
         if(f[0] != tag || f.length < 2 || count(f[1]) < 0)
         {
            return null;
         }
         return [tag,count(f[1]),f.slice(2)];
      }

      public static function live(text:String) : Object
      {
         var f:Array = text.split(";");
         if(f.length != 12 || f[0] != "L1" || !hex(f[1]) || !counts(f,[2,3,4,5,6,9,10,11]))
         {
            return null;
         }
         var points:String = score(f[7],f[8]);
         if(points == null || count(f[11]) > WATCH_PRIVATE || count(f[4]) > PHASE_OVER)
         {
            return null;
         }
         return {
            "kind":"live",
            "match":uint(parseInt(f[1],16)),
            "map":count(f[2]),
            "mode":count(f[3]),
            "phase":count(f[4]),
            "a":count(f[5]),
            "b":count(f[6]),
            "score":points,
            "played":count(f[9]),
            "spectators":count(f[10]),
            "watch":count(f[11]),
            "friends":[]
         };
      }

      public static function recent(text:String) : Object
      {
         var f:Array = text.split(";");
         if(f.length != 10 || f[0] != "M1" || !hex(f[1]) || !counts(f,[2,3,6,7,8,9]))
         {
            return null;
         }
         var points:String = score(f[4],f[5]);
         if(points == null || count(f[9]) > REPLAY_OLD)
         {
            return null;
         }
         return {
            "kind":"recent",
            "match":uint(parseInt(f[1],16)),
            "map":count(f[2]),
            "mode":count(f[3]),
            "score":points,
            "players":count(f[6]),
            "length":count(f[7]),
            "minutes":count(f[8]),
            "replay":count(f[9]),
            "friends":[]
         };
      }

      public static function played(text:String) : Object
      {
         var f:Array = text.split(";");
         if(f.length != 10 || f[0] != "P2" || !hex(f[1]) || !counts(f,[2,3,4,5,6,7,8,9]))
         {
            return null;
         }
         if(count(f[2]) > 0xFE || count(f[5]) > 2 || count(f[9]) > REPLAY_OLD)
         {
            return null;
         }
         return {
            "kind":"played",
            "match":uint(parseInt(f[1],16)),
            "place":count(f[2]),
            "map":count(f[3]),
            "mode":count(f[4]),
            "result":count(f[5]),
            "kills":count(f[6]),
            "deaths":count(f[7]),
            "minutes":count(f[8]),
            "replay":count(f[9]),
            "friends":[]
         };
      }

      // An `L2` (roster) or `F1` (friend) row: {match, place, team, kills, deaths, name}.
      public static function player(text:String, tag:String) : Object
      {
         var f:Array = text.split(";");
         var named:int = tag == "L2" ? 6 : 4;
         if(f.length <= named || f[0] != tag || !hex(f[1]) || !counts(f,tag == "L2" ? [2,3,4,5] : [2,3]))
         {
            return null;
         }
         if(count(f[2]) > 0xFE || count(f[3]) > 1)
         {
            return null;
         }
         return {
            "match":uint(parseInt(f[1],16)),
            "place":count(f[2]),
            "team":count(f[3]),
            "kills":tag == "L2" ? count(f[4]) : 0,
            "deaths":tag == "L2" ? count(f[5]) : 0,
            "name":f.slice(named).join(";")
         };
      }

      // A replay session from `place`'s view, from the start (REPLAY_WATCH_ALL's layout).
      public static function watchAsk(place:int) : uint
      {
         return (NotificationListScrollContent.REPLAY_WATCH_ALL & 0xFF000000) | uint(place) << 16;
      }
   }
}
