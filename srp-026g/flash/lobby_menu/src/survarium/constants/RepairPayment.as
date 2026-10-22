package survarium.constants
{
   // How a repair is paid (repair_item's second argument; the server's 47 03).
   public class RepairPayment
   {
      
      public static const REGULAR:int = 0;
      
      public static const PREMIUM:int = 1;
      
      // a repair voucher
      public static const VOUCHER:int = 2;
      
      public function RepairPayment()
      {
         super();
      }
   }
}
