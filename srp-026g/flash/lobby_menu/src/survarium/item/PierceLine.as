package survarium.item
{
   import survarium.data.ItemDescriptor;
   import survarium.data.LocalizationData;

   // Material penetration (pierce-line-1.json): the hardest wall material a bullet goes through
   // keeping half its speed and piercing. A round's card line 174 says it for each weapon that fires
   // the round, "<weapon dict id>:<dictionary key> ..." (the content build computes it from the game
   // data); a weapon has no line of its own (its config has no room): it is read from its rounds'.
   public final class PierceLine
   {
      public static const ID:int = 174;
      private static const VARIES:String = "st_srp_pierce_varies";

      // A round's line, {weapon dict id: key}; null without one.
      private static function byWeapon(properties:Array) : Object
      {
         for each(var prop:Object in properties)
         {
            if(prop.prop_id != ID || !prop.prop_value) continue;
            var keys:Object = {};
            for each(var pair:String in String(prop.prop_value).split(" "))
            {
               var at:int = pair.indexOf(":");
               if(at > 0) keys[int(pair.substr(0,at))] = pair.substr(at + 1);
            }
            return keys;
         }
         return null;
      }

      // One key when they all agree, Varies when they differ, null for none.
      private static function agreed(keys:Array) : String
      {
         var found:String = null;
         for each(var key:String in keys)
         {
            if(found == null) found = key;
            else if(key != found) return VARIES;
         }
         return found;
      }

      // A round's: what every weapon that fires it agrees on.
      public static function ofRound(properties:Array) : String
      {
         var keys:Object = byWeapon(properties), all:Array = [];
         for each(var key:String in keys) all.push(key);
         return agreed(all);
      }

      // A weapon's: what each of its rounds says for it.
      public static function ofWeapon(weapon:int) : String
      {
         var items:ItemDescriptor = ItemDescriptor.getItemDescriptor("InventoryItems");
         var all:Array = [];
         for each(var id:int in ItemFactory.getList(weapon))
         {
            var keys:Object = byWeapon(items.getItemDescription(id).item_properties);
            if(keys != null && keys[weapon] !== undefined) all.push(keys[weapon]);
         }
         return agreed(all);
      }

      // Its detail row: the material; a dash beside a compared item that has one, so the rows stay aligned.
      public static function row(result:Array, key:String, other:String) : void
      {
         if(key == null && other == null) return;
         result.push({kind:"stat",id:ID,label:LocalizationData.getName("st_srp_inv_material_pierce"),
            value:key == null ? "–" : LocalizationData.getName(key),comparison:{kind:"neutral"}});
      }
   }
}
