package survarium.item
{
   import survarium.data.LocalizationData;

   // Material penetration (pierce-line-1.json): the hardest wall material a weapon's bullet gets
   // through with a round (piercing above the material's armour, as the game's try_pierce), and the
   // damage it keeps there. A round's card line 174 says it for each weapon that fires the round,
   // "<weapon dict id>:<rank>:<dictionary key>:<percent> ..." (the content build computes it from
   // the game data; the rank is the material's place, 0 the softest). A weapon config has no room
   // for a line: the weapon's card reads its rounds' (WeaponDetails).
   public final class PierceLine
   {
      public static const ID:int = 174;

      // What a round's line says for `weapon`: {key, rank, percent}; null when it says nothing.
      public static function forWeapon(properties:Array, weapon:int) : Object
      {
         for each(var prop:Object in properties)
         {
            if(prop.prop_id != ID || !prop.prop_value) continue;
            for each(var pair:String in String(prop.prop_value).split(" "))
            {
               var parts:Array = pair.split(":");
               if(parts.length == 4 && int(parts[0]) == weapon) return {rank:int(parts[1]),key:parts[2],percent:int(parts[3])};
            }
         }
         return null;
      }

      // "Concrete · 52%".
      public static function text(pierce:Object) : String
      {
         return LocalizationData.format("st_srp_pierce_value",LocalizationData.getName(pierce.key),
            LocalizationData.format("st_srp_unit_percent",pierce.percent));
      }

      // The harder material is better, then the more damage kept (a percent is at most 100).
      public static function score(pierce:Object) : Number
      {
         return pierce.rank * 1000 + pierce.percent;
      }
   }
}
