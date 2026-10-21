package survarium.constants
{
   // An item modification's state on the wire ({id, state}): 0 a modification, any other a defect
   // (srp-026g/crates/survarium/src/v026g/modifications.rs). The lobby shows DEFECT as critical damage.
   public class ModificationState
   {
      
      public static const MODIFICATION:int = 0;
      
      public static const DEFECT:int = 1;
      
      public function ModificationState()
      {
         super();
      }
   }
}
