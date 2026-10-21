package survarium.constants
{
   // How a property's modifier applies (the Flash key modifier_type of ui_properties): the exe sets it
   // to ADD when the property's mod_type is 1 and to MULTIPLY otherwise (0x221654..0x221711).
   public class ModifierType
   {
      
      // added to the value; shown with the property's postfix
      public static const ADD:int = 0;
      
      // value * (1 + modifier); shown as a percentage
      public static const MULTIPLY:int = 1;
      
      // the movie's own: set when the key is missing, then refused
      public static const INVALID:int = 2;
      
      public function ModifierType()
      {
         super();
      }
   }
}
