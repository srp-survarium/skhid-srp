package survarium.item
{
   import survarium.Util;
   import survarium.constants.ModifierType;
   import survarium.constants.PropertyDirection;
   import survarium.constants.QuickSlotSize;
   import survarium.data.ItemData;
   import survarium.data.ModificationValues;
   import survarium.data.PropertiesDescriptor;

   // A bonus at the best value its modifications roll shows gold, not green; the reroll window shows
   // a bonus's range on hover (docs/notes/max-bonus.md).
   public class ModificationRolls
   {

      // By prop_id: the modifications that roll the property, [the first of its five ids, the card's
      // scale], the values (rolls) the game's own (ModificationValues, made by the build from
      // scripts/content-edit/modifications.tsv, the one place they are written), each times the
      // scale, as its card line shows it (ui_desc/props_list: armour x100, a reload or a reliability
      // of a weapon the opposite sign). The movie gets no modification id, only {prop_id,
      // prop_value}; the types that share a property (weight, aim time, reliability, recoil, reload,
      // anomaly armour) roll the same values, so one run of ids stands for them. srp's
      // modifications::tests::the_lobby_knows_the_values_each_bonus_rolls checks it against the
      // data and prints the table when it differs.
      private static const ROLLS:Object = {
         1:[2000,1], // damage
         2:[2020,1], // dispersion
         3:[2110,1], // armour piercing
         4:[2030,1], // rate of fire
         6:[1000,100], // armour
         8:[1020,100], // anomaly armour, the mask's filters (1180..)
         11:[1190,1], // oxygen
         15:[1060,1], // weight (2060..)
         24:[2010,1], // damage distance
         101:[1070,1], // accuracy
         102:[1080,1], // aim time (2070..)
         103:[1090,1], // health regeneration
         104:[1100,1], // stamina regeneration
         105:[1110,1], // movement speed
         106:[1120,1], // carried weight
         108:[1130,1], // artefact search
         114:[1140,1], // reload (2040.., the opposite sign)
         115:[1150,1], // recoil (2080..)
         116:[1160,1], // stamina spending
         119:[2050,1], // chamber a round
         123:[1200,-1] // reliability (2090..)
      };

      // The values a property rolls, weakest first (ROLLS); null for a property that does not roll.
      private static function rollsOf(propId:uint) : Array
      {
         var row:Array = ROLLS[propId] as Array;
         if(!row)
         {
            return null;
         }
         var values:Array = [];
         for(var i:int = 0; i < 5; i++)
         {
            // Rounded: 0.15 x 100 is 15.000000000000002.
            values.push(Math.round(Number(ModificationValues.value[String(int(row[0]) + i)]) * Number(row[1]) * 1000000) / 1000000);
         }
         return values;
      }

      private static const EPSILON:Number = 0.000001;

      public function ModificationRolls()
      {
         super();
      }

      // A {prop_id, prop_value} entry's value, read as the cards read it.
      public static function value(prop:Object) : Number
      {
         var number:Number = Number(prop.prop_value);
         if(isNaN(number))
         {
            number = Util.stringToNumber(String(prop.prop_value));
         }
         return number;
      }

      // Whether the value of a bonus reaches the best its property rolls, or, for a property that
      // does not roll, the premium bonus that gives it (isPremium); never for the rest (pockets,
      // slots, ammo bags).
      public static function isBest(propId:uint, value:Number) : Boolean
      {
         var rolls:Array = rollsOf(propId);
         if(!rolls)
         {
            return isPremium(propId,value);
         }
         return reaches(value,rolls[rolls.length - 1]);
      }

      // Whether a bonus is a premium one, the most there is (owner 2026-10-22): a weapon's
      // silver, the premium armour's experience and reputation (ItemFactory.BONUS_UI, by the same
      // property and value). They never roll; srp's
      // modifications::tests::the_premium_bonuses_are_the_most_their_property_gives checks that
      // no modification gives the property more, nor rolls it.
      public static function isPremium(propId:uint, value:Number) : Boolean
      {
         var row:Array;
         for each(row in ItemFactory.BONUS_UI)
         {
            if(uint(row[2]) == propId && reaches(value,Number(ItemFactory.bonusValue(row))))
            {
               return true;
            }
         }
         return false;
      }

      private static function reaches(value:Number, best:Number) : Boolean
      {
         return value * best > 0 && Math.abs(value) >= Math.abs(best) - EPSILON;
      }

      // The style of a bonus line's value: red for a loss, gold at the best roll, green below it.
      public static function style(description:Object, propId:uint, value:Number) : String
      {
         return styleOf(description,value,isBest(propId,value));
      }

      // The style of a bonus line on an item's card (its modifications' lines and their sums, which
      // Alt keeps): a booster's bonus is gold, any other by its value (style). A booster's bonuses
      // are all built-in (its config's default_modifications: no roll, no defect, nothing to
      // better), so each is the most it gives (the owner, 2026-11-06), whether its property rolls
      // on other items (the regeneration +15 %) or not (the artefact cooldown -20 %).
      public static function styleOn(item:ItemData, description:Object, propId:uint, value:Number) : String
      {
         return styleOf(description,value,isBooster(item) || isBest(propId,value));
      }

      // A booster fills a whole pocket (fills_whole_pocket), as no other item does.
      private static function isBooster(item:ItemData) : Boolean
      {
         return item.quickSlotItemSize == QuickSlotSize.WHOLE;
      }

      private static function styleOf(description:Object, value:Number, best:Boolean) : String
      {
         var good:Boolean = description.direction == PropertyDirection.LOWER_IS_BETTER && value < 0 || description.direction == PropertyDirection.HIGHER_IS_BETTER && value > 0;
         if(!good)
         {
            return "red";
         }
         return best ? "gold" : "green";
      }

      // A value as the cards show it: "+5%", or "+8" with the property's postfix.
      public static function format(description:Object, value:Number) : String
      {
         var add:Boolean = description.modifier_type == ModifierType.ADD;
         var postfix:String = add ? description.postfix : "%";
         var shown:Number = Math.round((add ? value : value * 100) * 100) / 100;
         return (value >= 0 ? "+" : "") + shown + (postfix ? postfix : "");
      }

      // A bonus line of the reroll or craft window (ModificatorListRenderer) shows the values its
      // modification rolls on hover, the rolled one marked (rangeTip; the renderer's tip shows
      // `name`).
      public static function withRange(option:Object, modifier:Object) : Object
      {
         var tip:String = rangeTip(modifier);
         if(tip)
         {
            option.name = tip;
            option.rollRange = true;
         }
         return option;
      }
      
      // A modification's range, one line: "Damage: +1%  +2%  [+3%]  +4%  +5%". The others grey,
      // the rolled one white in brackets (plain ASCII: the lobby's font lacks symbols); no gold
      // here (owner 2026-10-22: it confused). "" for one that does not roll.
      public static function rangeTip(modifier:Object) : String
      {
         var descriptor:PropertiesDescriptor = PropertiesDescriptor.getPropertiesDescriptor("properties");
         if(!modifier || !modifier.props || !descriptor)
         {
            return "";
         }
         var tip:String = "";
         var prop:Object;
         var rolls:Array;
         var description:Object;
         var rolled:Number;
         var parts:Array;
         var marked:Boolean;
         var i:uint;
         var text:String;
         for each(prop in modifier.props)
         {
            rolls = rollsOf(uint(prop.prop_id));
            if(!rolls)
            {
               continue;
            }
            description = descriptor.getPropertyDescription(prop.prop_id);
            if(!description || description.prop_name == undefined)
            {
               continue;
            }
            rolled = value(prop);
            parts = [];
            marked = false;
            i = 0;
            while(i < rolls.length)
            {
               text = format(description,rolls[i]);
               if(!marked && Math.abs(rolls[i] - rolled) < EPSILON)
               {
                  marked = true;
                  parts.push(Util.styledChar("[" + text + "]","white"));
               }
               else
               {
                  parts.push(Util.styledChar(text,"gray"));
               }
               i++;
            }
            tip += (tip ? "   " : "") + description.prop_name + ": " + parts.join("  ");
         }
         return tip;
      }
   }
}
