package survarium.constants
{
   // Where a buy was started (LobbyMenu.try_buy_item, set by each caller). Only a buy started in the
   // shop asks first about an item owned already (docs/notes/items.md §6; the owner, 2026-11-05).
   public final class BuyOrigin
   {

      // a shop entry: its card's Buy, a double click, its right-click Buy, a drop on the kit; the
      // results screen's drop cards (a stack: never asked)
      public static const SHOP:BuyOrigin = new BuyOrigin("shop");

      // the player's own items: an inventory row or its card, a kit slot, their right-click menu
      public static const OWN:BuyOrigin = new BuyOrigin("own");

      // the craft window ("Upgrade") and the attachment window: their slots' and list's Buy
      public static const UPGRADE:BuyOrigin = new BuyOrigin("upgrade");

      private var _name:String;

      public function BuyOrigin(name:String)
      {
         super();
         this._name = name;
      }

      public function toString() : String
      {
         return this._name;
      }
   }
}
