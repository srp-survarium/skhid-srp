package survarium.constants
{
   // What an item config lacking these keys gets (ItemFactory).
   public class ItemDefaults
   {
      
      // camo_available
      public static const CAMO_AVAILABLE:int = 1;
      
      // decal_available
      public static const DECAL_AVAILABLE:int = 0;
      
      // upgrades_available: three decal slots and the camouflage
      public static const UPGRADES_AVAILABLE:int = 4;
      
      public function ItemDefaults()
      {
         super();
      }
   }
}
