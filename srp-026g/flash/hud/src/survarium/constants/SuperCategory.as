package survarium.constants
{
   // The group of item categories an item is shown in (ItemFactory.getCategory, ItemData.superCategory):
   // the shop's tabs, filter_all / filter_equipment / filter_weapons / filter_ammo / filter_items /
   // filter_upgrades (FiltersWindow.focusCategory; a tab's index is its value).
   public class SuperCategory
   {
      
      // the "all" tab (no item is in it)
      public static const ALL:int = 0;
      
      // armour: ItemCategory 1..7
      public static const EQUIPMENT:int = 1;
      
      // ItemCategory 8 and 13..17
      public static const WEAPONS:int = 2;
      
      // ItemCategory 9 and 18..21
      public static const AMMO:int = 3;
      
      // everything else: medkits, traps, grenades, artefacts, scopes
      public static const ITEMS:int = 4;
      
      // camouflages, decals, vouchers and money (ItemCategory 25..29)
      public static const UPGRADES:int = 5;
      
      // focusCategory(-1): every tab enabled, the last one shown again
      public static const NO_FOCUS:int = -1;
      
      public function SuperCategory()
      {
         super();
      }
   }
}
