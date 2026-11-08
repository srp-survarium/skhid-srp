package
{
   // The lobby server's rows the Watch window takes (survarium.watch.WatchRows), made up for the
   // harness (ReplayButtonsHarness) and the screenshots (WatchShots): PlayerRanking's row objects.
   public class WatchFake
   {
      public static const MATCH:uint = 0x1CE45CBE;

      public static const OTHER:uint = 0x69F8C4BC;

      // Live: 5 good matches (a friend's second in the server's order, one full, one private, one
      // counting down) and a broken row, 30 s behind; the friends Petro (place 3) and Olha in 1ce45cbe.
      public static const LIVE:Array = ["L0;6;30",
         "L1;69f8c4bc;2;2;2;10;10;100;98;1300;0;1",
         "L1;1ce45cbe;13;1;2;8;7;12;9;421;2;1",
         "L1;00000004;1;3;2;3;2;4;1;610;0;0",
         "L1;bad",
         "L1;00000005;4;4;2;5;5;10;7;1800;1;2",
         "L1;00000006;6;1;1;5;4;-;-;0;0;1",
         "F1;1ce45cbe;3;0;Petro",
         "F1;1ce45cbe;9;1;Olha_Kovalenko"];

      // the roster of 1ce45cbe (Eve's row is another match's)
      public static const ROSTER:Array = ["L2;69f8c4bc;1;0;0;0;Eve",
         "L2;1ce45cbe;3;0;5;2;Petro",
         "L2;1ce45cbe;4;0;7;1;Ada",
         "L2;1ce45cbe;5;0;2;3;Сергій_Коваленко",
         "L2;1ce45cbe;9;1;1;3;Olha_Kovalenko",
         "L2;1ce45cbe;10;1;4;4;Bob;the;Builder",
         "L2;1ce45cbe;11;1;0;2;xX_Very_Long_Nickname_Here_Xx"];

      // Recent: each replay state once (0 not recorded .. 4 old version), the friend Petro in the
      // second; a broken row
      public static const RECENT:Array = ["M0;5",
         "M1;00000011;3;1;100;98;16;912;12;0",
         "M1;69f8c4bc;2;2;3;1;20;1260;25;1",
         "M1;00000013;13;1;12;9;15;840;70;2",
         "M1;00000014;9;4;-;-;8;300;2000;3",
         "M1;00000015;5;5;2;2;12;600;4400;4",
         "M1;broken",
         "F1;69f8c4bc;6;1;Petro"];

      // A player's matches (P1 names the player): a victory, a defeat, a draw; a broken row.
      public static function played(nickname:String) : Array
      {
         return ["P1;3;" + nickname,
            "P2;1ce45cbe;4;13;1;1;12;7;35;2",
            "P2;69f8c4bc;2;2;3;0;3;9;180;1",
            "P2;broken;1",
            "P2;00000021;7;9;4;2;5;5;3000;3"];
      }

      public static function rows(names:Array) : Array
      {
         var list:Array = [];
         for each(var name:String in names)
         {
            list.push({"nickname":name,"place":0,"ranking":0,"ranking_diff":0});
         }
         return list;
      }
   }
}
