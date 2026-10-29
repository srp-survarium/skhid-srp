package survarium.player
{
   import survarium.Util;
   import survarium.data.LocalizationData;

   // The speed's own tooltip: the total in the heading, then its four parts
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
      // The total as the row and the heading show it: whole percent, coral below
      // the base speed, green above it.
      public function get value() : String
      {
         var percent:Number = Math.round(total * 100);
         var style:String = percent < 100 ? WeightEffects.WARNING_STYLE : percent > 100 ? "green" : "highlight";
         return Util.styledChar(LocalizationData.format("st_srp_unit_percent",percent.toString()),style);
      }
      public function get title() : String
      {
         return LocalizationData.format("st_srp_speed_title",(Math.round(total * 100)).toString() + "%");
      }
      public function rows() : Array
      {
         return [{icon:11,label:LocalizationData.format("st_srp_speed_weight",WeightEffects.signed(_weight,_weight < 0))},
            {icon:11,label:LocalizationData.format("st_srp_speed_bonuses",WeightEffects.signed(_parts.bonuses,_parts.bonuses < 0))},
            {icon:11,label:LocalizationData.format("st_srp_speed_skills",WeightEffects.signed(_parts.skills,_parts.skills < 0))},
            {icon:11,label:LocalizationData.format("st_srp_speed_equipment",WeightEffects.signed(_parts.equipment,_parts.equipment < 0))}];
      }
   }
}
