package survarium.item
{
   import survarium.data.ItemDescriptor;
   import survarium.data.LocalizationData;
   import survarium.data.PropertiesDescriptor;
   import survarium.player.MatchEffects;

   // Read-only weapon stats from weapon-details-ui-1.json. These are not item
   // modifications and must never be added to an item's bonus/modifier sums.
   public final class WeaponDetails
   {
      // The detail properties weapon-details-ui-1 gives a weapon's card.
      private static const FIRST:int = 138, LAST:int = 160;
      // Chambering time, on bolt and pump weapons only (parameters/chamber_a_round_time).
      private static const CHAMBER:int = 160;
      private static const TACTICAL:int = 162;
      private var values:Object;
      // In a match's view (Alt): the weapon's own base values, before its modifications, the kit
      // and the skills, for the rows that change ("base -> in a match"); null in the card's own.
      private var base:Object = null;
      // The rounds it fires (the dictionary's pairs), by dict id: {damage and arp (the round's card
      // lines 21 and 22 / 100: k_damage x pellets, k_arp), speed (163, ammo-cards-1), pellets (165),
      // each NaN without its line; pierce (PierceLine.forWeapon, null without one)}.
      private var rounds:Array = [];
      private static const DAMAGE:int = 1, ARP:int = 3, ROUND_DAMAGE:int = 21, ROUND_ARP:int = 22, SPEED:int = 163, PELLETS:int = 165;
      // Its own card lines 1 (damage x100) and 3 (armour piercing x100), NaN without them.
      private var damage:Number = NaN, arp:Number = NaN;
      public function WeaponDetails(data:Object) { values = data; }
      // The weapon's stats with its modifications; with effects (held Alt), as a match has them
      // with the kit and the skills too.
      public static function read(item:Object, effects:MatchEffects = null) : WeaponDetails
      {
         if(item == null || !item.isWeapon) return null;
         var details:WeaponDetails = fromProperties(item.properties);
         if(details == null) return null;
         details.rounds = roundsOf(item.dictId);
         details.damage = line(item.properties,DAMAGE);
         details.arp = line(item.properties,ARP);
         if(effects != null) details.base = copy(details.values);
         details.applyWeaponModifiers(item.modifiers);
         if(effects != null) for(var id:String in details.values) details.values[id] = effects.stat(item,int(id),details.values[id]);
         return details;
      }
      // A card line's number, NaN without it.
      private static function line(properties:Array, id:int) : Number
      {
         for each(var prop:Object in properties)
         {
            if(prop.prop_id != id) continue;
            var n:Number = ModificationRolls.value(prop);
            if(isFinite(n)) return n;
         }
         return NaN;
      }
      private static function roundsOf(weapon:int) : Array
      {
         var items:ItemDescriptor = ItemDescriptor.getItemDescriptor("InventoryItems");
         var out:Array = [];
         for each(var id:int in ItemFactory.getList(weapon))
         {
            var props:Array = items.getItemDescription(id).item_properties;
            var round:Object = {damage:line(props,ROUND_DAMAGE) / 100,arp:line(props,ROUND_ARP) / 100,
               speed:line(props,SPEED),pellets:line(props,PELLETS),pierce:PierceLine.forWeapon(props,weapon)};
            if(!isNaN(round.damage) || !isNaN(round.arp) || !isNaN(round.speed) || round.pierce != null) out.push(round);
         }
         return out;
      }
      // The native bars of damage (card line 1) and armour piercing (3) take the weapon's best round:
      // its value times the highest of its rounds' k_damage x pellets (21) or k_arp (22), as a match
      // multiplies them (bullet::bullet 0x32BA60); a shotgun of both kinds its best slug's (its
      // detail rows give both kinds; against another gun the slug is what compares); 1 for another
      // line or a weapon without rounds.
      public static function bestRoundFactor(item:Object, id:int) : Number
      {
         if(item == null || !item.isWeapon || id != DAMAGE && id != ARP) return 1;
         var rounds:Array = roundsOf(item.dictId), kinds:Object = kindsOf(rounds);
         if(kinds != null) rounds = kinds.slug;
         var best:Number = NaN;
         for each(var round:Object in rounds)
         {
            var k:Number = id == DAMAGE ? round.damage : round.arp;
            if(!isNaN(k) && (isNaN(best) || k > best)) best = k;
         }
         return isNaN(best) ? 1 : best;
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
            if((prop.prop_id < FIRST || prop.prop_id > LAST) && prop.prop_id != TACTICAL) continue;
            var n:Number = ModificationRolls.value(prop);
            if(isFinite(n) && n >= 0) data[prop.prop_id] = n;
         }
         // Old content keeps the old distance row, with no invented details.
         if(data[138] == undefined || data[139] == undefined || data[140] == undefined || data[141] == undefined) return null;
         // The native dispersion loader (0x330FE3) defaults the optional
         // from_the_hip_multiplier to 1, including Kiparis and AK-74M.
         if(data[152] == undefined) data[152] = 1;
         // 162 transports empty/tactical clip duration, not a second timer.
         // Native state 314 scales its tactical hands clip to reload_time.
         if(data[TACTICAL] !== undefined && data[TACTICAL] > 0)
         {
            var ratio:Number = data[TACTICAL];
            data[TACTICAL] = data[138];
            data[138] *= ratio;
         }
         else delete data[TACTICAL];
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
         if(has(TACTICAL)) values[TACTICAL] *= Math.max(0,1 - sums[114]);
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
      // In a match's view, a changed value's verdict for the player (MatchEffects.BETTER or WORSE,
      // by the row's direction: "lower", "higher"); null where it did not change or the row has
      // no better direction.
      private function verdictOf(before:Number, after:Number, direction:String) : String
      {
         if(base == null || !MatchEffects.changed(before,after) || direction != "lower" && direction != "higher") return null;
         return MatchEffects.verdict(before,after,direction == "higher");
      }
      // `unit` is the dictionary text of the value with its unit (%1$s the number).
      private function scalar(result:Array, id:int, key:String, direction:String, unit:String = null, multiplier:Number = 1) : void
      {
         if(!has(id)) return;
         var n:Number = values[id] * multiplier;
         var before:Number = base == null ? n : base[id] * multiplier;
         var text:String = shown(before,n,decimals(n));
         result.push({kind:"stat",id:id,label:LocalizationData.getName(key),
            value:unit == null ? text : LocalizationData.format(unit,text),verdict:verdictOf(before,n,direction),
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
         var before:Number = base == null ? values[CHAMBER] : base[CHAMBER];
         var value:String = LocalizationData.format("st_srp_unit_seconds",shown(before,values[CHAMBER],decimals(values[CHAMBER])));
         result.push({kind:"stat",id:CHAMBER,label:label,value:value,verdict:verdictOf(before,values[CHAMBER],"lower"),
            comparison:{kind:"lower",value:Number(modified)}});
      }
      // The stats a round changes: [the row's id, its label, has (which rounds have it), text, score
      // (higher is better)]. Damage and armour piercing as the native bars show them with that
      // round (the weapon's own card number x the round's k, whole), bullet speed the round's, the
      // material and damage kept PierceLine's (the harder material, then the more damage kept).
      private function roundStats() : Array
      {
         var names:PropertiesDescriptor = PropertiesDescriptor.getPropertiesDescriptor("properties");
         var whole:Function = function(n:Number):String { return MatchEffects.fixed(Math.round(n),0); };
         return [
            [DAMAGE,names.getPropertyDescription(DAMAGE).prop_name,
               function(round:Object):Boolean { return !isNaN(damage * round.damage); },
               function(round:Object):String { return whole(damage * round.damage); },
               function(round:Object):Number { return damage * round.damage; }],
            [ARP,names.getPropertyDescription(ARP).prop_name,
               function(round:Object):Boolean { return !isNaN(arp * round.arp); },
               function(round:Object):String { return whole(arp * round.arp); },
               function(round:Object):Number { return arp * round.arp; }],
            [SPEED,LocalizationData.getName("st_srp_inv_bullet_speed"),
               function(round:Object):Boolean { return !isNaN(round.speed); },
               function(round:Object):String { return LocalizationData.format("st_srp_unit_mps",MatchEffects.fixed(round.speed,0)); },
               function(round:Object):Number { return round.speed; }],
            [PierceLine.ID,LocalizationData.getName("st_srp_inv_material_pierce"),
               function(round:Object):Boolean { return round.pierce != null; },
               function(round:Object):String { return PierceLine.text(round.pierce); },
               function(round:Object):Number { return PierceLine.score(round.pierce); }]];
      }
      // Of `rounds`, the one `stat` scores best (stat[4], higher is better) among those it has (stat[2]); null for none.
      private static function bestOf(rounds:Array, stat:Array) : Object
      {
         var best:Object = null;
         for each(var round:Object in rounds) if(stat[2](round) && (best == null || stat[4](round) > stat[4](best))) best = round;
         return best;
      }
      // The best of `rounds`' scores at `stat`, NaN for none.
      private static function scoreOf(rounds:Array, stat:Array) : Number
      {
         var best:Object = bestOf(rounds,stat);
         return best == null ? NaN : stat[4](best);
      }
      // A row of `round`'s value of `stat`, compared with the score `against` (NaN: nothing to compare
      // with, neutral); a dash, neutral, without a round.
      private static function statRow(result:Array, id:String, label:String, stat:Array, round:Object, against:Number,
         child:Boolean = false) : void
      {
         result.push(round == null ? {kind:"stat",id:id,label:label,value:"–",child:child,comparison:{kind:"neutral"}}
            : {kind:"stat",id:id,label:label,value:stat[3](round),child:child,
               comparison:isNaN(against) ? {kind:"neutral"} : {kind:"higher",value:stat[4](round),against:against}});
      }
      // The card's row of a stat a round changes (roundStats): its best round's, against the other
      // card's best (a shotgun's slug); none when neither card has it. A shotgun of both kinds of
      // round (kinds) shown alone or beside another such (split): the stat's label alone over its
      // best Buckshot and best Slug, each against the other's same kind. Beside any other gun a
      // shotgun is a gun of slugs: its best slug's rows, no kinds shown.
      private function roundStat(result:Array, other:WeaponDetails, stat:Array, split:Boolean) : void
      {
         var mine:Object = kinds(), theirs:Object = other == null ? null : other.kinds();
         var best:Number = other == null ? NaN : scoreOf(theirs == null ? other.rounds : theirs.slug,stat);
         if(!split)
         {
            var own:Object = bestOf(mine == null ? rounds : mine.slug,stat);
            if(own != null || !isNaN(best)) statRow(result,String(stat[0]),stat[1],stat,own,best);
            return;
         }
         result.push({kind:"stat",id:String(stat[0]),label:stat[1],value:"",comparison:{kind:"neutral"}});
         statRow(result,stat[0] + "/buckshot","• " + LocalizationData.getName("st_srp_ammo_buckshot"),stat,bestOf(mine.buckshot,stat),
            theirs == null ? NaN : scoreOf(theirs.buckshot,stat),true);
         statRow(result,stat[0] + "/slug","• " + LocalizationData.getName("st_srp_ammo_slug"),stat,bestOf(mine.slug,stat),best,true);
      }
      // A shotgun's rounds by kind, {buckshot: its rounds of more than one pellet (card line 165),
      // slug: the others}, when it has both kinds; null for every other weapon.
      private function kinds() : Object
      {
         return kindsOf(rounds);
      }
      private static function kindsOf(rounds:Array) : Object
      {
         var buckshot:Array = rounds.filter(function(round:Object, ... rest):Boolean { return round.pellets > 1; });
         var slug:Array = rounds.filter(function(round:Object, ... rest):Boolean { return !(round.pellets > 1); });
         return buckshot.length && slug.length ? {buckshot:buckshot,slug:slug} : null;
      }
      private function tactical(result:Array, other:WeaponDetails) : void
      {
         if(has(TACTICAL)) scalar(result,TACTICAL,"st_srp_weapon_reload_tactical","lower","st_srp_unit_seconds");
         else if(other != null && other.has(TACTICAL))
            result.push({kind:"stat",id:TACTICAL,label:LocalizationData.getName("st_srp_weapon_reload_tactical"),value:"-",comparison:{kind:"neutral"}});
      }
      private static function recoilNumber(n:Number) : String
      {
         return MatchEffects.fixed(n,3);
      }
      // A kick range "low…high"; in a match's view "base -> in a match" where it changed, each
      // end to the decimals at which it differs; a smaller kick (both ends nearer 0) is better.
      private function kickRange(result:Array, id:int, key:String, low:Array, high:Array, comparison:Object) : void
      {
         var value:String = recoilNumber(low[1]) + "…" + recoilNumber(high[1]);
         var verdict:String = null;
         if(base != null && (MatchEffects.changed(low[0],low[1]) || MatchEffects.changed(high[0],high[1])))
         {
            var lowDigits:int = MatchEffects.decimalsFor(low[0],low[1],2), highDigits:int = MatchEffects.decimalsFor(high[0],high[1],2);
            value = MatchEffects.fixed(low[0],lowDigits) + "…" + MatchEffects.fixed(high[0],highDigits) + MatchEffects.ARROW +
               MatchEffects.fixed(low[1],lowDigits) + "…" + MatchEffects.fixed(high[1],highDigits);
            verdict = MatchEffects.verdict(Math.abs(low[0]) + Math.abs(high[0]),Math.abs(low[1]) + Math.abs(high[1]),false);
         }
         result.push({kind:"stat",id:id,label:LocalizationData.getName(key),value:value,verdict:verdict,comparison:comparison});
      }
      // `variance`: [base, shown].
      private function variability(result:Array, id:int, key:String, variance:Array) : void
      {
         var value:Number = Math.sqrt(variance[1]);
         result.push({kind:"stat",id:id,label:LocalizationData.getName(key),value:shown(Math.sqrt(variance[0]),value,3),
            verdict:verdictOf(Math.sqrt(variance[0]),value,"lower"),
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
         scalar(result,138,has(TACTICAL) ? "st_srp_weapon_reload_empty" : "st_srp_weapon_reload","lower","st_srp_unit_seconds");
         tactical(result,other);
         chamber(result,other);
         scalar(result,139,"st_srp_weapon_full_damage","higher","st_srp_unit_meters");
         scalar(result,140,"st_srp_weapon_minimum_damage_from","higher","st_srp_unit_meters");
         scalar(result,141,"st_srp_weapon_minimum_damage","higher","st_srp_unit_percent",100);
         // The best round's bullet speed and material (its damage and armour piercing are the
         // native bars', ItemCompare); a shotgun of both kinds, alone or beside another, all four by
         // kind.
         var stats:Array = roundStats();
         var split:Boolean = kinds() != null && (other == null || other.kinds() != null);
         for(var at:int = split ? 0 : 2; at < stats.length; at++) roundStat(result,other,stats[at],split);
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
