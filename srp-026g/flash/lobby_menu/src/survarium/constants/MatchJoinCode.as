package survarium.constants
{
   // Whether a running match may be joined (docs/notes/lobby.md, the matches window).
   public class MatchJoinCode
   {
      
      public static const NONE:int = 0;
      
      public static const MAY_JOIN:int = 1;
      
      public static const BALANCED:int = 2;
      
      public static const NO_PLACE:int = 3;
      
      public static const ANOTHER_JOINING:int = 4;
      
      public static const NOT_IN_PLAY:int = 5;
      
      public static const ENDS_SOON:int = 6;
      
      public static const NO_LATE_JOIN:int = 7;
      
      public static const IN_MATCH:int = 8;
      
      public static const IN_SQUAD:int = 9;
      
      public static const OUTDATED:int = 10;
      
      public static const PLAYED_BEFORE:int = 11;
      
      public static const MAINTENANCE:int = 12;
      
      public static const QUEUED:int = 13;
      
      public function MatchJoinCode()
      {
         super();
      }
   }
}
