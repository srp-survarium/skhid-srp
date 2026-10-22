package survarium.constants
{
   // match_result of the results screens: the exe sends VICTORY when the own team won, else DEFEAT
   // (docs/notes/results.md).
   public class MatchResult
   {
      
      public static const DEFEAT:int = 0;
      
      // the screens show a result above it as a victory
      public static const DEFEAT_MAX:int = 2;
      
      public static const VICTORY:int = 5;
      
      public function MatchResult()
      {
         super();
      }
   }
}
