package survarium.item
{
   import survarium.data.ItemDescriptor;
   import survarium.data.LocalizationData;

   // Material penetration (pierce-line-1.json): the hardest wall material a bullet goes through
   // keeping half its speed and piercing. A round's card line 174 says it for each weapon that fires
   // the round, "<weapon dict id>:<rank>:<dictionary key> ..." (the content build computes it from
   // the game data; the rank is the material's place, 0 the softest); a weapon has no line of its
   // own (its config has no room): it is read from its rounds'.
   // What a card shows: a material {key, rank}, or Varies {key} (no rank: it compares with nothing).
   public final class PierceLine
   {
      public static const ID:int = 174;
      private static const VARIES:Object = {key:"st_srp_pierce_varies"};

      // A round's line, {weapon dict id: {key, rank}}; null without one.
      private static function byWeapon(properties:Array) : Object
      {
         for each(var prop:Object in properties)
         {
            if(prop.prop_id != ID || !prop.prop_value) continue;
            var materials:Object = {};
            for each(var pair:String in String(prop.prop_value).split(" "))
            {
               var parts:Array = pair.split(":");
               if(parts.length == 3) materials[int(parts[0])] = {key:parts[2],rank:int(parts[1])};
            }
            return materials;
         }
         return null;
      }

      // One material when they all agree, Varies when they differ, null for none.
      private static function agreed(materials:Array) : Object
      {
         var found:Object = null;
         for each(var material:Object in materials)
         {
            if(found == null) found = material;
            else if(material.key != found.key) return VARIES;
         }
         return found;
      }

      // A round's: what every weapon that fires it agrees on.
      public static function ofRound(properties:Array) : Object
      {
         var materials:Object = byWeapon(properties), all:Array = [];
         for each(var material:Object in materials) all.push(material);
         return agreed(all);
      }

      // A weapon's: what each of its rounds says for it.
      public static function ofWeapon(weapon:int) : Object
      {
         var items:ItemDescriptor = ItemDescriptor.getItemDescriptor("InventoryItems");
         var all:Array = [];
         for each(var id:int in ItemFactory.getList(weapon))
         {
            var materials:Object = byWeapon(items.getItemDescription(id).item_properties);
            if(materials != null && materials[weapon] !== undefined) all.push(materials[weapon]);
         }
         return agreed(all);
      }

      // Its detail row: the material, compared by rank as the card's other rows (a harder material
      // is better; Varies, or a compared item without the row, stays neutral); a dash beside a
      // compared item that has the row, so the rows stay aligned.
      public static function row(result:Array, material:Object, other:Object) : void
      {
         if(material == null && other == null) return;
         result.push({kind:"stat",id:ID,label:LocalizationData.getName("st_srp_inv_material_pierce"),
            value:material == null ? "–" : LocalizationData.getName(material.key),
            comparison:material != null && material.rank !== undefined ? {kind:"higher",value:material.rank} : {kind:"neutral"}});
      }
   }
}
