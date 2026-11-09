package survarium.constants
{
   // An item modification's state on the wire ({id, state}): 0 a modification; any other is the game's
   // defect, which srp never gives (no wear, no repair: the owner, 2026-11-08).
   public class ModificationState
   {
      
      public static const MODIFICATION:int = 0;
      
      public function ModificationState()
      {
         super();
      }
   }
}
