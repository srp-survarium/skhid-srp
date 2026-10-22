package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.IEventDispatcher;
   import flash.events.TimerEvent;
   import flash.utils.Timer;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.Util;
   import survarium.buttons.ButtonClick;
   import survarium.buttons.ButtonProgress;
   import survarium.constants.Events;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IInventoryItemFactory;
   import survarium.item.ItemFactory;
   import survarium.upgrade.UpgradeWindow;
   import survarium.constants.ItemCategory;
   import survarium.constants.ItemCondition;
   import survarium.constants.ItemPlace;
   import survarium.constants.ItemPrice;
   import survarium.constants.ModificationState;
   import survarium.constants.MoneyType;
   import survarium.constants.ProfileSlot;
   
   public class ItemControlPanel extends MovieClip
   {
      
      public var btn_item_upgrade:ButtonClick;
      
      public var btn_item_fix:ButtonProgress;
      
      public var btn_item_buy:Button;
      
      public var btnSellBig:Button;
      
      public var btn_item_sell:Button;
      
      public var btn_show_l:Button;
      
      public var btn_show_r:Button;
      
      public var btn_show_use:Button;
      
      public var btn_item_reroll:Button;
      
      public var btn_item_craft:Button;
      
      public var rerollTimer:MovieClip;
      
      public var btnAccept:Button;
      
      public var btnCancel:Button;
      
      private var backgroundWidth:uint = 574;
      
      // meaning unknown: the most steps (or units) the buy and sell counter allows (maxCount, which nothing reads)
      private static const MAX_COUNT_STEPS:int = 100;
      
      private var _data:ItemData = null;
      
      private var _inventory:IInventoryItemFactory = null;
      
      private var _cost:Number = 0;
      
      private var _isShopItems:Boolean;
      
      private var _isQuickSlot:Boolean;
      
      private var _isStack:Boolean;
      
      private var _money_type:int = MoneyType.REGULAR;
      
      private var _step:int = 0;
      
      private var _itemCard:*;
      
      private var _locked:Boolean = false;
      
      private var _shopMode:Boolean = false;
      
      private var _rerollable:Boolean = false;
      
      public function ItemControlPanel()
      {
         super();
         LocalizationData.addButton(this.btn_show_l,"buy");
         LocalizationData.addButton(this.btnSellBig,"buy");
         addEventListener(Event.RENDER,this.configUI);
      }
      
      public function toggleShop(isShop:Boolean) : void
      {
         _shopMode = isShop;
         this.updateButtons();
         this.sortButton();
      }
      
      private function configUI(event:Event) : void
      {
         removeEventListener(Event.RENDER,this.configUI);
         LocalizationData.addCallback(this.onLocalization,"sell");
         this.rerollTimer.mouseEnabled = false;
         this.rerollTimer.mouseChildren = false;
         this.rerollTimer.visible = false;
         this.btnAccept.visible = false;
         this.btnCancel.visible = false;
         this.btn_show_use.visible = false;
         this.updateData(null);
         var timer:Timer = MainScene.getTimerInstance();
         timer.addEventListener(TimerEvent.TIMER,this.onTimerUpdate);
         this.btnSellBig.width = this.btn_show_l.width = this.btn_show_r.width = this.btn_show_use.width = 140;
      }
      
      private function onTimerUpdate(event:TimerEvent) : void
      {
         var timer:Timer;
         var elapsed:uint;
         if(this._data && this._data.rerollTime)
         {
            timer = MainScene.getTimerInstance();
            elapsed = (timer.currentCount);
            var left:String = Util.getTimeStringMinutes(Math.max(0,int(this._data.rerollTime / 60) - (elapsed - this._data.timeDelta)));
            this.rerollTimer.setTime(left);
            // Skhid: the button keeps its name, the wait for a silver reroll beside it (the game's
            // empty button with a timer read as no button; a gold reroll works meanwhile)
            if(this._rerollable)
            {
               this.btn_item_reroll.label = LocalizationData.getName("btn_reroll_label") + " " + left;
            }
         }
      }
      
      private function onLocalization(localizedText:String) : void
      {
         this.btn_item_upgrade.label = LocalizationData.getName("btn_upgrade_label");
         this.btn_item_upgrade.data = {name:LocalizationData.getName("btn_upgrade_hint")};
         this.btn_item_sell.label = LocalizationData.getName("sell");
         this.btn_item_buy.label = LocalizationData.getName("buy");
         this.btn_item_fix.label = LocalizationData.getName("fix");
         this.btn_show_r.label = LocalizationData.getName("sell");
         this.btn_show_use.label = LocalizationData.getName("btn_use_item");
         this.btn_item_reroll.label = LocalizationData.getName("btn_reroll_label");
         this.btn_item_craft.label = LocalizationData.getName("btn_craft_label");
      }
      
      public function setItemControl(itemCard:*) : void
      {
         _itemCard = itemCard;
         this.btn_item_sell.addEventListener(ButtonEvent.CLICK,this._itemCard.onBtnClick2);
         this.btn_item_buy.addEventListener(ButtonEvent.CLICK,this._itemCard.onBtnClick1);
         this.btnSellBig.addEventListener(ButtonEvent.CLICK,this._itemCard.onBtnClick1);
         this.btn_item_fix.addEventListener(ButtonEvent.CLICK,this._itemCard.repairItem);
         this.btn_item_upgrade.addEventListener(ButtonEvent.CLICK,this.openModifiersWindow);
         this.btn_item_reroll.addEventListener(ButtonEvent.CLICK,this._itemCard.upgradeItem);
         this.btn_item_craft.addEventListener(ButtonEvent.CLICK,this.craftItem);
         this.btn_show_r.addEventListener(ButtonEvent.CLICK,this._itemCard.onBtnClick2);
         this.btn_show_use.addEventListener(ButtonEvent.CLICK,this._itemCard.onBtnClick3);
         this.btn_show_l.addEventListener(ButtonEvent.CLICK,this._itemCard.onBtnClick1);
      }
      
      private function craftItem(event:ButtonEvent) : void
      {
         dispatchEvent(new Event(Events.CRAFFT_ITEM));
      }
      
      public function setInvenory(inventory:IInventoryItemFactory) : void
      {
         _inventory = inventory;
      }
      
      private function openModifiersWindow(event:Event) : void
      {
         (this._inventory as IEventDispatcher).dispatchEvent(new Event(Events.OPEN_MODIFIERS));
      }
      
      public function toggleModifiersButtons() : void
      {
         _locked = !this._locked;
         this.updateButtons();
      }
      
      public function updateData(itemData:ItemData) : void
      {
         var maxCount:int = 0;
         var price:uint = 0;
         var sellPrice:uint = 0;
         var modifierCount:uint = 0;
         var i:uint = 0;
         var modifier:Object = null;
         this._data = itemData;
         if(!this._data)
         {
            this.checkButtonEnabled();
            this.updateButtons();
            return;
         }
         var canBuy:Boolean = this._data.canBuy;
         this._isShopItems = this._data.descriptor == ItemPlace.SHOP || this._data.descriptor == ItemPlace.BUNDLE;
         if(!this._isShopItems)
         {
            canBuy = this._data.canBuy;
         }
         this._isQuickSlot = this._data.descriptor < ItemPlace.INVENTORY && this._data.slotId > ProfileSlot.WEAPON_2_AMMO_2;
         this._isStack = this._data.isStack;
         if(this._isShopItems)
         {
            this._cost = this._data.cost;
         }
         else
         {
            price = this._data.isPremium ? uint(this._data.cost * PlayerInfo.premiumExchangeRate) : this._data.cost;
            sellPrice = price / ItemPrice.SELL_DIVISOR * (this._data.isStack ? 1 : this._data.value / ItemCondition.FULL);
            this._cost = sellPrice;
         }
         this._step = this._data.clipSize == 0 ? 1 : int(this._data.clipSize);
         this.btn_show_r.enabled = !this._isShopItems;
         this.btn_show_l.enabled = canBuy;
         this.btn_show_r.alpha = this.btn_show_r.enabled ? 1 : 0.5;
         this.btn_show_l.alpha = this.btn_show_l.enabled ? 1 : 0.5;
         var hasModifierToFix:Boolean = false;
         this._rerollable = false;
         if(!this._data.isStack && Boolean(this._data.modifiers))
         {
            modifierCount = this._data.modifiers.length;
            i = 0;
            while(i < modifierCount)
            {
               modifier = this._data.modifiers[i];
               if(Boolean(modifier) && modifier.state == ModificationState.DEFECT)
               {
                  hasModifierToFix = true;
               }
               if(Boolean(modifier) && Boolean(modifier.state == ModificationState.MODIFICATION) && !modifier.isDefault)
               {
                  this._rerollable = true;
               }
               i = ++i;
            }
         }
         if(this._rerollable && Boolean(this._data.rerollTime))
         {
            if(Boolean(this._data.rerollTime))
            {
               this.rerollTimer.visible = false;
               this.onTimerUpdate(null);
               this.btn_item_reroll.tipEnabled = true;
               this.btn_item_reroll.tooltipRendererName = "SrvProfileTipRenderer";
               this.btn_item_reroll.tipData = {"name":LocalizationData.getName("reroll_timer_hint")};
            }
         }
         else
         {
            this.btn_item_reroll.tipEnabled = false;
            this.btn_item_reroll.label = LocalizationData.getName("btn_reroll_label");
            this.rerollTimer.visible = false;
         }
         this.btn_item_reroll.enabled = this._rerollable && !this._isShopItems && !this._data.repairTime;
         this.btn_item_craft.enabled = ItemFactory.isItemCraftable(this._data);
         if(Boolean(this._data.repairTime))
         {
            if(this.rerollTimer.visible == false)
            {
               this.btn_item_reroll.tipEnabled = true;
               this.btn_item_reroll.tooltipRendererName = "SrvProfileTipRenderer";
               this.btn_item_reroll.tipData = {"name":LocalizationData.getName("cant_upgrade_while_repair_hint")};
            }
            this.btn_item_craft.tipEnabled = true;
            this.btn_item_craft.tooltipRendererName = "SrvProfileTipRenderer";
            this.btn_item_craft.tipData = {"name":LocalizationData.getName("cant_upgrade_while_repair_hint")};
         }
         else
         {
            this.btn_item_craft.tipEnabled = false;
         }
         this.btn_item_fix.enabled = !this._isShopItems && (this._data.value != ItemCondition.FULL || hasModifierToFix) && this._data.enabled;
         this.btn_item_sell.data = {"name":this.btn_item_sell.label};
         this.btn_item_buy.data = {"name":this.btn_item_buy.label};
         this.btn_item_fix.data = {"name":this.btn_item_fix.label};
         trace("ICPData:",this._data.name,canBuy,"isShopItems::",this._isShopItems);
         this.updateButtons();
         this.btn_item_sell.tooltipRendererName = "SrvProfileTipRenderer";
         this.btn_item_buy.tooltipRendererName = "SrvProfileTipRenderer";
         this.btn_item_sell.tipData = {"name":LocalizationData.getName("cost") + ": " + this._cost.toString()};
         this.btn_item_buy.tipData = {"name":LocalizationData.getName("cost") + ": " + this._cost.toString()};
         this.btnSellBig.label = LocalizationData.getName("buy");
         if(this._isShopItems)
         {
            maxCount = this._data.cost < 1 ? int(Math.min(Math.max(0,int((PlayerInfo.MAX_FREE_ITEMS_COUNT - this._data.value) / this._step)),MAX_COUNT_STEPS)) : int((this._data.isPremium ? PlayerInfo.premiumMoney : PlayerInfo.genericMoney) / this._data.cost);
         }
         else
         {
            maxCount = Math.min(MAX_COUNT_STEPS * this._step,this._data.isStack ? this._data.value : 1);
         }
         var premiumLocked:Boolean = !this._data.canBuy && Boolean(this._data.premiumCost);
         this.btn_item_upgrade.enabled = !this._isShopItems && !this._isStack && this.isUpgradeable(this._data) && this._data.enabled;
         this.btn_item_upgrade.alpha = this.btn_item_upgrade.enabled ? 1 : 0.5;
         this.btn_item_upgrade.label = UpgradeWindow.isWeaponCategory(int(this._data.category)) ? LocalizationData.getName("scope_window_title") : LocalizationData.getName("btn_upgrade_label");
         var condition:uint = this._isShopItems ? ItemCondition.FULL : this._data.value;
         var conditionRatio:Number = condition / ItemCondition.FULL;
         this.btn_item_fix.progress = conditionRatio;
         this.btn_item_fix.progressLabel = condition.toString() + "%";
         if(this._isQuickSlot || this._isStack)
         {
            this.checkButtonEnabled();
         }
         else
         {
            this.checkButtonEnabled();
         }
         if(this._data.usable)
         {
            this.btn_show_use.enabled = true;
            this.btn_show_use.visible = true;
         }
         else
         {
            this.btn_show_use.enabled = false;
            this.btn_show_use.visible = false;
         }
         this.sortButton();
      }
      
      private function isUpgradeable(item:Object) : Boolean
      {
         var category:int = int(item.category);
         return (category == ItemCategory.BODY_ARMOR || category == ItemCategory.HELMETS || category == ItemCategory.PANTS || category == ItemCategory.RIGS) && (Boolean(this._data.camoAvailable) || Boolean(this._data.decalAvailable)) || UpgradeWindow.isWeaponCategory(category) && (UpgradeWindow.hasScopes(int(item.dictId)) || Boolean(this._data.camoAvailable) || Boolean(this._data.decalAvailable));
      }
      
      public function checkButtonEnabled(event:Event = null) : void
      {
         if(!this._data)
         {
            this.btnSellBig.enabled = this.btn_item_sell.enabled = this.btn_item_buy.enabled = this.btn_item_upgrade.enabled = this.btn_item_fix.enabled = this.btn_item_reroll.enabled = false;
            this.btn_item_craft.enabled = false;
            this.btn_item_craft.alpha = this.btn_item_reroll.alpha = this.btn_item_upgrade.alpha = this.btn_item_buy.alpha = this.btn_item_sell.alpha = 0.5;
            return;
         }
         var buyUnlocked:Boolean = !this._inventory.getElelementLocked(LobbyMenu.SHOP_ELEMNENT_ID);
         var sellUnlocked:Boolean = !this._inventory.getElelementLocked(LobbyMenu.INVENTORY_ELEMNENT_ID);
         var premiumLocked:Boolean = !this._data.canBuy && this._data.premiumCost;
         var canAfford:Boolean = this._data.isPremium || !this._data.isPremium && PlayerInfo.genericMoney >= this._data.cost;
         this.btn_item_sell.enabled = this._data.enabled && sellUnlocked;
         this.btn_item_buy.enabled = (this._data.canBuy || premiumLocked) && buyUnlocked && canAfford;
         this.btnSellBig.enabled = this.btn_item_buy.enabled;
         this.btnSellBig.alpha = this.btnSellBig.enabled ? 1 : 0.5;
         this.btn_item_buy.alpha = this.btn_item_buy.enabled ? 1 : 0.5;
         this.btn_item_sell.alpha = this.btn_item_sell.enabled ? 1 : 0.5;
         this.btn_item_reroll.alpha = this.btn_item_reroll.enabled ? 1 : 0.5;
         this.btn_item_craft.alpha = this.btn_item_craft.enabled ? 1 : 0.5;
         this.btn_item_upgrade.alpha = this.btn_item_upgrade.enabled ? 1 : 0.5;
         var buyable:Boolean = this._data.canBuy;
         if(!this._isShopItems)
         {
            buyable = this._data.canBuy;
         }
         this.btn_show_r.enabled = !this._isShopItems && buyUnlocked && sellUnlocked;
         this.btn_show_l.enabled = (buyable || premiumLocked) && buyUnlocked && sellUnlocked;
         this.btn_show_r.alpha = this.btn_show_r.enabled ? 1 : 0.5;
         this.btn_show_l.alpha = this.btn_show_l.enabled ? 1 : 0.5;
         trace("ICP:checkButtonEnabled","canBuy",this._data.canBuy,buyable,"lockedPremium::",premiumLocked,this.btn_show_l.enabled,this.btn_item_sell.enabled,"locked::",this._inventory.getElelementLocked(LobbyMenu.INVENTORY_ELEMNENT_ID),this._inventory.getElelementLocked(LobbyMenu.SHOP_ELEMNENT_ID));
      }
      
      private function updateButtons() : void
      {
         if(this._shopMode)
         {
            this.btnSellBig.visible = true;
            this.btn_show_l.visible = false;
            this.btn_show_r.visible = false;
            this.btn_item_upgrade.visible = false;
            this.btn_item_reroll.visible = false;
            this.btn_item_sell.visible = false;
            this.btn_item_buy.visible = false;
            this.btn_item_fix.visible = false;
            this.btn_item_craft.visible = false;
            return;
         }
         this.btnSellBig.visible = false;
         if(!this._data)
         {
            this.btn_show_l.visible = false;
            this.btn_show_r.visible = false;
            this.btn_item_upgrade.visible = true;
            this.btn_item_fix.visible = true;
            this.btn_item_reroll.visible = true;
            this.btn_item_craft.visible = true;
         }
         if(this._locked)
         {
            this.btn_show_l.visible = false;
            this.btn_show_r.visible = false;
            this.btn_item_sell.visible = !this._isShopItems;
            this.btn_item_buy.visible = !this.btn_item_sell.visible;
            this.btn_item_reroll.visible = true;
            this.btn_item_upgrade.visible = true;
            this.btn_item_fix.visible = true;
            this.btn_item_craft.visible = true;
            this.btn_item_reroll.enabled = this.btn_item_craft.enabled = this.btn_item_sell.enabled = this.btn_item_buy.enabled = this.btn_item_fix.enabled = this.btn_item_upgrade.enabled = false;
            this.btn_item_reroll.alpha = this.btn_item_craft.alpha = this.btn_item_sell.alpha = this.btn_item_buy.alpha = this.btn_item_fix.alpha = this.btn_item_upgrade.alpha = 0.5;
         }
         else if(this._isStack && !this._isQuickSlot)
         {
            this.btn_show_l.visible = true;
            this.btn_show_r.visible = true;
            this.btn_item_upgrade.visible = false;
            this.btn_item_reroll.visible = false;
            this.btn_item_sell.visible = false;
            this.btn_item_buy.visible = false;
            this.btn_item_fix.visible = false;
            this.btn_item_craft.visible = false;
         }
         else if(this._isQuickSlot)
         {
            this.btn_show_l.visible = true;
            this.btn_show_r.visible = true;
            this.btn_item_upgrade.visible = false;
            this.btn_item_sell.visible = false;
            this.btn_item_buy.visible = false;
            this.btn_item_fix.visible = false;
            this.btn_item_reroll.visible = false;
            this.btn_item_craft.visible = false;
         }
         else
         {
            this.btn_show_l.visible = false;
            this.btn_show_r.visible = false;
            this.btn_item_sell.visible = !this._isShopItems;
            this.btn_item_buy.visible = !this.btn_item_sell.visible;
            this.btn_item_craft.visible = true;
            this.btn_item_reroll.visible = true;
            this.btn_item_upgrade.visible = true;
            this.btn_item_fix.visible = true;
         }
      }
      
      private function sortButton() : void
      {
         var offset:uint;
         this.btnSellBig.x = (this.backgroundWidth - 140) / 2;
         if(this.btn_show_use.visible)
         {
            offset = 140;
            this.btn_show_l.x = int((width - offset * 3) / 2);
            this.btn_show_r.x = int(this.btn_show_l.x + offset + 10);
            this.btn_show_use.x = int(this.btn_show_r.x + offset + 10);
            this.btn_show_use.y = this.btn_show_l.y;
         }
         else
         {
            offset = this.backgroundWidth / 2;
            this.btn_show_r.x = int(offset + 5);
            this.btn_show_l.x = int(offset - 5 - this.btn_show_l.width);
         }
      }
   }
}

