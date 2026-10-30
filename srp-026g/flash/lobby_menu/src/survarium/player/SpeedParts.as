package survarium.player
{
   import survarium.char.SkillNames;
   import survarium.data.LocalizationData;

   // What movement-speed modifier 4 sums besides the weight, one source each:
   // the kit's skills (the exe's kit float: Rapid, skill 208), each item that
   // changes the speed (KitItems: the armour's bonuses, the boosters a pocket
   // carries, which work while carried: inventory fill 0x2FCB70 -> booster
   // 0x33F960/0x33F990), and the weapon in hand's own movement parameter, left
   // out when it is a penalty and Bear Arms is on (0x2E2C5C..0x2E2C9C).
   public final class SpeedParts
   {
      private var _skills:Number;
      private var _bearArms:Boolean;
      // {name, value}: the items' entries as they count.
      private var _sources:Array;
      public function SpeedParts(skills:Number, bearArms:Boolean, sources:Array)
      {
         _skills = skills;
         _bearArms = bearArms;
         _sources = sources;
      }
      public function withItems(items:KitItems) : SpeedParts
      {
         var sources:Array = [];
         for each(var item:Object in items.speed) sources.push(item);
         if(items.weapon != null && !(_bearArms && items.weapon.value < 0)) sources.push(items.weapon);
         return new SpeedParts(_skills,_bearArms,sources);
      }
      // Every source with its name and value, the skills by their skill's name.
      public function sources() : Array
      {
         var result:Array = _sources.concat();
         if(_skills != 0) result.unshift({name:skillName(208),value:_skills});
         return result;
      }
      // A skill's name as the skill tree shows it (Rapid 208, Athlete 209, Packrat 202),
      // or "Skills" before the exe has given the skills.
      public static function skillName(id:uint) : String
      {
         var name:String = SkillNames.get(id);
         return name != null ? name : LocalizationData.getName("st_srp_skills");
      }
      // base x max(0, 1 + S): the weight's part with these (apply_modifier 0x2D5AE0).
      public function total(weight:Number) : Number
      {
         var sum:Number = 1 + weight;
         for each(var source:Object in sources()) sum += source.value;
         return Math.max(0,sum);
      }
   }
}
