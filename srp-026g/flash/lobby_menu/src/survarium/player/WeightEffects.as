package survarium.player
{
   import survarium.Util;
   import survarium.data.LocalizationData;

   // Parsed once at the native callback. Fractions are the weight's contribution
   // to the game's modifier sums, with the selected kit's skills already applied.
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
      public function WeightEffects(movement:Number, spending:Number, sprint:int)
      {
         _movement = movement;
         _spending = spending;
         _sprint = sprint;
      }
      public static function parse(movement:Number, spending:Number, sprint:Number) : WeightEffects
      {
         if(!isFinite(movement) || !isFinite(spending) ||
            (sprint != SPRINT_ALLOWED && sprint != SPRINT_WEIGHT && sprint != SPRINT_HEAVYWEIGHT)) return null;
         return new WeightEffects(movement,spending,int(sprint));
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
         var result:Array = [{icon:11,label:LocalizationData.format("st_srp_weight_speed",percent(_movement,_movement < 0))}];
         if(_sprint != SPRINT_HEAVYWEIGHT)
            result.push({icon:16,label:LocalizationData.format("st_srp_weight_sprint_stamina",percent(_spending,_spending > 0))});
         if(_sprint != SPRINT_ALLOWED)
            result.push({icon:11,label:Util.styledChar(LocalizationData.getName(_sprint == SPRINT_WEIGHT ? "st_srp_weight_no_sprint_limit" : "st_srp_weight_no_sprint_heavyweight"),"red")});
         return result;
      }
   }
}
