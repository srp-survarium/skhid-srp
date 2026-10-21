package survarium.constants
{
   // An item's rank (rarity_level: its base rarity plus its modifications, at most MAX; docs/notes/items.md).
   public class ItemRank
   {
      
      public static const ONE:int = 1;
      
      public static const TWO:int = 2;
      
      // also the most modifications an item takes (st_craft_message_modifers_count)
      public static const MAX:int = 3;
      
      public function ItemRank()
      {
         super();
      }
   }
}
