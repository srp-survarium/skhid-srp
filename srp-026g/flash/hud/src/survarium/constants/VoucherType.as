package survarium.constants
{
   // A voucher's voucher_type (gameplay/items/vouchers/*.options; ItemData.type).
   public class VoucherType
   {
      
      // rename_account_voucher
      public static const RENAME:int = 0;
      
      // drop_card
      public static const DROP_CARD:int = 1;
      
      // repair_voucher
      public static const REPAIR:int = 2;
      
      // premium_access_voucher_1d
      public static const PREMIUM:int = 3;
      
      public function VoucherType()
      {
         super();
      }
   }
}
