package survarium.item
{
   import survarium.constants.ModificationState;
   import survarium.data.ItemData;

   // The content build copies the weapon's own movement parameter into property
   // 105. Present it as a built-in bonus, using the normal modifier formatting.
   public final class WeaponMovement
   {
      public static function expose(item:ItemData) : void
      {
         if(!item.isWeapon || item.properties == null) return;
         var properties:Array = [];
         var defaults:Array = item.modifiersDefault.concat();
         for each(var prop:Object in item.properties)
         {
            if(prop.prop_id != 105)
            {
               properties.push(prop);
               continue;
            }
            var amount:Number = ModificationRolls.value(prop);
            if(isFinite(amount) && amount != 0)
            {
               defaults.push({state:ModificationState.MODIFICATION, isDefault:true,
                  icon:11, props:[{prop_id:105, prop_value:amount}]});
            }
         }
         // Descriptor arrays are shared by shop/inventory instances. Keep them
         // intact so creating another card cannot duplicate or lose this line.
         item.properties = properties;
         item.modifiersDefault = defaults;
         item.modifiers = defaults;
      }
   }
}
