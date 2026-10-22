package survarium.constants
{
   // Which way a property is better (ui_properties direction, gameplay/static_game_parameters).
   public class PropertyDirection
   {
      
      // e.g. st_inv_dispersion
      public static const LOWER_IS_BETTER:int = 0;
      
      // e.g. st_inv_damage
      public static const HIGHER_IS_BETTER:int = 1;
      
      public function PropertyDirection()
      {
         super();
      }
   }
}
