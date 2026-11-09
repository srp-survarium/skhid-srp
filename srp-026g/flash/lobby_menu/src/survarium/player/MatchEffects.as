package survarium.player
{
   import survarium.constants.ItemCategory;
   import survarium.constants.PlayerModifier;
   import survarium.constants.ProfileSlot;
   import survarium.data.LocalizationData;
   import survarium.item.ModificationRolls;

   // What a match adds to a weapon's, a medicine's, a grenade's or a trap's card stats in the selected kit, shown while Alt is held
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

      // Whether Alt shows an item's card as a match has it: a weapon's, a medicine's, a grenade's or a trap's.
      public static function covers(item:Object) : Boolean
      {
         return item != null && (item.isWeapon || item.category == ItemCategory.MEDKITS || throwable(item));
      }

      private static function throwable(item:Object) : Boolean
      {
         return item.category == ItemCategory.GRENADES || item.category == ItemCategory.BOOBY_TRAPS;
      }

      // A card stat (its prop_id) of `item`, already with the item's own modifications, as a
      // match has it with the kit and the skills.
      public function stat(item:Object, propId:int, value:Number) : Number
      {
         if(item.category == ItemCategory.MEDKITS) return medicine(item,propId,value);
         if(throwable(item)) return thrown(propId,value);
         switch(propId)
         {
            case 133:
               // Aim time: instant_aim_start 0x2E1B88 (index 49 on parameters/aim_time).
               return value * factor(PlayerModifier.AIM_TIME);
            case 138:
            case 162:
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

      // A medicine card stat (docs/notes/medical-skills.md; client-patches/rust/patches/src/
      // medical_skills.rs): the card's healing lines are the share of a pool (%), its pain
      // protection the % a hit loses.
      private function medicine(item:Object, propId:int, value:Number) : Number
      {
         var painkiller:int = _kit == null ? 0 : _kit.painkillerRank;
         var bandaging:int = _kit == null ? 0 : _kit.bandagingRank;
         switch(propId)
         {
            case 19:
               // Use time: x max(0, 1 + S33) (set_active 0x33D45A), then Fast Bandaging's
               // -5 % a rank on a medicine that heals (data/influences: a healing line on its card).
               return value * factor(PlayerModifier.DRUG_USING_TIME) * (heals(item) ? 1 - bandaging * 0.05 : 1);
            case 18:
               // Active time: Potent Painkiller's +1.5 s a rank on a pain protector.
               return has(item,PAIN_PROTECTION) ? value + painkiller * 1.5 : value;
            case PAIN_PROTECTION:
               // The hit's coefficient c = (1 - protection) x max(0, 1 + S58); Potent Painkiller
               // adds a percentage point a rank: max(0, c - 0.01 x rank) (medical::protect). In %,
               // so that no skill leaves the card's own number exact.
               var coefficient:Number = (100 - value) * factor(PlayerModifier.DRUG_HIT_COEFF);
               coefficient = Math.max(0,coefficient - painkiller);
               return 100 - coefficient;
            case 13:
            case 135:
            case 136:
            case 137:
               // Healing: each tick's amount x max(0, 1 + S57) (medkit::tick 0x33D790; Medical
               // Training +8 % a rank), a pool never above its maximum.
               return Math.min(100,value * factor(PlayerModifier.DRUG_AMOUNT));
            default:
               return value;
         }
      }

      // The lines a medicine's card adds below its own: what the medical skills do while it is
      // active or as it heals, none of them a stat of the item (medical_skills.rs). Each is a
      // share of the game's own (100 %), {name, before: 100, after, higherIsBetter}; a skill
      // without a rank adds no line.
      public function medicineSkillLines(item:Object) : Array
      {
         var lines:Array = [];
         if(_kit == null || item.category != ItemCategory.MEDKITS) return lines;
         if(has(item,PAIN_PROTECTION))
         {
            // While a painkiller is active (set_active's flag on a pain protector): Numb Legs, the
            // sprint's stamina x (1 - 0.2 x rank) (player_stamina::sprint 0x329AAD); Unflinching,
            // the hits' stopping power x (1 - 0.1 x rank); Concentration, the sight's sway x (1 - 0.05
            // x rank) (breath_vibration_calculator::tick 0x31D727, added to modifier 61's own).
            skillLine(lines,"st_srp_medicine_sprint_stamina",1 - _kit.numbLegsRank * 0.2,false);
            skillLine(lines,"st_srp_medicine_hit_slow",1 - _kit.unflinchingRank * 0.1,false);
            skillLine(lines,"st_srp_medicine_sight_sway",1 - _kit.concentrationRank * 0.05,false);
         }
         // Take Cover: each heal tick's amount x (1 + 0.1 x rank) while crouched (medkit::tick
         // 0x33D948, after Medical Training's S57), on a medicine that heals.
         if(heals(item)) skillLine(lines,"st_srp_medicine_crouched_healing",1 + _kit.takeCoverRank * 0.1,true);
         return lines;
      }

      private static function skillLine(lines:Array, key:String, factor:Number, higherIsBetter:Boolean) : void
      {
         if(factor != 1) lines.push({name:LocalizationData.getName(key),before:100,after:100 * factor,higherIsBetter:higherIsBetter});
      }

      // A grenade's or a trap's card stat (docs/notes/alt-weapon-stats.md), each as the match's
      // apply_modifier takes it.
      private function thrown(propId:int, value:Number) : Number
      {
         switch(propId)
         {
            case 1:
            case 132:
            case 167:
               // Damage, stopping power and stamina damage: a grenade's splash (0x30A350), a trap's
               // hit on the foot (0x304A40), x max(0, 1 + S11).
               return value * factor(PlayerModifier.DEVICE_DAMAGE);
            case 23:
               // Radius: x max(0, 1 + S12) (0x30A350).
               return value * factor(PlayerModifier.SPLASH_RADIUS);
            case 169:
               // Throw speed: the grenade's velocity, throw_power x max(0, 1 + S27) (0x30A2DE;
               // Grenadier +10 % a rank). Anything else the kit adds to S27 (a booster in a
               // pocket) belongs in the sums the constructor makes, not here.
               return value * factor(PlayerModifier.GRENADE_THROW_POWER);
            default:
               return value;
         }
      }

      // A medicine card's pain protection (%), the painkiller's.
      private static const PAIN_PROTECTION:int = 17;

      // A medicine card's healing lines: infection, arms and legs, body and head, pain.
      private static const HEALING:Array = [13,135,136,137];

      private static function heals(item:Object) : Boolean
      {
         for each(var propId:int in HEALING)
         {
            if(has(item,propId)) return true;
         }
         return false;
      }

      private static function has(item:Object, propId:int) : Boolean
      {
         for each(var prop:Object in item.properties)
         {
            if(prop.prop_id == propId) return true;
         }
         return false;
      }

      // What a drawn weapon's own movement parameter (its card's movement line) adds to the
      // speed's sum S4 in a match: weapon_core::initialize (0x2E2A30, every firearm: weapon::initialize
      // 0x2449E0 when it is put in hand; weapon_core::deserialize 0x2E3490 the same at 0x2E37A4) adds
      // it unless S62 (Bear Arms, additive: 0 + S) is above 0 and the parameter is not above 0
      // (0x2E2C5C..0x2E2C9C). A bonus always counts.
      public function weaponMovement(value:Number) : Number
      {
         return sum(PlayerModifier.BEAR_ARMS) > 0 && value <= 0 ? 0 : value;
      }

      private function sum(index:int) : Number
      {
         return (_kit == null ? 0 : _kit.at(index)) + (_armour[index] == undefined ? 0 : _armour[index]);
      }

      private function factor(index:int) : Number
      {
         return Math.max(0,1 + sum(index));
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
         // Scaleform's Number.toString can expose binary tails even after
         // rounding. Keep the fixed decimal text when removing trailing zeros.
         var text:String = (Math.round(n * scale) / scale).toFixed(digits);
         if(digits > 0) text = text.replace(/0+$/, "").replace(/\.$/, "");
         return text == "-0" ? "0" : text;
      }
   }
}
