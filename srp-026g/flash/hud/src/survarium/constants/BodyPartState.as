package survarium.constants
{
   // A body part's state (crit_player_body_part / set_body_part_state). Named by use: the damage
   // indicator shows the part at full, 0.4 and no health.
   public class BodyPartState
   {
      
      public static const HEALTHY:int = 0;
      
      public static const CRITICAL:int = 1;
      
      public static const BROKEN:int = 2;
      
      public function BodyPartState()
      {
         super();
      }
   }
}
