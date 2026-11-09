package survarium.constants
{
   // fill_crafted_item's result (the server's 83 46 09): a modification added (1), or this. The game's
   // "nothing happened" (0) and "one fewer" are never sent: an upgrade always adds one.
   public class CraftResult
   {
      
      // the item cannot be crafted
      public static const FAILED:int = -2;
      
      public function CraftResult()
      {
         super();
      }
   }
}
