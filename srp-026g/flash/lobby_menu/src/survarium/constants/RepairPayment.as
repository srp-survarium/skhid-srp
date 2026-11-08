package survarium.constants
{
   // How a repair is paid (repair_item's second argument; the server's 47 03).
   public class RepairPayment
   {
      
      // silver, the one way left (no gold, no repair kit: the owner, 2026-11-08)
      public static const REGULAR:int = 0;
      
      public function RepairPayment()
      {
         super();
      }
   }
}
