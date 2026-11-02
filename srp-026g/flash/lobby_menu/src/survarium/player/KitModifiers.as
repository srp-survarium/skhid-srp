package survarium.player
{
   // The selected kit's own 68 player modifier floats, by index (survarium.constants.PlayerModifier):
   // what the exe sends with root.set_kit_modifiers before each weight callback
   // (client-patches/weight_ui_026g.c). srp puts the sums of the account's skills there; the kit's
   // items are not in them (the movie adds those itself).
   public final class KitModifiers
   {
      public static const COUNT:int = 68;

      private var _values:Array;

      public function KitModifiers(values:Array)
      {
         _values = values;
      }

      // The floats as the exe gives them; null unless they are 68 finite numbers.
      public static function parse(values:Array) : KitModifiers
      {
         if(values == null || values.length != COUNT) return null;
         var parsed:Array = [];
         for each(var value:* in values)
         {
            if(!(value is Number) || !isFinite(value)) return null;
            parsed.push(Number(value));
         }
         return new KitModifiers(parsed);
      }

      public function at(index:int) : Number
      {
         return _values[index];
      }
   }
}
