package survarium.item
{
   import survarium.rooms.RoomsText;

   // Read-only weapon stats from weapon-details-ui-1.json. These are not item
   // modifications and must never be added to an item's bonus/modifier sums.
   public final class WeaponDetails
   {
      private var values:Object;
      private static const WORDS:Object = {
         base:["Weapon stats","Характеристики зброї","Характеристики оружия","Waffenwerte","Statystyki broni","Osobine oružja"],
         reload:["Reload","Перезаряджання","Перезарядка","Nachladen","Przeładowanie","Punjenje"],
         full:["Full damage up to","Повна шкода до","Полный урон до","Voller Schaden bis","Pełne obrażenia do","Puna šteta do"],
         minimum:["Minimum damage from","Мін. шкода від","Мин. урон от","Minimalschaden ab","Min. obrażenia od","Minimalna šteta od"],
         floor:["Minimum damage","Мінімальна шкода","Минимальный урон","Minimalschaden","Minimalne obrażenia","Minimalna šteta"],
         hint:["Hold Shift for details","Утримуйте Shift: подробиці","Удерживайте Shift: подробности","Shift halten: Details","Przytrzymaj Shift: szczegóły","Držite Shift za detalje"],
         draw:["Draw time","Час діставання","Время доставания","Ziehzeit","Czas wyciągnięcia","Vreme vađenja"],
         holster:["Holster time","Час прибирання","Время убирания","Wegsteckzeit","Czas schowania","Vreme odlaganja"],
         zoom:["Base aim zoom","Базове збільшення","Базовое увеличение","Basis-Zielzoom","Bazowe przybliżenie","Osnovno uvećanje"],
         stamina:["Stamina damage","Шкода витривалості","Урон выносливости","Ausdauerschaden","Obrażenia wytrzymałości","Šteta izdržljivosti"],
         hip:["Hip-fire spread","Розкид від стегна","Разброс от бедра","Hüftfeuer-Streuung","Rozrzut z biodra","Rasipanje sa boka"],
         reference:["Recoil, spread and sway","Відбій, розкид і хитання","Отдача, разброс и качание","Rückstoß, Streuung, Schwanken","Odrzut, rozrzut i kołysanie","Trzaj, rasipanje i njihanje"],
         up:["Initial upward kick","Початковий поштовх угору","Начальный толчок вверх","Anfänglicher Rückstoß oben","Początkowy odrzut w górę","Početni trzaj nagore"],
         upVariation:["Upward variability","Мінливість поштовху вгору","Изменчивость толчка вверх","Vertikale Variabilität","Zmienność odrzutu w górę","Promenljivost trzaja nagore"],
         side:["Initial sideways kick","Початковий бічний поштовх","Начальный боковой толчок","Anfänglicher Seitenstoß","Początkowy odrzut boczny","Početni bočni trzaj"],
         sideVariation:["Sideways variability","Мінливість бічного поштовху","Изменчивость бокового толчка","Seitliche Variabilität","Zmienność odrzutu bocznego","Promenljivost bočnog trzaja"],
         shot:["Spread added per shot","Розкид від пострілу","Разброс от выстрела","Streuung pro Schuss","Rozrzut na strzał","Rasipanje po hicu"],
         recovery:["Spread recovery rate","Швидкість відновлення точності","Скорость восстановления точности","Erholung der Präzision","Tempo odzyskania celności","Brzina oporavka preciznosti"],
         growth:["Spread growth rate","Швидкість зростання розкиду","Скорость роста разброса","Zunahme der Streuung","Tempo wzrostu rozrzutu","Brzina rasta rasipanja"],
         cap:["Maximum extra spread","Максимальний додатковий розкид","Максимальный доп. разброс","Maximale Zusatzstreuung","Maksymalny dodatkowy rozrzut","Maksimalno dodatno rasipanje"],
         swayH:["Horizontal sway","Горизонтальне хитання","Горизонтальное качание","Horizontales Schwanken","Kołysanie poziome","Vodoravno njihanje"],
         swayV:["Vertical sway","Вертикальне хитання","Вертикальное качание","Vertikales Schwanken","Kołysanie pionowe","Uspravno njihanje"],
         cycle:["Sway cycle","Період хитання","Период качания","Schwankungsperiode","Cykl kołysania","Ciklus njihanja"]
      };
      public function WeaponDetails(data:Object) { values = data; }
      private static function word(key:String) : String { return WORDS[key][RoomsText.language()]; }
      public static function read(item:Object) : WeaponDetails
      {
         if(item == null || !item.isWeapon) return null;
         var details:WeaponDetails = fromProperties(item.properties);
         if(details != null) details.applyWeaponModifiers(item.modifiers);
         return details;
      }
      private static function fromProperties(properties:Array) : WeaponDetails
      {
         var data:Object = {};
         for each(var prop:Object in properties)
         {
            if(prop.prop_id < 138 || prop.prop_id > 159) continue;
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
         var distance:Number = 0, reload:Number = 0;
         for each(var modifier:Object in modifiers)
         {
            if(modifier == null || !(modifier.props is Array)) continue;
            for each(var prop:Object in modifier.props)
            {
               if(prop.prop_id != 24 && prop.prop_id != 114) continue;
               var amount:Number = ModificationRolls.value(prop);
               if(!isFinite(amount)) continue;
               if(prop.prop_id == 24) distance += amount;
               else reload += amount;
            }
         }
         // item_config_cook's type-2 operator sums modifiers before applying
         // base * max(0, 1 + sum). Weapon reload UI uses the opposite sign.
         values[139] *= Math.max(0,1 + distance);
         values[140] *= Math.max(0,1 + distance);
         values[138] *= Math.max(0,1 - reload);
         // The current recoil upgrades target obsolete first/queue-shoot
         // fields, not the min/max fields loaded by 0x331840. Do not invent
         // an effect for those, or apply dispersion bonuses to unrelated
         // spread-growth/recovery parameters.
      }
      private function has(id:int) : Boolean { return values[id] !== undefined; }
      private static function number(n:Number) : String
      {
         var precision:Number = n > 0 && n < 0.1 ? 1000 : 100;
         return (Math.round(n * precision) / precision).toString();
      }
      private function scalar(result:Array, id:int, key:String, direction:String, suffix:String = "", multiplier:Number = 1) : void
      {
         if(!has(id)) return;
         var n:Number = values[id] * multiplier;
         result.push({kind:"stat",id:id,label:word(key),value:number(n) + suffix,
            comparison:direction == "neutral" ? {kind:"neutral"} : {kind:direction,value:Number(number(n))}});
      }
      private static function recoilNumber(n:Number) : String
      {
         return (Math.round(n * 1000) / 1000).toString();
      }
      private function kickRange(result:Array, id:int, key:String, low:Number, high:Number, comparison:Object) : void
      {
         result.push({kind:"stat",id:id,label:word(key),value:recoilNumber(low) + "…" + recoilNumber(high),comparison:comparison});
      }
      private function variability(result:Array, id:int, key:String, variance:Number) : void
      {
         var value:String = recoilNumber(Math.sqrt(variance));
         result.push({kind:"stat",id:id,label:word(key),value:value,comparison:{kind:"lower",value:Number(value)}});
      }
      private function recoil(result:Array) : void
      {
         // Raw initial shot impulse, before accumulated-kick scaling and PID
         // recovery. Each axis setting is an independent uniform draw.
         if(has(146) && has(147) && values[146] <= values[147])
         {
            kickRange(result,146,"up",values[146],values[147],{kind:"lower-range",
               low:Number(recoilNumber(values[146])),high:Number(recoilNumber(values[147]))});
            variability(result,147,"upVariation",Math.pow(values[147] - values[146],2) / 12);
         }
         if(has(148) && has(149) && has(150) && has(151) && values[148] <= values[149] && values[150] <= values[151])
         {
            // The engine subtracts left from right. A signed range has no
            // universal better direction; compare its variability separately.
            kickRange(result,148,"side",values[150] - values[149],values[151] - values[148],{kind:"neutral"});
            variability(result,149,"sideVariation",(Math.pow(values[149] - values[148],2) + Math.pow(values[151] - values[150],2)) / 12);
         }
      }
      public function rows(expanded:Boolean) : Array
      {
         var result:Array = [{kind:"heading",text:word("base")}];
         scalar(result,138,"reload","lower","s");
         scalar(result,139,"full","higher","m");
         scalar(result,140,"minimum","higher","m");
         scalar(result,141,"floor","higher","%",100);
         if(!expanded) { result.push({kind:"hint",text:word("hint")}); return result; }
         scalar(result,142,"draw","lower","s");
         scalar(result,143,"holster","lower","s");
         scalar(result,144,"zoom","neutral","x");
         scalar(result,145,"stamina","higher");
         scalar(result,152,"hip","lower","x");
         scalar(result,158,"cycle","neutral","s");
         result.push({kind:"heading",text:word("reference")});
         recoil(result);
         scalar(result,153,"shot","lower");
         scalar(result,154,"recovery","higher");
         scalar(result,155,"growth","lower");
         scalar(result,159,"cap","lower");
         scalar(result,156,"swayH","lower");
         scalar(result,157,"swayV","lower");
         return result;
      }
   }
}
