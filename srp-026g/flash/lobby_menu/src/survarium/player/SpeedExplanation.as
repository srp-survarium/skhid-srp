package survarium.player
{
   import survarium.Util;
   import survarium.data.LocalizationData;

   // The speed's own tooltip: the total in the heading, then each thing that
   // changes it by its own name (the weight, the skill, every item and the
   // weapon), biggest speed-up first, biggest slow-down last, none at 0; green
   // speeds the player up and red slows them down, as the item cards' rows
   // (ModificationRolls.style) (WeightEffects, SpeedParts; docs/notes/data.md
   // "The kit's movement speed").
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
      // The total as the box beside the weight shows it, in the weight's colours.
      public function get value() : String
      {
         var style:String = percent < 100 ? WeightEffects.WARNING_STYLE : percent > 100 ? "green" : "highlight";
         return Util.styledChar(LocalizationData.format("st_srp_unit_percent",percent.toString()),style);
      }
      public function get title() : String { return LocalizationData.format("st_srp_speed_title",titleValue); }
      public function get titleValue() : String { return LocalizationData.format("st_srp_unit_percent",percent.toString()); }
      // The heading's total in the rows' colours (their names in the row renderer's
      // style sheet): green faster, red slower, white at the base speed.
      public function get titleStyle() : String { return percent > 100 ? "green" : percent < 100 ? "red" : "white"; }
      public function rows() : Array
      {
         var sources:Array = _parts.sources();
         if(_weight != 0) sources.push({name:LocalizationData.getName("equipment_weight"),value:_weight});
         return SourceRows.rows(sources,11,percentText);
      }
      private static function percentText(value:Number) : String
      {
         return (value < 0 ? "-" : "+") + LocalizationData.format("st_srp_unit_percent",(Math.round(Math.abs(value) * 1000) / 10).toString());
      }
   }
}
