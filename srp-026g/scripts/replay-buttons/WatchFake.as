package
{
   // The lobby server's rows the Watch window takes (survarium.watch.WatchRows), made up for the
   // harness (ReplayButtonsHarness) and the screenshots (WatchShots): PlayerRanking's row objects.
   // Maps by their maps_dict ids, as the server sends them; POOL is the rooms' pool that names them
   // (RoomsWindow.entry: level and weather), 99 a map it no longer has.
   public class WatchFake
   {
      public static const MATCH:uint = 0x1CE45CBE;

      public static const OTHER:uint = 0x69F8C4BC;

      // the rooms' pool page (`S1;M;<n>;<page>`, then `<index>;<map>;<level>;<weather>;<mode>;<extra>`)
      public static const POOL:Array = ["S1;M;5;0","0;117;1;1;4;0","1;61;3;3;5;0","2;62;4;1;4;0","3;63;2;2;2;0","4;64;13;1;5;1"];

      public static const POOL_START:uint = 2060000010;

      // Live: 6 good matches, 30 s behind (Rudnya in play with the friends Petro (place 3) and Olha;
      // Vostok in play; Chemical plant counting down; one full, one closed to spectators, one over)
      // and a broken row.
      public static const LIVE:Array = ["L0;7;30",
         "L1;69f8c4bc;61;5;2;8;8;2;1;422;0;1",
         "L1;1ce45cbe;117;4;2;10;10;87;64;760;2;1",
         "L1;00000004;62;4;1;5;4;-;-;0;0;1",
         "L1;bad",
         "L1;00000005;63;2;2;10;10;1;0;610;8;0",
         "L1;00000006;64;5;2;4;4;0;1;300;0;2",
         "L1;00000007;117;4;3;6;6;100;80;1400;0;1",
         "F1;1ce45cbe;3;0;Petro",
         "F1;1ce45cbe;9;1;Olha_Kovalenko"];

      // Recent: each replay state once (0 not recorded .. 4 old version), the friend Petro in the
      // second (a recorded match: no place); a broken row
      public static const RECENT:Array = ["M0;5",
         "M1;00000011;117;4;100;87;20;720;12;1",
         "M1;69f8c4bc;61;5;3;1;16;1080;25;1",
         "M1;00000013;99;2;12;9;15;840;70;0",
         "M1;00000014;62;4;-;-;8;300;2000;3",
         "M1;00000015;63;2;1;3;20;1260;4400;4",
         "M1;broken",
         "F1;69f8c4bc;255;1;Petro"];

      // Recent with a friend: the friend's match only.
      public static const FRIENDS:Array = ["M0;1",
         "M1;69f8c4bc;61;5;3;1;16;1080;25;1",
         "F1;69f8c4bc;255;1;Petro"];

      // A player's matches (P1 names the player): a defeat, a victory, a draw with an old
      // version's replay, one not kept (of before seats were kept: no place); a broken row.
      public static function played(nickname:String) : Array
      {
         return ["P1;4;" + nickname,
            "P2;1ce45cbe;4;117;4;0;4;7;35;1",
            "P2;69f8c4bc;2;61;5;1;9;2;180;1",
            "P2;broken;1",
            "P2;00000021;7;62;4;2;11;6;1500;4",
            "P2;00000022;255;63;2;0;6;8;5000;3"];
      }

      public static function rows(names:Array, first:uint = 0) : Array
      {
         var list:Array = [];
         for(var i:int = 0; i < names.length; i++)
         {
            list.push({"nickname":names[i],"place":first + i,"ranking":0,"ranking_diff":0});
         }
         return list;
      }
   }
}
