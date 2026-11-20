package survarium.item
{
   import survarium.Util;
   import survarium.constants.ModificationState;
   import survarium.constants.PropertyDirection;
   import survarium.data.ItemData;
   import survarium.data.PropertiesDescriptor;
   import survarium.player.MatchEffects;

   // The content build copies the weapon's own movement parameter into property
   // 105. Present it as a built-in bonus, using the normal modifier formatting.
   public final class WeaponMovement
   {
      private static const PROP_ID:uint = 105;

      public static function expose(item:ItemData) : void
      {
         if(!item.isWeapon || item.properties == null) return;
         var properties:Array = [];
         var defaults:Array = item.modifiersDefault.concat();
         for each(var prop:Object in item.properties)
         {
            if(prop.prop_id != PROP_ID)
            {
               properties.push(prop);
               continue;
            }
            var amount:Number = ModificationRolls.value(prop);
            if(isFinite(amount) && amount != 0)
            {
               defaults.push({state:ModificationState.MODIFICATION, isDefault:true,
                  icon:11, props:[{prop_id:PROP_ID, prop_value:amount}]});
            }
         }
         // Descriptor arrays are shared by shop/inventory instances. Keep them
         // intact so creating another card cannot duplicate or lose this line.
         item.properties = properties;
         item.modifiersDefault = defaults;
         item.modifiers = defaults;
      }

      // A card's bonus line as a match has it (held Alt): the weapon's own movement line (expose's)
      // that Bear Arms cancels (MatchEffects.weaponMovement) reads "Movement speed: -5% -> 0%", the
      // current value in the change's colour (docs/notes/alt-weapon-stats.md). Any other line, and
      // this one when nothing changes it, keeps its own text.
      public static function inMatch(item:Object, modifier:Object, effects:MatchEffects) : String
      {
         var props:Array = modifier.props as Array;
         if(!item.isWeapon || !modifier.isDefault || props == null || props.length != 1 || props[0].prop_id != PROP_ID)
            return modifier.textValue;
         var before:Number = ModificationRolls.value(props[0]);
         var after:Number = effects.weaponMovement(before);
         var description:Object = PropertiesDescriptor.getPropertiesDescriptor("properties").getPropertyDescription(PROP_ID);
         if(description == null || !MatchEffects.changed(before,after)) return modifier.textValue;
         var current:String = percent(after);
         // Ctrl (neutral colours) leaves the change neutral, as the stats' changes.
         if(!MainScene.isCtrl)
         {
            var better:Boolean = MatchEffects.verdict(before,after,description.direction == PropertyDirection.HIGHER_IS_BETTER) == MatchEffects.BETTER;
            current = Util.styledChar(current,better ? "green" : "red");
         }
         // The line break fillModifiersDescription ends a line with, so the row keeps its height.
         var end:String = String(modifier.textValue).slice(-1) == "\n" ? "\n" : "";
         return description.prop_name + ": " + percent(before) + MatchEffects.ARROW + current + end;
      }

      // A fraction as the bonus line shows it: "-5%", "0%", "+3%".
      private static function percent(value:Number) : String
      {
         return (value > 0 ? "+" : "") + MatchEffects.fixed(value * 100,2) + "%";
      }
   }
}
