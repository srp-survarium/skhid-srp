package survarium.constants
{
   // A player modifier: its index in a kit's 68 floats, the exe's name table at 0x928448
   // (srp's skills::MODIFIER_NAMES; docs/notes/static_info.md 3.1). Only those a weapon's, a
   // medicine's, a grenade's, a trap's or an armour piece's card shows the effect of
   // (docs/notes/alt-weapon-stats.md) and the regeneration box's (docs/notes/regeneration-box.md).
   public class PlayerModifier
   {

      // the stance's spread, at the stance's weight (1 standing, walking, crouching, aiming)
      public static const DISPERSION:int = 0;

      // a health pool's regeneration rate (body_part::regenerate 0x32A4B0)
      public static const HEALTH_REGENERATION_SPEED:int = 2;

      // the stamina's regeneration rate (player_stamina::regenerate 0x3298C0)
      public static const STAMINA_REGENERATION_SPEED:int = 3;

      // a grenade's splash damage, stopping power and stamina damage, a trap's hit
      public static const DEVICE_DAMAGE:int = 11;

      // a grenade's splash radius
      public static const SPLASH_RADIUS:int = 12;

      public static const RELOAD_SPEED:int = 13;

      public static const RECOIL:int = 18;

      public static const CHAMBER_A_ROUND:int = 21;

      // draw and holster
      public static const CHANGE_WEAPON_SPEED:int = 22;

      // a grenade's throw speed (skill 203, Grenadier)
      public static const GRENADE_THROW_POWER:int = 27;

      // a medicine's use time (medkit::set_active 0x33D45A)
      public static const DRUG_USING_TIME:int = 33;

      // artefact_power, unused by 0.26g: the artefact tree's skills, one packed integer
      // (docs/notes/artefact-skills.md, Transport)
      public static const ARTEFACT_RANKS:int = 40;

      // armored_trap, unused by 0.26g: srp's medical skill ranks, one packed integer
      // (docs/notes/medical-skills.md)
      public static const MEDICAL_RANKS:int = 43;

      // can_defuse_traps, unused by 0.26g: the medical combo skills' ranks, one packed integer
      // (docs/notes/medical-skills.md)
      public static const COMBO_RANKS:int = 45;

      public static const AIM_TIME:int = 49;

      public static const DISPERSION_FROM_HIP:int = 56;

      // the health a medicine gives (medkit::tick 0x33D790; Medical Training)
      public static const DRUG_AMOUNT:int = 57;

      // a medicine's damage protection coefficient (medkit::reduce_damage)
      public static const DRUG_HIT_COEFF:int = 58;

      // Bear Arms (skill 112), additive: above 0 the drawn weapon's movement penalty is not added
      // to the speed (weapon_core::initialize 0x2E2C5C..0x2E2C9C)
      public static const BEAR_ARMS:int = 62;

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
