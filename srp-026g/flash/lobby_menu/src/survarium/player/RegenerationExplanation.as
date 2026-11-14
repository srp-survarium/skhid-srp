package survarium.player
{
   import survarium.constants.PlayerModifier;
   import survarium.data.LocalizationData;

   // The kit's regeneration: the box's value is the health's, a share of the base rate; its
   // tooltip has two sections, the health's regeneration and the stamina's, each with what
   // changes it by its own name, none at 0, green faster and red slower (RegenerationParts,
   // SourceRows; docs/notes/regeneration-box.md).
   public final class RegenerationExplanation implements IKitStat
   {
      // The modifier atlas icons of the two bonuses (their modifications' ui_desc icon).
      private static const HEALTH_ICON:uint = 9;
      private static const STAMINA_ICON:uint = 10;
      // Breath, the skill of the stamina's float.
      private static const STAMINA_SKILL:uint = 204;
      private var _health:RegenerationParts;
      private var _stamina:RegenerationParts;
      public function RegenerationExplanation(health:RegenerationParts, stamina:RegenerationParts)
      {
         _health = health;
         _stamina = stamina;
      }
      // Of the kit's own floats (the skills) and its items.
      public static function of(kit:KitModifiers, items:KitItems) : RegenerationExplanation
      {
         return new RegenerationExplanation(
            new RegenerationParts(kit.at(PlayerModifier.HEALTH_REGENERATION_SPEED),0,items.regeneration.concat()),
            new RegenerationParts(kit.at(PlayerModifier.STAMINA_REGENERATION_SPEED),STAMINA_SKILL,items.staminaRegeneration.concat()));
      }
      public function get health() : Number { return _health.total; }
      public function get stamina() : Number { return _stamina.total; }
      public function get label() : String { return LocalizationData.getName("st_srp_regeneration_label"); }
      public function get value() : String { return _health.percentText; }
      // The box's value in the lobby's style sheet: coral below the base rate, green above
      // (the speed's).
      public function get valueStyle() : String
      {
         return _health.percent < 100 ? WeightEffects.WARNING_STYLE : _health.percent > 100 ? "green" : "highlight";
      }
      public function get title() : String { return LocalizationData.getName("st_srp_regeneration_health") + ": " + titleValue; }
      public function get titleValue() : String { return _health.percentText; }
      public function get titleStyle() : String { return _health.style; }
      public function rows() : Array
      {
         var value:String = _stamina.percentText;
         var heading:Object = {heading:LocalizationData.getName("st_srp_regeneration_stamina") + ": " + value,headingValue:value,headingStyle:_stamina.style};
         // One array a concat: the game's Scaleform takes each further argument as one element.
         var result:Array = _health.rows(HEALTH_ICON);
         result.push(heading);
         return result.concat(_stamina.rows(STAMINA_ICON));
      }
   }
}
