package survarium.player
{
   import survarium.Util;
   import survarium.data.LocalizationData;

   // The carry limit by source, for the weight's tooltip. The lobby's limit is
   // apply_kit_parameter(kit, 5) 0x2F7660 (called at 0x222217): the menu's base
   // (+3118h) through apply_modifier 0x2D5AE0 index 5 (additive: base + the kit's
   // float, the skills: Athlete, +2 kg a rank), then each kit item's parameter and
   // modifications (0x2F7470): the armour's and the pockets' weight-limit
   // modifications (prop 106, e.g. the Duty suit's and the Anomalous Force Protein's 1124,
   // +5 kg). What the items' own parameters add is the rest of the exe's limit
   // (Equipment; 0 in every config so far). Packrat (modifier 26, -5 % a rank)
   // scales the weight's movement penalty.
   public final class LimitParts
   {
      private var _maximum:Number;
      private var _base:Number;
      private var _skills:Number;
      private var _packrat:Number;
      private var _bonuses:Number;
      private var _boosters:Number;
      public function LimitParts(maximum:Number, base:Number, skills:Number, packrat:Number, bonuses:Number, boosters:Number)
      {
         _maximum = maximum;
         _base = base;
         _skills = skills;
         _packrat = packrat;
         _bonuses = bonuses;
         _boosters = boosters;
      }
      public static function parse(maximum:Number, base:Number, skills:Number, packrat:Number) : LimitParts
      {
         if(!isFinite(maximum) || !isFinite(base) || !isFinite(skills) || !isFinite(packrat)) return null;
         return new LimitParts(maximum,base,skills,packrat,0,0);
      }
      public function withItems(items:KitItems) : LimitParts
      {
         return new LimitParts(_maximum,_base,_skills,_packrat,finite(items.limitBonuses),finite(items.limitBoosters));
      }
      private static function finite(value:Number) : Number { return isFinite(value) ? value : 0; }
      // The items' own parameters: what the exe's limit has beyond the other parts.
      public function get equipment() : Number
      {
         var rest:Number = Math.round((_maximum - _base - _skills - _bonuses - _boosters) * 100) / 100;
         return Math.abs(rest) < 0.01 ? 0 : rest;
      }
      private static function kg(value:Number, signed:Boolean) : String
      {
         var shown:String = (Math.round(Math.abs(value) * 100) / 100).toString();
         return LocalizationData.format("st_srp_unit_kg",(signed ? (value < 0 ? "-" : "+") : "") + shown);
      }
      // Its own icon (the weight-limit modifications', ui_desc icon 12) and colour:
      // green raises the limit, red lowers it (the rows' own style sheet).
      private static function part(key:String, value:Number) : Object
      {
         return {icon:12,label:LocalizationData.format(key,Util.styledChar(kg(value,true),value > 0 ? "green" : "red"))};
      }
      // The base limit, then each source that changes it (shown only when it does),
      // adding up to the limit beside the weight; Packrat when the kit has it.
      public function rows() : Array
      {
         var result:Array = [{icon:12,label:LocalizationData.format("st_srp_weight_base",Util.styledChar(kg(_base,false),"white"))}];
         if(_skills != 0) result.push(part("st_srp_speed_skills",_skills));
         if(_bonuses != 0) result.push(part("st_srp_speed_bonuses",_bonuses));
         if(_boosters != 0) result.push(part("st_srp_speed_boosters",_boosters));
         if(equipment != 0) result.push(part("st_srp_speed_equipment",equipment));
         if(_packrat < 0)
         {
            var magnitude:Number = Math.round(-_packrat * 1000) / 10;
            result.push({icon:11,label:LocalizationData.getName("overweight_penalty_name") + ": " +
               Util.styledChar(LocalizationData.format("st_srp_weight_penalty","-" + magnitude.toString()),"green")});
         }
         return result;
      }
   }
}
