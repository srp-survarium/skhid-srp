package survarium.player
{
   // What each of the kit's items adds, one entry an item that changes it
   // ({name, value}; the profile's, from the item's modifications: built-in,
   // rolled and defects): movement speed (prop 105: the armour's and the
   // pockets', and the first weapon's own movement parameter) and carry limit
   // (prop 106, kg: the armour's and the pockets'). The totals are summed
   // from these lists.
   public final class KitItems
   {
      // {name, value} of the armour and the pockets.
      public var speed:Array = [];
      // {name, value} of the first weapon (the one in hand), or null.
      public var weapon:Object = null;
      public var limit:Array = [];
   }
}
