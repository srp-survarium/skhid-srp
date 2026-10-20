package
{
    import flash.display.DisplayObjectContainer;
    import flash.events.Event;
    import scaleform.clik.events.ButtonEvent;
    import survarium.constants.Events;
    import survarium.controls.GangShop;
    import survarium.data.ItemData;
    import survarium.item.ItemFactory;
    import survarium.controls.InventoryList;
    import survarium.controls.SrvContextMenu;
    import survarium.data.LocalizationData;
    import survarium.messages.MessageCraftItem;

    // The craft window ("Upgrade": two items of a kind make a better one)
    // locks the inventory list, and a locked list gave no right-click menu.
    // While it is open, right-click on an item of the list gives Buy and Sell
    // only, by the list's own rules for them; nothing else (no Upgrade).
    public class SrpCraftMenu
    {
        // InventoryList.showContextMenu reads `this._inventoryLocked` through
        // this: true ends the method there.
        public static function locked(value:*, list:InventoryList, slot:*):Boolean
        {
            // Only the craft window's lock: a list locked otherwise, or not
            // locked, is as before.
            if (!value)
                return false;
            var window:* = craftWindow(list);
            if (window == null)
                return true;
            var item:Object = slot != null ? slot.data : null;
            if (item == null || !(item.enabled || item.canBuy))
                return true;
            var owned:Boolean = item.descriptor <= 100 && item.enabled;
            var options:Array = [];
            // As the list's menu: Buy for an owned item, a stack or an item
            // of no rarity. Sell for an owned item (the list's sellElement
            // wants it enabled), not in the shop, and not one of the two the
            // window holds: the craft would name an item no longer owned.
            if (item.canBuy && (item.isStack || !item.rank || owned))
                options.push({"name": LocalizationData.getName("buy"),
                              "click_function": function(e:* = null):void { buy(list, item); }});
            if (owned && !list.isShop && !holds(window, item.itemId))
                options.push({"name": LocalizationData.getName("sell"),
                              "click_function": function(e:* = null):void { list.sellItem(item.itemId); }});
            if (options.length > 0)
                SrvContextMenu.show(options);
            return true;
        }

        // The end of MessageCraftItem.fillCraftedItem: the result is shown
        // with one button, "Ready", which closes the window. It gets two:
        // Exit (the window's Cancel, which the lobby wires to close it, as
        // Ready did) and Continue (the Ready button: the window again, for
        // the same item, upgraded, with an empty donor slot).
        public static function result(window:*, outcome:int):void
        {
            // -2: the craft was refused; the window closes by itself.
            if (outcome == -2)
                return;
            var accept:* = window.btnAccept;
            var cancel:* = window.btnCancel;
            // The game's words: "Exit" (btn_exit_label) and "Continue"
            // (btn_go_to_online_store, the store dialog's own Continue).
            cancel.label = LocalizationData.getName("btn_exit_label");
            accept.label = LocalizationData.getName("btn_go_to_online_store");
            cancel.visible = true;
            accept.enabled = true;
            // The game's order (MessageApply, MessageAsk, MessageExit): the
            // accepting button left, Cancel right.
            var middle:Number = window.width / 2;
            accept.x = middle - accept.width - 4;
            cancel.x = middle + 4;
            accept.addEventListener(ButtonEvent.CLICK, function(e:* = null):void { proceed(window); });
        }

        private static function proceed(window:*):void
        {
            var main:Object = window.dragItemSlot1.data;
            var list:InventoryList = window.parent != null ? window.parent["inventory_list"] as InventoryList : null;
            window.dispatchEvent(new Event("closeCraft"));
            if (main == null || list == null)
                return;
            // The item as the inventory has it now, upgraded; one with three
            // modifications, or that cannot be crafted, ends here as Exit.
            var item:ItemData = list.getInventoryItem(main.itemId);
            // As the game's own Upgrade entry: owned and enabled.
            if (item == null || !item.enabled || item.rank >= 3 || !ItemFactory.isItemCraftable(item))
                return;
            list.setPrimaryItem(item);
            list.dispatchEvent(new Event(Events.CRAFFT_ITEM));
        }

        private static function craftWindow(list:InventoryList):*
        {
            var lobby:DisplayObjectContainer = list.parent;
            if (lobby == null)
                return null;
            for (var i:int = 0; i < lobby.numChildren; i++)
                if (lobby.getChildAt(i) is MessageCraftItem)
                    return lobby.getChildAt(i);
            return null;
        }

        // The item being upgraded or the donor.
        private static function holds(window:*, itemId:uint):Boolean
        {
            var main:Object = window.dragItemSlot1.data;
            var donor:Object = window.dragItemSlot2.data;
            return (main != null && main.itemId == itemId) || (donor != null && donor.itemId == itemId);
        }

        // As the item card's Buy: by the dictionary id, at the shop's price;
        // the shop names the seller. A buy on an owned item is not asked
        // whether the account wants another (buy_modified_026g.py).
        private static function buy(list:InventoryList, item:Object):void
        {
            var profile:* = list.parent != null ? list.parent["player_profile"] : null;
            if (profile == null)
                return;
            var price:int = item.category == 100 ? int(item.cost) : int(GangShop.getInstance().getItemPrice(item.dictId));
            GangShop.buyFunction(profile.profileId, item.itemId, item.dictId, item.descriptor, 100, 1, price, -1);
        }
    }
}
