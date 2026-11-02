package survarium.player
{
   import survarium.constants.PlayerModifier;
   import survarium.constants.ProfileSlot;
   import survarium.data.LocalizationData;
   import survarium.item.ModificationRolls;

   // What a match adds to a weapon's card stats in the selected kit, shown while Alt is held
   // (docs/notes/alt-weapon-stats.md). A player modifier's sum S is the kit's own float (the
   // skills) plus the kit's armour's modifications of it (armor::add_modifiers 0x33F3A0): the sum
   // apply_modifier 0x2D5AE0 takes, B x max(0, 1 + k x S), k 1 at every site below.
   public final class MatchEffects
   {
      // Between a changed value's original and current ("0.2 -> 0.196"): plain ASCII, whatever the
      // font has.
      public static const ARROW:String = " -> ";

      // A change's verdict for the player: the card's green (better) or red (worse).
      public static const BETTER:String = "better";

      public static const WORSE:String = "worse";

      // The most decimals a changed value is shown with (change).
      private static const MAX_DECIMALS:int = 6;

      // null when the exe gave no floats: the skills are unknown and left out.
      private var _kit:KitModifiers;

      // By modifier index: the sum of the armour's modifications of it.
      private var _armour:Object = {};

      // `items`: the kit's items (ItemData with slotId); its armour's modifications (built-in,
      // rolled, defects) count.
      public function MatchEffects(kit:KitModifiers, items:Array)
      {
         _kit = kit;
         for each(var item:Object in items)
         {
            if(item == null || item.slotId < ProfileSlot.HEAD || item.slotId > ProfileSlot.FOOT) continue;
            for each(var modifier:Object in item.modifiers)
            {
               if(modifier == null) continue;
               for each(var prop:Object in modifier.props)
               {
                  var index:* = PlayerModifier.BY_ARMOUR_PROPERTY[prop.prop_id];
                  var amount:Number = ModificationRolls.value(prop);
                  if(index === undefined || !isFinite(amount)) continue;
                  _armour[index] = (_armour[index] == undefined ? 0 : _armour[index]) + amount;
               }
            }
         }
      }

      // The card's line saying what is on.
      public function get label() : String
      {
         return LocalizationData.getName(_kit != null ? "st_srp_weapon_with_kit_skills" : "st_srp_weapon_with_kit");
      }

      // A weapon card stat (its prop_id), already with the weapon's own modifications, as a
      // match has it with the kit and the skills.
      public function stat(propId:int, value:Number) : Number
      {
         switch(propId)
         {
            case 133:
               // Aim time: instant_aim_start 0x2E1B88 (index 49 on parameters/aim_time).
               return value * factor(PlayerModifier.AIM_TIME);
            case 138:
               // Reload: the animation's time scale (length / reload_time) x max(0, 1 + S13)
               // (0x30E210; the shotgun's, pistol's and double barrel's the same).
               return value / factor(PlayerModifier.RELOAD_SPEED);
            case 160:
               // Chambering: length / chamber_a_round_time, x max(0, 1 + S21) (0x30E55B).
               return value / factor(PlayerModifier.CHAMBER_A_ROUND);
            case 142:
            case 143:
               // Draw and holster: length / show_time (hide_time), x max(0, 1 + S22) (0x30DF7B).
               return value / factor(PlayerModifier.CHANGE_WEAPON_SPEED);
            case 146:
            case 147:
            case 148:
            case 149:
            case 150:
            case 151:
               // A shot's kick, each axis x max(0, 1 + S18) (weapon_recoil_calculator::on_shot 0x331D95).
               return value * factor(PlayerModifier.RECOIL);
            case 152:
               // Hip fire: the stance's spread through index 56 (get_target_koef 0x331776), the
               // whole target through index 0 (dispersion_calculator::tick 0x3313C1).
               return value * factor(PlayerModifier.DISPERSION_FROM_HIP) * factor(PlayerModifier.DISPERSION);
            case 153:
               // A shot's spread through index 0 (dispersion_calculator::fire 0x331150).
               return value * factor(PlayerModifier.DISPERSION);
            default:
               return value;
         }
      }

      private function factor(index:int) : Number
      {
         var sum:Number = (_kit == null ? 0 : _kit.at(index)) + (_armour[index] == undefined ? 0 : _armour[index]);
         return Math.max(0,1 + sum);
      }

      // Whether a changed value reads otherwise than the original at the most decimals shown.
      public static function changed(before:Number, after:Number) : Boolean
      {
         return fixed(before,MAX_DECIMALS) != fixed(after,MAX_DECIMALS);
      }

      // "before -> after", each rounded to `decimals`, or to the first decimal at which they
      // differ (decimalsFor): a change never reads "x -> x".
      public static function change(before:Number, after:Number, decimals:int) : String
      {
         var digits:int = decimalsFor(before,after,decimals);
         return fixed(before,digits) + ARROW + fixed(after,digits);
      }

      // Whether a change of a stat is better or worse for the player, by its direction.
      public static function verdict(before:Number, after:Number, higherIsBetter:Boolean) : String
      {
         return (after > before) == higherIsBetter ? BETTER : WORSE;
      }

      // `decimals`, or more up to the first at which the two values read otherwise.
      public static function decimalsFor(before:Number, after:Number, decimals:int) : int
      {
         var digits:int = decimals;
         while(digits < MAX_DECIMALS && fixed(before,digits) == fixed(after,digits)) digits++;
         return digits;
      }

      // `n` rounded to `digits` decimals, without trailing zeros.
      public static function fixed(n:Number, digits:int) : String
      {
         var scale:Number = Math.pow(10,digits);
         return String(Math.round(n * scale) / scale);
      }
   }
}
