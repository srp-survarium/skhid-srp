package survarium.item
{
   import survarium.Util;
   import survarium.constants.ModifierType;
   import survarium.constants.PropertyDirection;
   import survarium.data.PropertiesDescriptor;

   // A bonus at the best value its modifications roll shows gold, not green; the reroll window shows
   // a bonus's range on hover (docs/notes/max-bonus.md).
   public class ModificationRolls
   {

      // By prop_id: the values the modifications that roll give the property, weakest first, as
      // gameplay/items/modifications/modifications.options has them (ui_desc/props_list; the types
      // with a drop weight that some item takes). The movie gets no modification id, only
      // {prop_id, prop_value}; the types that share a property roll the same values. srp's
      // modifications::tests::the_lobby_knows_the_values_each_bonus_rolls checks it against the
      // data and prints the table when it differs.
      private static const ROLLS:Object = {
         1:[0.01,0.02,0.03,0.04,0.05], // damage (2000..2004)
         2:[-0.02,-0.04,-0.06,-0.08,-0.1], // dispersion (2020..)
         3:[0.01,0.02,0.03,0.04,0.05], // armour piercing (2110..)
         4:[0.02,0.04,0.06,0.08,0.1], // rate of fire (2030..)
         6:[2,3,4,5,8], // armour (1000..1004)
         8:[5,10,15,20,25], // anomaly armour, the mask's filters (1020.., 1180..)
         11:[5,10,15,20,25], // oxygen (1190..)
         15:[-0.05,-0.08,-0.1,-0.12,-0.15], // weight (1060.., 2060..)
         24:[0.05,0.1,0.15,0.2,0.25], // damage distance (2010..)
         101:[-0.02,-0.04,-0.06,-0.08,-0.1], // accuracy (1070..)
         102:[-0.02,-0.04,-0.06,-0.08,-0.1], // aim time (1080.., 2070..)
         103:[0.05,0.08,0.1,0.12,0.15], // health regeneration (1090..)
         104:[0.02,0.04,0.06,0.08,0.1], // stamina regeneration (1100..)
         105:[0.01,0.02,0.03,0.04,0.05], // movement speed (1110..)
         106:[1,2,3,4,5], // carried weight (1120..)
         108:[-0.05,-0.1,-0.15,-0.2,-0.25], // artefact search (1130..)
         114:[0.05,0.08,0.1,0.12,0.15], // reload (1140.., 2040..)
         115:[-0.02,-0.04,-0.06,-0.08,-0.1], // recoil (1150.., 2080..)
         116:[-0.02,-0.04,-0.06,-0.08,-0.1], // stamina spending (1160..)
         119:[-0.05,-0.08,-0.1,-0.12,-0.15], // chamber a round (2050..)
         123:[0.05,0.1,0.15,0.2,0.25] // reliability (1200.., 2090..)
      };

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

      // Whether the value of a bonus reaches the best its property rolls: never for one that does
      // not roll (the premium items' silver, experience and reputation bonuses, pockets, slots).
      public static function isBest(propId:uint, value:Number) : Boolean
      {
         var rolls:Array = ROLLS[propId] as Array;
         if(!rolls)
         {
            return false;
         }
         var best:Number = rolls[rolls.length - 1];
         return value * best > 0 && Math.abs(value) >= Math.abs(best) - EPSILON;
      }

      // The style of a bonus line's value: red for a loss, gold at the best roll, green below it.
      public static function style(description:Object, propId:uint, value:Number) : String
      {
         var good:Boolean = description.direction == PropertyDirection.LOWER_IS_BETTER && value < 0 || description.direction == PropertyDirection.HIGHER_IS_BETTER && value > 0;
         if(!good)
         {
            return "red";
         }
         return isBest(propId,value) ? "gold" : "green";
      }

      // A value as the cards show it: "+5%", or "+8" with the property's postfix.
      public static function format(description:Object, value:Number) : String
      {
         var add:Boolean = description.modifier_type == ModifierType.ADD;
         var postfix:String = add ? description.postfix : "%";
         var shown:Number = Math.round((add ? value : value * 100) * 100) / 100;
         return (value >= 0 ? "+" : "") + shown + (postfix ? postfix : "");
      }

      // A modification's range, one line: "Damage: +1%  +2%  [+3%]  +4%  +5%". The others grey,
      // the rolled one white in brackets (plain ASCII: the lobby's font lacks symbols), the best
      // gold, in brackets too when it is the rolled one. "" for one that does not roll.
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
         var best:Boolean;
         for each(prop in modifier.props)
         {
            rolls = ROLLS[prop.prop_id] as Array;
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
               best = i == rolls.length - 1;
               if(!marked && Math.abs(rolls[i] - rolled) < EPSILON)
               {
                  marked = true;
                  parts.push(Util.styledChar("[" + text + "]",best ? "gold" : "white"));
               }
               else
               {
                  parts.push(Util.styledChar(text,best ? "gold" : "gray"));
               }
               i++;
            }
            tip += (tip ? "   " : "") + description.prop_name + ": " + parts.join("  ");
         }
         return tip;
      }
   }
}
