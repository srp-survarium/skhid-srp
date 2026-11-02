package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ListEvent;
   import survarium.Util;
   import survarium.constants.Events;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.data.PropertiesDescriptor;
   import survarium.interfaces.IPlayerProfile;
   import survarium.upgrade.UpgradeWindow;
   import survarium.constants.Faction;
   import survarium.constants.ItemCategory;
   import survarium.constants.ItemCondition;
   import survarium.constants.ItemPlace;
   import survarium.constants.ItemPrice;
   import survarium.constants.ModificationState;
   import survarium.constants.ModifierType;
   import survarium.constants.ProfileSlot;
   import survarium.item.ModificationRolls;
   import survarium.constants.VoucherType;
   
   public class ItemCard extends MovieClip
   {
      
      protected const BORDER_OFFSET:uint = 9;
      
      protected const BUTTON_MARGIN:uint = 4;
      
      protected var _cost:Number = 0;
      
      protected var _data:ItemData;
      
      protected var isShopItems:Boolean;
      
      protected var isQuickSlot:Boolean;
      
      protected var isStack:Boolean;
      
      protected var _step:int = 0;
      
      protected var upgradeMessage:*;
      
      protected var _selectedItemData:ItemData = null;
      
      protected var _inventory:InventoryList;
      
      protected var _propsDescriptor:PropertiesDescriptor;
      
      protected var _modifiersIconContainer:IconContainer;
      
      public function ItemCard()
      {
         super();
         this.sortButton();
         alpha = 0;
      }
      
      public function updateButtonEnabled() : void
      {
         this.checkButtonEnabled();
      }
      
      public function upgradeItem(event:ButtonEvent = null) : void
      {
         trace("upgradeItem::",this._data.itemId,this._data.icon,this._data.messIcon);
         if(this._data.broken)
         {
            Message.showMessage("ok",LocalizationData.getName("reroll_message_broken_item"));
            return;
         }
         var rerollItem:ItemData = this._data.clone();
         rerollItem.itemId = this._data.itemId;
         rerollItem.name = this._data.name;
         rerollItem.icon = this._data.messIcon;
         rerollItem.rank = this._data.rank;
         rerollItem.value = this._data.value;
         rerollItem.factionId = this._data.factionId;
         rerollItem.rerollTime = this._data.rerollTime;
         upgradeMessage = Message.showMessage("upgrade_item",rerollItem,true);
         ExternalInterface.call("item_reroll_opened",this._data.itemId);
      }
      
      private function fillItemModifiersTest() : void
      {
         var options:Object = new Object();
         options.modifiers = this._data.modifiers;
         this.fillItemModifiers(options);
      }
      
      public function fillItemModifiers(options:Object) : void
      {
         var modifiers:Array = options.modifiers;
         trace("fillItemModifiers",modifiers);
         var modifiersItem:ItemData = new ItemData();
         modifiersItem.modifiersDefault = [];
         modifiersItem.modifiers = modifiers;
         this._inventory.fillModifiersDescription(modifiersItem);
         this.upgradeMessage.updateModifiers(options);
      }
      
      public function fillItemRerollOptions(modifiers:Array) : void
      {
         var modifiersItem:ItemData = new ItemData();
         modifiersItem.modifiersDefault = [];
         modifiersItem.modifiers = modifiers;
         this._inventory.fillModifiersDescription(modifiersItem);
         this.upgradeMessage.updateReroll(modifiers);
      }
      
      public function repairItem(event:ButtonEvent = null) : void
      {
         trace("repairItem::",this._data.itemId,this._data.icon,this._data.messIcon);
         var repairData:ItemData = new ItemData();
         repairData.itemId = this._data.itemId;
         repairData.name = this._data.name;
         repairData.sellerIcon = this._data.sellerIcon ? this._data.sellerIcon.clone() : null;
         repairData.icon = this._data.messIcon;
         repairData.value = this._data.value;
         repairData.modifiers = this.getFailModifiers();
         repairData.repairCostRatio = this._data.repairCostRatio;
         repairData.cost = this._data.cost;
         var profile:IPlayerProfile = parent["player_profile"];
         Message.showMessage("repair",repairData,true);
      }
      
      public function openModifiersWindow(event:ButtonEvent = null) : void
      {
         dispatchEvent(new Event(Events.OPEN_MODIFIERS));
      }
      
      private function onRollOut(event:ListEvent) : void
      {
      }
      
      private function onRollOver(event:ListEvent) : void
      {
         if(event.itemData)
         {
         }
      }
      
      public function updateSelectedItem(selection:Object) : void
      {
         _selectedItemData = selection.data;
         trace("itemCardUpdateSelectedItemSt::",selection.data);
         if(!this._selectedItemData)
         {
            return;
         }
         trace("updateValueTo::",this._data.name,this._data.itemId,this._selectedItemData.value);
         dispatchEvent(new Event(Event.COMPLETE));
         trace("itemCardUpdateSelectedItem::",this._selectedItemData);
      }
      
      protected function closeSelection(event:Event) : void
      {
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      protected function cancelAction(event:Event) : void
      {
      }
      
      // The card's Buy (its button, the list's double click): an item owned already is asked about
      // first (LobbyMenu.try_buy_item).
      public function onBtnClick1() : void
      {
         this.buy(false);
      }

      // Buy chosen from the right-click menu of an item the player owns (an inventory row, a kit
      // slot): the player's own wish for another, so no "already purchased" question (the owner,
      // 2026-11-01; docs/notes/items.md §6).
      public function buyFromOwnMenu() : void
      {
         this.buy(true);
      }

      private function buy(ownItemMenu:Boolean) : void
      {
         var item:Object;
         var profile:IPlayerProfile;
         var price:int;
         trace("IemCard::buy",this._data,ownItemMenu);
         if(this._data)
         {
            item = this._data;
            profile = parent["player_profile"];
            price = item.category == ItemCategory.BUNDLE ? (item.cost) : (GangShop.getInstance().getItemPrice(item.dictId));
            // No seller: the exe fills in the first price record's (lobby_requests.md, 47 00). It
            // read item.seller, which ItemData does not have: Scaleform gave undefined, so 0.
            GangShop.buyFunction(profile.profileId,item.itemId,item.dictId,item.descriptor,ItemPlace.INVENTORY,item.count,price,Faction.NONE,!item.isStack ? this.onItemBuyAccept : null,ownItemMenu);
         }
      }
      
      protected function onItemUse(event:* = null) : void
      {
         ExternalInterface.call("item_use",this._data.itemId);
      }
      
      public function onBtnClick3() : void
      {
         var messageData:Object;
         if(this._data.usable)
         {
            trace("useElement::",this._data.type,parent);
            if(this._data.type == VoucherType.RENAME)
            {
               if(parent)
               {
                  parent["statusPanel"].showChangeNameMessage();
               }
               return;
            }
            messageData = {
               text:LocalizationData.getName("use_item_message"),
               onYes:this.onItemUse,
               label_ok:LocalizationData.getName("btn_use_item"),
               label_cancel:LocalizationData.getName("no")
            };
            Message.showMessage("apply",messageData);
         }
      }
      
      public function onBtnClick2() : void
      {
         if(!this._data)
         {
            return;
         }
         var item:ItemData = this._data;
         trace("Try to Sell pr:" + " uid:" + item.dictId + " cnt:" + item.value + " iss:" + item.isStack + " price:" + int(GangShop.getInstance().getItemPrice(item.dictId)) + " jj:" + item.itemId);
         var sellData:Object = new Object();
         if(!item.isStack)
         {
            sellData.count = 1;
         }
         else
         {
            sellData.count = int(item.value);
         }
         sellData.price = this._cost;
         sellData.id = item.itemId;
         sellData.uid = item.dictId;
         sellData.sell = true;
         sellData.name = item.name;
         sellData.is_stack = item.isStack;
         sellData.weight = item.weight;
         if(item.clipSize)
         {
            sellData.clip_size = item.clipSize;
         }
         trace("sellFromIC::",item.name,item.isPremium,this._cost);
         sellData.isPremium = item.isPremium && this.isShopItems;
         sellData.icon = (item.icon as Icon).clone();
         Message.showMessage("asks",sellData,true);
      }
      
      public function getItemData() : ItemData
      {
         return this._data;
      }
      
      public function setInventory(inventory:InventoryList) : void
      {
         _inventory = inventory;
      }
      
      public function setItemData(itemData:ItemData) : void
      {
         _data = itemData;
         if(this._data != null)
         {
            trace("IC:updateItemData",itemData.name,itemData.broken);
            alpha = 1;
            this.updateData();
         }
         else
         {
            alpha = 0;
         }
         dispatchEvent(new Event(Event.ACTIVATE));
      }
      
      protected function onSlotUpdate(event:ListEvent = null) : void
      {
         this.slotUpdate(event.itemData);
      }
      
      protected function slotUpdate(slotData:Object) : void
      {
         var price:uint;
         var sellPrice:Number;
         if(this.isShopItems)
         {
            _cost = this._data.cost;
         }
         else
         {
            price = this._data.isPremium ? (this._data.cost * PlayerInfo.premiumExchangeRate) : this._data.cost;
            trace("IC::itemCost::",price,this._data.cost,PlayerInfo.premiumExchangeRate);
            sellPrice = price / ItemPrice.SELL_DIVISOR * (this._data.isStack ? 1 : this._data.value / ItemCondition.FULL);
            _cost = sellPrice;
         }
         _step = slotData.clip_size == undefined ? 1 : (slotData.clip_size);
         trace("onSlotUpdate",this._cost,slotData.cost);
      }
      
      protected function onItemBuyAccept() : void
      {
         dispatchEvent(new Event(Events.BUY));
      }
      
      public function onBtnClick4(event:Event = null) : void
      {
         if(this.isShopItems)
         {
            this.onBtnClick1();
         }
         else
         {
            this.onBtnClick2();
         }
      }
      
      public function checkButtonEnabled(event:Event = null) : void
      {
         if(!this._data)
         {
            return;
         }
      }
      
      protected function size() : void
      {
      }
      
      protected function updateLayout(event:Event = null) : void
      {
      }
      
      public function setPropsDescriptor(descriptor:PropertiesDescriptor) : void
      {
         trace("IC:setPropsDescriptor::",descriptor);
         _propsDescriptor = descriptor;
      }
      
      public function setModifiersIcons(icons:IconContainer) : void
      {
         _modifiersIconContainer = icons;
      }
      
      protected function getFailModifiers() : Array
      {
         var allModifiers:Array = this._data.modifiers != null ? this._data.modifiers : [];
         var failModifiers:Array = [];
         var i:uint = 0;
         var modifierIcon:Icon;
         while(i < allModifiers.length)
         {
            if(allModifiers[i].state == ModificationState.DEFECT)
            {
               modifierIcon = this._modifiersIconContainer.getIcon(allModifiers[i].icon);
               failModifiers.push({
                  name:allModifiers[i].name,
                  props:allModifiers[i].props,
                  icon:modifierIcon,
                  state:allModifiers[i].state,
                  fix_time_general:allModifiers[i].fix_time_general,
                  fix_cost_general:allModifiers[i].fix_cost_general,
                  fix_cost_gold:allModifiers[i].fix_cost_gold
               });
            }
            i++;
         }
         this.fillModifiersNames(failModifiers);
         return failModifiers;
      }
      
      protected function fillModifiersNames(modifiers:Array) : void
      {
         var i:uint = 0;
         var text:String;
         var props:Object;
         var prop:Object;
         var propDescription:Object;
         var postfix:String;
         var sign:String;
         var propValue:Number;
         var color:String;
         while(i < modifiers.length)
         {
            text = "";
            props = modifiers[i].props;
            trace("props::",text,props,this._propsDescriptor);
            for each(prop in props)
            {
               propDescription = this._propsDescriptor.getPropertyDescription(prop.prop_id);
               if(propDescription)
               {
                  if(propDescription.postfix == undefined)
                  {
                     propDescription.postfix = "";
                  }
                  postfix = propDescription.modifier_type == ModifierType.ADD ? propDescription.postfix : "%";
                  sign = prop.prop_value >= 0 ? "+" : "";
                  if(propDescription.modifier_type == undefined)
                  {
                     propDescription.modifier_type = ModifierType.INVALID;
                     propDescription.modifier_type = prop.modifier_type;
                  }
                  if(propDescription.modifier_type == ModifierType.INVALID)
                  {
                     throw Error("modifier_type at modifier " + modifiers[i].name + " for prop_id " + prop.prop_id + " is invalid");
                  }
                  trace("propValue::",propDescription.direction,prop.direction);
                  propValue = propDescription.modifier_type == ModifierType.ADD ? (prop.prop_value) : prop.prop_value * 100;
                  color = ModificationRolls.style(propDescription,prop.prop_id,ModificationRolls.value(prop));
                  text += propDescription.prop_name + ": " + Util.styledChar(sign + (int(propValue) == propValue ? propValue.toString() : propValue.toFixed(1)) + postfix,color) + "\n";
               }
            }
            modifiers[i].name = text;
            i++;
         }
      }
      
      protected function updateData() : void
      {
         var canBuy:Boolean = this._data.canBuy;
         var price:uint;
         var sellPrice:Number;
         isShopItems = this._data.descriptor == ItemPlace.SHOP;
         if(!this.isShopItems)
         {
            canBuy = GangShop.getInstance().getItemBuyable(this._data.dictId);
         }
         isQuickSlot = this._data.descriptor < ItemPlace.INVENTORY && this._data.slotId > ProfileSlot.WEAPON_2_AMMO_2;
         isStack = this._data.isStack;
         if(this.isShopItems)
         {
            _cost = this._data.cost;
         }
         else
         {
            price = this._data.isPremium ? (this._data.cost * PlayerInfo.premiumExchangeRate) : this._data.cost;
            sellPrice = price / ItemPrice.SELL_DIVISOR * (this._data.isStack ? 1 : this._data.value / ItemCondition.FULL);
            trace("IC::itemCost::",price,this._data.isPremium,this._data.cost,PlayerInfo.premiumExchangeRate,"sellCost::",sellPrice);
            _cost = sellPrice;
         }
         _step = this._data.clipSize == 0 ? 1 : (this._data.clipSize);
         if(this.isQuickSlot || this.isStack)
         {
            this.slotUpdate(this._data);
            this.checkButtonEnabled();
         }
         else
         {
            this.checkButtonEnabled();
         }
         if(this.upgradeMessage)
         {
            this.upgradeMessage.updateTimer(this._data);
         }
      }
      
      private function sortButton() : void
      {
      }
      
      private function isUpgradeable(item:Object) : Boolean
      {
         var category:int = int(item.category);
         return (category == ItemCategory.BODY_ARMOR || category == ItemCategory.HELMETS || category == ItemCategory.PANTS || category == ItemCategory.RIGS) && (Boolean(this._data.camoAvailable) || Boolean(this._data.decalAvailable)) || UpgradeWindow.isWeaponCategory(category) && (UpgradeWindow.hasScopes(int(item.dictId)) || Boolean(this._data.camoAvailable) || Boolean(this._data.decalAvailable));
      }
   }
}

