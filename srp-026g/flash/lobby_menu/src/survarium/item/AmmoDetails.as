package survarium.item
{
   import survarium.constants.SuperCategory;
   import survarium.data.LocalizationData;
   import survarium.data.PropertiesDescriptor;
   import survarium.player.MatchEffects;

   // An ammo card's stats as detail rows (ItemDetailsView), the card's own lines
   // (ammo-cards-1.json, ammo.tsv's cards): labelled by their ui_properties names.
   public final class AmmoDetails
   {
      // [prop_id, the value's unit (a dictionary format, %1$s the number; null: none), the
      // better direction], in the card's order. 164 and 165 only some rounds have.
      private static const ROWS:Array = [[21,"st_srp_unit_percent","higher"],[22,"st_srp_unit_percent","higher"],
         [163,"st_srp_unit_mps","higher"],[164,"st_srp_unit_percent","higher"],[165,null,"neutral"],
         [16,null,"higher"],[15,"st_srp_unit_kg","lower"]];
      private var values:Object;
      // What it pierces (PierceLine), null without the line.
      private var pierce:Object;
      public function AmmoDetails(data:Object, pierce:Object) { values = data; this.pierce = pierce; }
      public static function read(item:Object) : AmmoDetails
      {
         if(item == null || ItemFactory.getCategory(item.category) != SuperCategory.AMMO || !(item.properties is Array)) return null;
         var data:Object = {};
         for each(var prop:Object in item.properties)
         {
            var n:Number = ModificationRolls.value(prop);
            if(isFinite(n)) data[prop.prop_id] = n;
         }
         return new AmmoDetails(data,PierceLine.ofRound(item.properties));
      }
      private function has(id:int) : Boolean { return values[id] !== undefined; }
      // `other`: the compared round's details; a row only it has shows a dash here, so the
      // rows stay aligned.
      public function rows(expanded:Boolean, other:AmmoDetails = null) : Array
      {
         var names:PropertiesDescriptor = PropertiesDescriptor.getPropertiesDescriptor("properties");
         var result:Array = [{kind:"heading",text:LocalizationData.getName("st_srp_ammo_stats")}];
         for each(var row:Array in ROWS)
         {
            var id:int = row[0];
            var label:String = names.getPropertyDescription(id).prop_name;
            if(!has(id))
            {
               if(other != null && other.has(id)) result.push({kind:"stat",id:id,label:label,value:"–",comparison:{kind:"neutral"}});
               continue;
            }
            var n:Number = values[id];
            var text:String = MatchEffects.fixed(n,2);
            result.push({kind:"stat",id:id,label:label,value:row[1] == null ? text : LocalizationData.format(row[1],text),
               comparison:row[2] == "neutral" ? {kind:"neutral"} : {kind:row[2],value:n}});
         }
         PierceLine.row(result,pierce,other == null ? null : other.pierce);
         return result;
      }
   }
}
