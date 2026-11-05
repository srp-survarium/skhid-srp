package survarium.player
{
   import survarium.char.SkillNames;
   import survarium.data.LocalizationData;

   // Sprint stamina use by source, for the speed tooltip's second section. The
   // drain is player_stamina::sprint 0x329A70: the base drain through
   // apply_modifier 0x2D5AE0 index 19 (stamina_spending_speed: the weight's node,
   // +1.5 x q, the items' energy-cost modifications, the kit's float) then index 67
   // (stamina_spending_speed_skill: the skill tree's, -2 % a rank), both
   // multiplicative: (1 + S19) x (1 + S67), each at least 0.
   public final class StaminaParts
   {
      private var _weight:Number;
      private var _kit19:Number;
      private var _skill67:Number;
      // {name, value, category}: each item's energy-cost modifications (prop 116).
      private var _items:Array;
      public function StaminaParts(weight:Number, kit19:Number, skill67:Number, items:Array)
      {
         _weight = weight;
         _kit19 = kit19;
         _skill67 = skill67;
         _items = items;
      }
      public static function parse(weight:Number, kit19:Number, skill67:Number) : StaminaParts
      {
         if(!isFinite(weight) || !isFinite(kit19) || !isFinite(skill67)) return null;
         return new StaminaParts(weight,kit19,skill67,[]);
      }
      public function withItems(items:KitItems) : StaminaParts
      {
         return new StaminaParts(_weight,_kit19,_skill67,items.stamina.concat());
      }
      // Each source of index 19 and of index 67, named: the weight, each item, the skills.
      private function sources19() : Array
      {
         var result:Array = _items.concat();
         if(_weight != 0) result.push({name:LocalizationData.getName("equipment_weight"),value:_weight});
         if(_kit19 != 0) result.push({name:LocalizationData.getName("st_srp_skills"),value:_kit19});
         return result;
      }
      public function sources() : Array
      {
         var result:Array = sources19();
         if(_skill67 != 0) result.push({name:SpeedParts.skillName(201),value:_skill67});
         return result;
      }
      // The use as a fraction of the base drain.
      public function get total() : Number
      {
         var s19:Number = 0;
         for each(var source:Object in sources19()) s19 += source.value;
         return Math.max(0,1 + s19) * Math.max(0,1 + _skill67);
      }
      private function get percent() : Number { return Math.round(total * 100); }
      // Its section's heading: "Sprint stamina use: 125%", the total red above the base
      // (more use), green below, white at it (the rows' colours).
      public function heading() : Object
      {
         var value:String = LocalizationData.format("st_srp_unit_percent",percent.toString());
         return {heading:LocalizationData.format("st_srp_weight_sprint_stamina",value),headingValue:value,
            headingStyle:percent > 100 ? "red" : percent < 100 ? "green" : "white"};
      }
      // Less use is better: a cut is green, an increase red.
      public function rows() : Array
      {
         return [heading()].concat(SourceRows.rows(sources(),16,percentText,true));
      }
      private static function percentText(value:Number) : String
      {
         return (value < 0 ? "-" : "+") + LocalizationData.format("st_srp_unit_percent",(Math.round(Math.abs(value) * 1000) / 10).toString());
      }
   }
}
