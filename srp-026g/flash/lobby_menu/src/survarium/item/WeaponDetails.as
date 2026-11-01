package survarium.item
{
   import survarium.data.LocalizationData;
   import survarium.player.MatchEffects;

   // Read-only weapon stats from weapon-details-ui-1.json. These are not item
   // modifications and must never be added to an item's bonus/modifier sums.
   public final class WeaponDetails
   {
      // The detail properties weapon-details-ui-1 gives a weapon's card.
      private static const FIRST:int = 138, LAST:int = 160;
      // Chambering time, on bolt and pump weapons only (parameters/chamber_a_round_time).
      private static const CHAMBER:int = 160;
      private var values:Object;
      // In a match's view (Alt): the weapon's own base values, before its modifications, the kit
      // and the skills, for the rows that change ("base -> in a match"); null in the card's own.
      private var base:Object = null;
      public function WeaponDetails(data:Object) { values = data; }
      // The weapon's stats with its modifications; with effects (held Alt), as a match has them
      // with the kit and the skills too.
      public static function read(item:Object, effects:MatchEffects = null) : WeaponDetails
      {
         if(item == null || !item.isWeapon) return null;
         var details:WeaponDetails = fromProperties(item.properties);
         if(details == null) return null;
         if(effects != null) details.base = copy(details.values);
         details.applyWeaponModifiers(item.modifiers);
         if(effects != null) for(var id:String in details.values) details.values[id] = effects.stat(int(id),details.values[id]);
         return details;
      }
      private static function copy(from:Object) : Object
      {
         var to:Object = {};
         for(var id:String in from) to[id] = from[id];
         return to;
      }
      private static function fromProperties(properties:Array) : WeaponDetails
      {
         var data:Object = {};
         for each(var prop:Object in properties)
         {
            if(prop.prop_id < FIRST || prop.prop_id > LAST) continue;
            var n:Number = ModificationRolls.value(prop);
            if(isFinite(n) && n >= 0) data[prop.prop_id] = n;
         }
         // Old content keeps the old distance row, with no invented details.
         if(data[138] == undefined || data[139] == undefined || data[140] == undefined || data[141] == undefined) return null;
         // The native dispersion loader (0x330FE3) defaults the optional
         // from_the_hip_multiplier to 1, including Kiparis and AK-74M.
         if(data[152] == undefined) data[152] = 1;
         return new WeaponDetails(data);
      }
      private function applyWeaponModifiers(modifiers:Array) : void
      {
         // Only this weapon's list, which already includes its defaults. Never
         // read the profile's equipment/skills or add modifiersDefault twice.
         // By property: 24 damage distance, 114 reload, 115 recoil, 119 chambering.
         var sums:Object = {24:0,114:0,115:0,119:0};
         for each(var modifier:Object in modifiers)
         {
            if(modifier == null || !(modifier.props is Array)) continue;
            for each(var prop:Object in modifier.props)
            {
               if(sums[prop.prop_id] === undefined) continue;
               var amount:Number = ModificationRolls.value(prop);
               if(isFinite(amount)) sums[prop.prop_id] += amount;
            }
         }
         // item_config_cook's type-2 operator sums modifiers before applying
         // base * max(0, 1 + sum). Weapon reload UI uses the opposite sign.
         values[139] *= Math.max(0,1 + sums[24]);
         values[140] *= Math.max(0,1 + sums[24]);
         values[138] *= Math.max(0,1 - sums[114]);
         // Recoil rolls scale the six kick bounds the loader 0x331840 reads
         // (weapon-rolls-1). Dispersion bonuses stay off the unrelated
         // spread-growth/recovery parameters.
         for(var id:int = 146; id <= 151; id++) if(has(id)) values[id] *= Math.max(0,1 + sums[115]);
         if(has(CHAMBER)) values[CHAMBER] *= Math.max(0,1 + sums[119]);
      }
      private function has(id:int) : Boolean { return values[id] !== undefined; }
      private static function decimals(n:Number) : int { return n > 0 && n < 0.1 ? 3 : 2; }
      private static function number(n:Number) : String { return MatchEffects.fixed(n,decimals(n)); }
      // A row's value text: the value, or in a match's view "base -> in a match" where it changed.
      private function shown(before:Number, after:Number, digits:int) : String
      {
         return base != null && MatchEffects.changed(before,after) ? MatchEffects.change(before,after,digits) : MatchEffects.fixed(after,digits);
      }
      // `unit` is the dictionary text of the value with its unit (%1$s the number).
      private function scalar(result:Array, id:int, key:String, direction:String, unit:String = null, multiplier:Number = 1) : void
      {
         if(!has(id)) return;
         var n:Number = values[id] * multiplier;
         var text:String = shown(base == null ? n : base[id] * multiplier,n,decimals(n));
         result.push({kind:"stat",id:id,label:LocalizationData.getName(key),
            value:unit == null ? text : LocalizationData.format(unit,text),
            comparison:direction == "neutral" ? {kind:"neutral"} : {kind:direction,value:Number(number(n))}});
      }
      // Chambering time with its rolls, as every other row. A weapon
      // without one shows a dash beside a compared one that has it, so the rows stay aligned.
      private function chamber(result:Array, other:WeaponDetails) : void
      {
         var label:String = LocalizationData.getName("st_srp_weapon_chamber");
         if(!has(CHAMBER))
         {
            if(other != null && other.has(CHAMBER)) result.push({kind:"stat",id:CHAMBER,label:label,value:"\u2013",comparison:{kind:"neutral"}});
            return;
         }
         var modified:String = number(values[CHAMBER]);
         var value:String = LocalizationData.format("st_srp_unit_seconds",shown(base == null ? values[CHAMBER] : base[CHAMBER],values[CHAMBER],decimals(values[CHAMBER])));
         result.push({kind:"stat",id:CHAMBER,label:label,value:value,comparison:{kind:"lower",value:Number(modified)}});
      }
      private static function recoilNumber(n:Number) : String
      {
         return MatchEffects.fixed(n,3);
      }
      // A kick range "low…high"; in a match's view "base -> in a match" where it changed, each
      // end to the decimals at which it differs.
      private function kickRange(result:Array, id:int, key:String, low:Array, high:Array, comparison:Object) : void
      {
         var value:String = recoilNumber(low[1]) + "…" + recoilNumber(high[1]);
         if(base != null && (MatchEffects.changed(low[0],low[1]) || MatchEffects.changed(high[0],high[1])))
         {
            var lowDigits:int = MatchEffects.decimalsFor(low[0],low[1],2), highDigits:int = MatchEffects.decimalsFor(high[0],high[1],2);
            value = MatchEffects.fixed(low[0],lowDigits) + "…" + MatchEffects.fixed(high[0],highDigits) + " -> " +
               MatchEffects.fixed(low[1],lowDigits) + "…" + MatchEffects.fixed(high[1],highDigits);
         }
         result.push({kind:"stat",id:id,label:LocalizationData.getName(key),value:value,comparison:comparison});
      }
      // `variance`: [base, shown].
      private function variability(result:Array, id:int, key:String, variance:Array) : void
      {
         var value:Number = Math.sqrt(variance[1]);
         result.push({kind:"stat",id:id,label:LocalizationData.getName(key),value:shown(Math.sqrt(variance[0]),value,3),
            comparison:{kind:"lower",value:Number(recoilNumber(value))}});
      }
      // A recoil figure of the base values and of the shown ones: [base, shown] (the same twice
      // in the card's own view).
      private function both(figure:Function) : Array
      {
         return [figure(base == null ? values : base),figure(values)];
      }
      private function recoil(result:Array) : void
      {
         // Raw initial shot impulse, before accumulated-kick scaling and PID
         // recovery. Each axis setting is an independent uniform draw.
         if(has(146) && has(147) && values[146] <= values[147])
         {
            kickRange(result,146,"st_srp_weapon_kick_up",both(function(v:Object):Number { return v[146]; }),
               both(function(v:Object):Number { return v[147]; }),{kind:"lower-range",
               low:Number(recoilNumber(values[146])),high:Number(recoilNumber(values[147]))});
            variability(result,147,"st_srp_weapon_kick_up_variability",both(function(v:Object):Number { return Math.pow(v[147] - v[146],2) / 12; }));
         }
         if(has(148) && has(149) && has(150) && has(151) && values[148] <= values[149] && values[150] <= values[151])
         {
            // The engine subtracts left from right. A signed range has no
            // universal better direction; compare its variability separately.
            kickRange(result,148,"st_srp_weapon_kick_side",both(function(v:Object):Number { return v[150] - v[149]; }),
               both(function(v:Object):Number { return v[151] - v[148]; }),{kind:"neutral"});
            variability(result,149,"st_srp_weapon_kick_side_variability",
               both(function(v:Object):Number { return (Math.pow(v[149] - v[148],2) + Math.pow(v[151] - v[150],2)) / 12; }));
         }
      }
      // `other`: the compared weapon's details, for the rows only one of the two has.
      public function rows(expanded:Boolean, other:WeaponDetails = null) : Array
      {
         var result:Array = [{kind:"heading",text:LocalizationData.getName("st_srp_weapon_stats")}];
         scalar(result,138,"st_srp_weapon_reload","lower","st_srp_unit_seconds");
         chamber(result,other);
         scalar(result,139,"st_srp_weapon_full_damage","higher","st_srp_unit_meters");
         scalar(result,140,"st_srp_weapon_minimum_damage_from","higher","st_srp_unit_meters");
         scalar(result,141,"st_srp_weapon_minimum_damage","higher","st_srp_unit_percent",100);
         if(!expanded) { result.push({kind:"hint",text:LocalizationData.getName("st_srp_weapon_shift_hint")}); return result; }
         scalar(result,142,"st_srp_weapon_draw_time","lower","st_srp_unit_seconds");
         scalar(result,143,"st_srp_weapon_holster_time","lower","st_srp_unit_seconds");
         scalar(result,144,"st_srp_weapon_aim_zoom","neutral","st_srp_unit_times");
         scalar(result,145,"st_srp_weapon_stamina_damage","higher");
         scalar(result,152,"st_srp_weapon_hip_spread","lower","st_srp_unit_times");
         scalar(result,158,"st_srp_weapon_sway_cycle","neutral","st_srp_unit_seconds");
         result.push({kind:"heading",text:LocalizationData.getName("st_srp_weapon_recoil_heading")});
         recoil(result);
         scalar(result,153,"st_srp_weapon_spread_per_shot","lower");
         scalar(result,154,"st_srp_weapon_spread_recovery","higher");
         scalar(result,155,"st_srp_weapon_spread_growth","lower");
         scalar(result,159,"st_srp_weapon_spread_max","lower");
         scalar(result,156,"st_srp_weapon_sway_horizontal","lower");
         scalar(result,157,"st_srp_weapon_sway_vertical","lower");
         return result;
      }
   }
}
