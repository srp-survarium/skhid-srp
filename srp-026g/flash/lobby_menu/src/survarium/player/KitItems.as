package survarium.player
{
   // What each of the kit's items adds, one entry an item that changes it
   // ({name, value}; the profile's, from the item's modifications: built-in,
   // rolled and defects): movement speed (prop 105: the armour's and the
   // pockets', and the first weapon's own movement parameter) and carry limit
   // (prop 106, kg: the armour's and the pockets'). The totals are summed
   // from these lists. Each entry also has its item's category (items_dict's
   // item_category), for its row's group icon.
   public final class KitItems
   {
      // {name, value, category} of the armour and the pockets.
      public var speed:Array = [];
      // {name, value, category, killIcon} of the first weapon (the one in hand), or null;
      // killIcon its ui_desc/combat_log_icon (-1 if none).
      public var weapon:Object = null;
      public var limit:Array = [];
      // {name, value, category}: the armour's and the pockets' energy costs (prop 116).
      public var stamina:Array = [];
   }
}
