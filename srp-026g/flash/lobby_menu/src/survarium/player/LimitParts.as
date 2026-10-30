package survarium.player
{
   import survarium.Util;
   import survarium.data.LocalizationData;

   // The carry limit by source, for the weight's tooltip. The lobby's limit is
   // apply_kit_parameter(kit, 5) 0x2F7660 (called at 0x222217): the menu's base
   // (+3118h) through apply_modifier 0x2D5AE0 index 5 (additive: base + the kit's
   // float, the skills: Athlete, +2 kg a rank), then each kit item's parameter and
   // modifications (0x2F7470): the armour's and the pockets' weight-limit
   // modifications (prop 106, e.g. the Duty suit's and the Anomalous Force
   // Protein's 1124, +5 kg). Packrat (modifier 26, -5 % a rank) scales the
   // weight's movement penalty.
   public final class LimitParts
   {
      private var _base:Number;
      private var _skills:Number;
      private var _packrat:Number;
      // {name, value}: each item raising or lowering the limit (KitItems.limit).
      private var _items:Array;
      public function LimitParts(base:Number, skills:Number, packrat:Number, items:Array)
      {
         _base = base;
         _skills = skills;
         _packrat = packrat;
         _items = items;
      }
      public static function parse(base:Number, skills:Number, packrat:Number) : LimitParts
      {
         if(!isFinite(base) || !isFinite(skills) || !isFinite(packrat)) return null;
         return new LimitParts(base,skills,packrat,[]);
      }
      public function withItems(items:KitItems) : LimitParts
      {
         return new LimitParts(_base,_skills,_packrat,items.limit.concat());
      }
      // The limit these add up to (the exe's, beside the weight).
      public function get total() : Number
      {
         var sum:Number = _base + _skills;
         for each(var item:Object in _items) sum += item.value;
         return sum;
      }
      private static function kg(value:Number) : String
      {
         return (value < 0 ? "-" : "+") + LocalizationData.format("st_srp_unit_kg",(Math.round(Math.abs(value) * 100) / 100).toString());
      }
      // The base limit, then each thing that changes it by its own name (the
      // skill, each item), biggest first; Packrat when the kit has it.
      public function rows() : Array
      {
         var sources:Array = _items.concat();
         if(_skills != 0) sources.push({name:SpeedParts.skillName(209),value:_skills});
         // Its own section, headed by the weight's own word (the weight label's key).
         var result:Array = [{heading:LocalizationData.getName("equipment_weight")},{icon:12,label:LocalizationData.format("st_srp_weight_base",
            Util.styledChar(LocalizationData.format("st_srp_unit_kg",(Math.round(_base * 100) / 100).toString()),"white"))}];
         result = result.concat(SourceRows.rows(sources,12,kg));
         if(_packrat < 0)
         {
            var magnitude:Number = Math.round(-_packrat * 1000) / 10;
            result.push({icon:11,label:SpeedParts.skillName(202) + " " +
               Util.styledChar(LocalizationData.format("st_srp_weight_penalty","-" + magnitude.toString()),"green")});
         }
         return result;
      }
   }
}
