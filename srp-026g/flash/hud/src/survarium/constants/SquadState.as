package survarium.constants
{
   // A player list row's squad_state (the exe: in a squad ? 1 : 0, 0x2352A3).
   public class SquadState
   {
      
      public static const NONE:int = 0;
      
      public static const IN_SQUAD:int = 1;
      
      // meaning unknown: the exe never sends it; the player lists show the squad icon for it as for IN_SQUAD
      public static const STATE_2:int = 2;
      
      public function SquadState()
      {
         super();
      }
   }
}
