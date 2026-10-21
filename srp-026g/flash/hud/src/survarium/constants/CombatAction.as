package survarium.constants
{
   // A combat log line's action_id (the exe: 0x237DC8 sends KILL, or SUICIDE when the killer is the
   // victim; 0x2378D8 sends 3..5 for battery events).
   public class CombatAction
   {
      
      // shows the victim
      public static const KILL:int = 1;
      
      public static const SUICIDE:int = 2;
      
      public function CombatAction()
      {
         super();
      }
   }
}
