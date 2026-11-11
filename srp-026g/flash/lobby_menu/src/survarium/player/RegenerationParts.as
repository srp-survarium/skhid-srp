package survarium.player
{
   import survarium.data.LocalizationData;

   // What one regeneration rate sums, one source each: each item of the kit that changes it
   // (KitItems: the armour's bonuses and defects, the boosters a pocket carries) and the kit's
   // own float of its player modifier (the skills' sum). A match's rate is the base rate x
   // max(0, 1 + S) (apply_modifier 0x2D5AE0, k 1): a health pool's with index 2
   // (body_part::regenerate 0x32A4B0), the stamina's with index 3 (player_stamina::regenerate
   // 0x3298C0). docs/notes/regeneration-box.md.
   public final class RegenerationParts
   {
      private var _skills:Number;
      // The skill the float is of, by its id (its row has the skill's name); 0 when the tree
      // has no one skill for it: the row says "Skills".
      private var _skill:uint;
      // {name, value, category}: the items' entries as they count.
      private var _items:Array;
      public function RegenerationParts(skills:Number, skill:uint, items:Array)
      {
         _skills = skills;
         _skill = skill;
         _items = items;
      }
      // Every source with its name and value: the items as the kit lists them (armour head to
      // boots, the pockets), then the skills.
      public function sources() : Array
      {
         var result:Array = _items.concat();
         if(_skills != 0)
            result.push({name:_skill != 0 ? SpeedParts.skillName(_skill) : LocalizationData.getName("st_srp_skills"),value:_skills});
         return result;
      }
      // The rate as a fraction of the base rate.
      public function get total() : Number
      {
         var sum:Number = 1;
         for each(var source:Object in sources()) sum += source.value;
         return Math.max(0,sum);
      }
      public function get percent() : Number { return Math.round(total * 100); }
      // The total as text ("115%"), and its colour among the rows': green faster, red slower,
      // white at the base rate.
      public function get percentText() : String { return LocalizationData.format("st_srp_unit_percent",percent.toString()); }
      public function get style() : String { return percent > 100 ? "green" : percent < 100 ? "red" : "white"; }
      // One row a source but 0, with the modifier atlas icon `icon` for a source without a
      // group icon (a skill, a pocket's booster).
      public function rows(icon:uint) : Array { return SourceRows.rows(sources(),icon,signedPercent); }
      private static function signedPercent(value:Number) : String
      {
         return (value < 0 ? "-" : "+") + LocalizationData.format("st_srp_unit_percent",(Math.round(Math.abs(value) * 1000) / 10).toString());
      }
   }
}
