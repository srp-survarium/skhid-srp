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
   public final class WeightEffects implements IEffectRows
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
      // The carry limit by source; null when the exe gives none (an older one).
      private var _limit:LimitParts;
      public function WeightEffects(movement:Number, spending:Number, sprint:int, speed:SpeedParts, limit:LimitParts)
      {
         _movement = movement;
         _spending = spending;
         _sprint = sprint;
         _speed = speed;
         _limit = limit;
      }
      public static function parse(movement:Number, spending:Number, sprint:Number, skills:Number = NaN, bearArms:Number = NaN,
         base:Number = NaN, limitSkills:Number = NaN, packrat:Number = NaN) : WeightEffects
      {
         if(!isFinite(movement) || !isFinite(spending) ||
            (sprint != SPRINT_ALLOWED && sprint != SPRINT_WEIGHT && sprint != SPRINT_HEAVYWEIGHT)) return null;
         var speed:SpeedParts = isFinite(skills) && (bearArms == 0 || bearArms == 1) ? new SpeedParts(skills,bearArms == 1,[]) : null;
         return new WeightEffects(movement,spending,int(sprint),speed,LimitParts.parse(base,limitSkills,packrat));
      }
      // The same effects with the kit's items (KitItems: the profile's sums).
      public function withItems(items:KitItems) : WeightEffects
      {
         return new WeightEffects(_movement,_spending,_sprint,
            _speed == null ? null : _speed.withItems(items),
            _limit == null ? null : _limit.withItems(items));
      }
      public function get knowsSpeed() : Boolean { return _speed != null; }
      // The kit's movement speed as a fraction of the base speed.
      public function get speed() : Number { return _speed == null ? NaN : _speed.total(_movement); }
      // The speed's own indicator and tooltip; null for an exe that gives no speed.
      public function get speedExplanation() : SpeedExplanation
      {
         return _speed == null ? null : new SpeedExplanation(_movement,_speed);
      }
      public function get slowsMovement() : Boolean { return _movement < 0; }
      public function get title() : String { return LocalizationData.getName("st_srp_weight_title"); }
      public function get titleValue() : String { return null; }
      public function get titleStyle() : String { return null; }
      // A part's signed percentage, red when it hurts: the weight rows and the speed's parts.
      public static function signed(value:Number, bad:Boolean) : String
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
         var result:Array = [{icon:11,label:LocalizationData.format("st_srp_weight_speed",signed(_movement,_movement < 0))}];
         if(_sprint != SPRINT_HEAVYWEIGHT)
            result.push({icon:16,label:LocalizationData.format("st_srp_weight_sprint_stamina",signed(_spending,_spending > 0))});
         if(_sprint != SPRINT_ALLOWED)
            result.push({icon:11,label:Util.styledChar(LocalizationData.getName(_sprint == SPRINT_WEIGHT ? "st_srp_weight_no_sprint_limit" : "st_srp_weight_no_sprint_heavyweight"),"red")});
         // What raises the carry limit, by source (LimitParts).
         if(_limit != null) result = result.concat(_limit.rows());
         return result;
      }
   }
}
