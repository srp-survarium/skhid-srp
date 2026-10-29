package survarium.player
{
   import survarium.Util;
   import survarium.data.LocalizationData;

   // Parsed once at the native callback. Fractions are the weight's contribution
   // to the game's modifier sums, with the selected kit's skills already applied.
   // The movement speed a match gives the kit is base x max(0, 1 + S), S the sum of
   // modifier 4 (base_player::movement_speed 0x2DBB50, apply_modifier 0x2D5AE0): the
   // weight, the kit's skills (the native kit float), the armour's bonuses
   // (armor::add_modifiers 0x33F3A0) and the weapon in hand's own movement
   // parameter, left out when a penalty and Bear Arms is on (0x2E2C5C).
   public final class WeightEffects
   {
      public static const WARNING_STYLE:String = "weight_warning";
      public static const WARNING_COLOR:String = "#F18A78";
      public static const SPRINT_ALLOWED:int = 0;
      public static const SPRINT_WEIGHT:int = 1;
      public static const SPRINT_HEAVYWEIGHT:int = 2;
      private var _movement:Number;
      private var _spending:Number;
      private var _sprint:int;
      // The other parts of the speed; null when the exe gives no skills (an older one).
      private var _speed:SpeedParts;
      public function WeightEffects(movement:Number, spending:Number, sprint:int, speed:SpeedParts)
      {
         _movement = movement;
         _spending = spending;
         _sprint = sprint;
         _speed = speed;
      }
      public static function parse(movement:Number, spending:Number, sprint:Number, skills:Number = NaN, bearArms:Number = NaN) : WeightEffects
      {
         if(!isFinite(movement) || !isFinite(spending) ||
            (sprint != SPRINT_ALLOWED && sprint != SPRINT_WEIGHT && sprint != SPRINT_HEAVYWEIGHT)) return null;
         var speed:SpeedParts = isFinite(skills) && (bearArms == 0 || bearArms == 1) ? new SpeedParts(skills,bearArms == 1,0,0) : null;
         return new WeightEffects(movement,spending,int(sprint),speed);
      }
      // The same effects with the kit's items: the armour's movement bonuses and the
      // weapon in hand's own movement parameter.
      public function withItems(bonuses:Number, weapon:Number) : WeightEffects
      {
         if(_speed == null) return this;
         return new WeightEffects(_movement,_spending,_sprint,_speed.withItems(bonuses,weapon));
      }
      public function get knowsSpeed() : Boolean { return _speed != null; }
      // The kit's movement speed as a fraction of the base speed.
      public function get speed() : Number { return _speed == null ? NaN : _speed.total(_movement); }
      // "96%" beside the speed icon, coral below the base speed, green above it.
      public function get speedText() : String
      {
         if(_speed == null) return "";
         var percent:Number = Math.round(speed * 100);
         var style:String = percent < 100 ? WARNING_STYLE : percent > 100 ? "green" : "highlight";
         return Util.styledChar(LocalizationData.format("st_srp_unit_percent",percent.toString()),style);
      }
      public function get slowsMovement() : Boolean { return _movement < 0; }
      public function get title() : String { return LocalizationData.getName("st_srp_weight_title"); }
      private function percent(value:Number, bad:Boolean) : String
      {
         // One decimal, without float noise or a displayed negative zero.
         var magnitude:Number = Math.round(Math.abs(value) * 1000) / 10;
         // The game's embedded font has the ASCII minus, not U+2212.
         var text:String = (magnitude == 0 ? "" : value < 0 ? "-" : "+") + LocalizationData.format("st_srp_unit_percent",magnitude.toString());
         return Util.styledChar(text,bad ? "red" : "white");
      }
      // Same modifier atlas entries as the game's movement/energy defects
      // (modifications.options: defects 4070 and 4110).
      public function rows() : Array
      {
         var result:Array;
         if(_speed == null)
            result = [{icon:11,label:LocalizationData.format("st_srp_weight_speed",percent(_movement,_movement < 0))}];
         else
         {
            var total:Number = speed;
            result = [{icon:11,label:LocalizationData.format("st_srp_weight_speed",Util.styledChar(LocalizationData.format("st_srp_unit_percent",(Math.round(total * 1000) / 10).toString()),total < 1 ? "red" : "white"))},
               {icon:11,label:LocalizationData.format("st_srp_speed_weight",percent(_movement,_movement < 0))},
               {icon:11,label:LocalizationData.format("st_srp_speed_bonuses",percent(_speed.bonuses,_speed.bonuses < 0))},
               {icon:11,label:LocalizationData.format("st_srp_speed_skills",percent(_speed.skills,_speed.skills < 0))},
               {icon:11,label:LocalizationData.format("st_srp_speed_equipment",percent(_speed.equipment,_speed.equipment < 0))}];
         }
         if(_sprint != SPRINT_HEAVYWEIGHT)
            result.push({icon:16,label:LocalizationData.format("st_srp_weight_sprint_stamina",percent(_spending,_spending > 0))});
         if(_sprint != SPRINT_ALLOWED)
            result.push({icon:11,label:Util.styledChar(LocalizationData.getName(_sprint == SPRINT_WEIGHT ? "st_srp_weight_no_sprint_limit" : "st_srp_weight_no_sprint_heavyweight"),"red")});
         return result;
      }
   }
}
