package survarium.player
{
   import survarium.Util;
   import survarium.data.LocalizationData;

   // The speed's own tooltip: the total in the heading, then its parts, each
   // green when it speeds the player up and red when it slows them down, as the
   // item cards colour bonuses (ModificationRolls.style); white at 0
   // (WeightEffects, SpeedParts; docs/notes/data.md "The kit's movement speed").
   public final class SpeedExplanation implements IEffectRows
   {
      private var _weight:Number;
      private var _parts:SpeedParts;
      public function SpeedExplanation(weight:Number, parts:SpeedParts)
      {
         _weight = weight;
         _parts = parts;
      }
      public function get total() : Number { return _parts.total(_weight); }
      private function get percent() : Number { return Math.round(total * 100); }
      // The total's style: coral below the base speed, green above it.
      private function get style() : String
      {
         return percent < 100 ? WeightEffects.WARNING_STYLE : percent > 100 ? "green" : "highlight";
      }
      // The total as the box shows it.
      public function get value() : String
      {
         return Util.styledChar(LocalizationData.format("st_srp_unit_percent",percent.toString()),style);
      }
      public function get title() : String
      {
         return LocalizationData.format("st_srp_speed_title",titleValue);
      }
      public function get titleValue() : String { return LocalizationData.format("st_srp_unit_percent",percent.toString()); }
      public function get titleStyle() : String { return style; }
      // A part: "+4%" green, "-5%" red, "0%" white.
      private static function part(key:String, value:Number) : Object
      {
         var magnitude:Number = Math.round(Math.abs(value) * 1000) / 10;
         var text:String = (magnitude == 0 ? "" : value < 0 ? "-" : "+") + LocalizationData.format("st_srp_unit_percent",magnitude.toString());
         return {icon:11,label:LocalizationData.format(key,Util.styledChar(text,magnitude == 0 ? "white" : value > 0 ? "green" : "red"))};
      }
      public function rows() : Array
      {
         return [part("st_srp_speed_weight",_weight),part("st_srp_speed_bonuses",_parts.bonuses),
            part("st_srp_speed_boosters",_parts.boosters),part("st_srp_speed_skills",_parts.skills),
            part("st_srp_speed_equipment",_parts.equipment)];
      }
   }
}
