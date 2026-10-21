package survarium.constants
{
   // fill_crafted_item's result (the server's 83 46 09); any other value: a modification removed.
   public class CraftResult
   {
      
      public static const NOTHING:int = 0;
      
      // a modification was added
      public static const ADDED:int = 1;
      
      // the item cannot be crafted
      public static const FAILED:int = -2;
      
      public function CraftResult()
      {
         super();
      }
   }
}
