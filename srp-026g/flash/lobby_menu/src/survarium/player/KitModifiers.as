package survarium.player
{
   import survarium.constants.PlayerModifier;

   // The selected kit's own 68 player modifier floats, by index (survarium.constants.PlayerModifier):
   // what the exe sends with root.set_kit_modifiers before each weight callback
   // (client-patches/weight_ui_026g.c). srp puts the sums of the account's skills there; the kit's
   // items are not in them (the movie adds those itself).
   public final class KitModifiers
   {
      public static const COUNT:int = 68;

      private var _values:Array;

      // The medical skills' ranks a medicine card needs, from the packed float 43
      // (docs/notes/medical-skills.md): Potent Painkiller (bits 0..2), Fast Bandaging (3..5).
      private var _painkillerRank:int;

      private var _bandagingRank:int;

      public function KitModifiers(values:Array)
      {
         _values = values;
         // As the exe's medical_skills: an exact integer below 2^24 whose five-rank fields are 0..5,
         // otherwise no medical skill at all.
         var packed:Number = values[PlayerModifier.MEDICAL_RANKS];
         if(packed < 0 || packed > 16777215 || packed != Math.floor(packed)) return;
         var bits:uint = uint(packed);
         for each(var shift:int in [0,3,6,9,12,17,21])
         {
            if((bits >> shift & 7) > 5) return;
         }
         _painkillerRank = bits & 7;
         _bandagingRank = bits >> 3 & 7;
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

      public function get painkillerRank() : int
      {
         return _painkillerRank;
      }

      public function get bandagingRank() : int
      {
         return _bandagingRank;
      }
   }
}
