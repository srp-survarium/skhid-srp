package survarium.constants
{
   // Where an item is (ItemData.descriptor, and the source/target slot of a buy or a move). Below
   // INVENTORY it is the profile slot the item is worn in (ProfileSlot); NONE is also ProfileSlot.HEAD.
   public class ItemPlace
   {
      
      // not placed (the ItemData default; a bundle's contents)
      public static const NONE:int = 0;
      
      public static const INVENTORY:int = 100;
      
      public static const SHOP:int = 200;
      
      // a shop bundle
      public static const BUNDLE:int = 300;
      
      public function ItemPlace()
      {
         super();
      }
   }
}
