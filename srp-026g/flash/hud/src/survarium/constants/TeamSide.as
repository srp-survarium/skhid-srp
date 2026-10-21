package survarium.constants
{
   // A player's side in the player lists, relative to the local player (the exe: own team ? 1 : 2,
   // 0x234FF5; docs/notes/results.md).
   public class TeamSide
   {
      
      // not sent by the exe; the movie's test players use it
      public static const NONE:int = 0;
      
      public static const ALLY:int = 1;
      
      public static const ENEMY:int = 2;
      
      public function TeamSide()
      {
         super();
      }
   }
}
