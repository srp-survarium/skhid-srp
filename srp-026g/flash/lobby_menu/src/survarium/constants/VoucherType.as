package survarium.constants
{
   // A voucher's voucher_type (gameplay/items/vouchers/*.options; ItemData.type). The drop card is the one
   // voucher left: the rename (0), repair (2) and 1-day premium (3) vouchers are gone (the owner, 2026-11-08).
   public class VoucherType
   {
      
      // drop_card
      public static const DROP_CARD:int = 1;
      
      public function VoucherType()
      {
         super();
      }
   }
}
