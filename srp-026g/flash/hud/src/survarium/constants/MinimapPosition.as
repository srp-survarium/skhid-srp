package survarium.constants
{
   // set_minimap_position's value (an exe setting, 0x233BEB).
   public class MinimapPosition
   {
      
      public static const TOP:int = 0;
      
      public static const BOTTOM:int = 1;
      
      public static const HIDDEN:int = 2;
      
      public function MinimapPosition()
      {
         super();
      }

      // The exe's value as one of the three; any other is BOTTOM, where the stock hud put it.
      public static function parse(value:uint) : int
      {
         return value == TOP || value == HIDDEN ? int(value) : BOTTOM;
      }
   }
}
