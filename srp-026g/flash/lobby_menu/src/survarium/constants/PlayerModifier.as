package survarium.constants
{
   // A player modifier: its index in a kit's 68 floats, the exe's name table at 0x928448
   // (srp's skills::MODIFIER_NAMES; docs/notes/static_info.md 3.1). Only those a weapon's
   // or a medicine's card shows the effect of (docs/notes/alt-weapon-stats.md).
   public class PlayerModifier
   {

      // the stance's spread, at the stance's weight (1 standing, walking, crouching, aiming)
      public static const DISPERSION:int = 0;

      public static const RELOAD_SPEED:int = 13;

      public static const RECOIL:int = 18;

      public static const CHAMBER_A_ROUND:int = 21;

      // draw and holster
      public static const CHANGE_WEAPON_SPEED:int = 22;

      // a medicine's use time (medkit::set_active 0x33D45A)
      public static const DRUG_USING_TIME:int = 33;

      // armored_trap, unused by 0.26g: srp's medical skill ranks, one packed integer
      // (docs/notes/medical-skills.md)
      public static const MEDICAL_RANKS:int = 43;

      public static const AIM_TIME:int = 49;

      public static const DISPERSION_FROM_HIP:int = 56;

      // the health a medicine gives (medkit::tick 0x33D790; Medical Training)
      public static const DRUG_AMOUNT:int = 57;

      // a medicine's damage protection coefficient (medkit::reduce_damage)
      public static const DRUG_HIT_COEFF:int = 58;

      // By card property: the player modifier an armour piece's modification adds to (its
      // `parameters/<name>_modifier`, type 1; ui_properties' prop_id of the modification).
      public static const BY_ARMOUR_PROPERTY:Object = {
         101:DISPERSION,
         102:AIM_TIME,
         114:RELOAD_SPEED,
         115:RECOIL
      };

      public function PlayerModifier()
      {
         super();
      }
   }
}
