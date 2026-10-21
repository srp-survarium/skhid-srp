package survarium.constants
{
   // Whether a Player vs Bots match may start (the server's PvbCode, push.rs); UNKNOWN and NO_ANSWER are
   // the movie's own.
   public class PvbCode
   {
      
      public static const NO_ANSWER:int = -2;
      
      // still checking
      public static const UNKNOWN:int = -1;
      
      public static const ALLOWED:int = 0;
      
      public static const GEAR_TOO_HIGH:int = 1;
      
      public static const IN_SQUAD:int = 2;
      
      public static const SEARCHING:int = 3;
      
      public static const IN_MATCH:int = 4;
      
      public static const OUTDATED:int = 5;
      
      public static const OFF:int = 6;
      
      public static const AT_LIMIT:int = 7;
      
      public static const MAINTENANCE:int = 8;
      
      public static const CLAN_TEAM:int = 9;
      
      public static const UNKNOWN_CHOICE:int = 10;
      
      public function PvbCode()
      {
         super();
      }
   }
}
