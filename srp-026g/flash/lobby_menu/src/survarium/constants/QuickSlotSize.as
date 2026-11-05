package survarium.constants
{
   // The share of a quick slot (a pocket) one unit of an item takes (docs/notes/one-pocket-a-type.md).
   public class QuickSlotSize
   {

      // One unit: the pocket holds its size.
      public static const HALF:int = 1;

      // The whole pocket, one in it (fills_whole_pocket: the boosters).
      public static const WHOLE:int = 2;

      // Two units (parameters/pocket_units 2: the RGD-5): the pocket holds half its size, at least one.
      public static const PAIR:int = 3;

      // The card property that carries parameters/pocket_units (content-edit pocket-units-1): the exe
      // gives the movie fills_whole_pocket but no other key of an item's parameters.
      private static const POCKET_UNITS:int = 163;

      public function QuickSlotSize()
      {
         super();
      }

      // The share of an item's description (the exe's item descriptor).
      public static function of(description:Object) : int
      {
         if(description.fills_whole_pocket)
         {
            return WHOLE;
         }
         for each(var prop:Object in description.item_properties)
         {
            if(prop.prop_id == POCKET_UNITS && int(prop.prop_value) == 2)
            {
               return PAIR;
            }
         }
         return HALF;
      }
   }
}
