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
      
      private var _data:ItemData = null;
      
      private var _inventory:IInventoryItemFactory = null;
      
      private var _cost:Number = 0;
      
      private var _isShopItems:Boolean;
      
      private var _isQuickSlot:Boolean;
      
      private var _isStack:Boolean;
      
      private var _money_type:int = 0;
      
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
      
      public function toggleShop(param1:Boolean) : void
      {
         _shopMode = param1;
         this.updateButtons();
         this.sortButton();
      }
      
      private function configUI(param1:Event) : void
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
         var _loc2_:Timer = MainScene.getTimerInstance();
         _loc2_.addEventListener(TimerEvent.TIMER,this.onTimerUpdate);
         this.btnSellBig.width = this.btn_show_l.width = this.btn_show_r.width = this.btn_show_use.width = 140;
      }
      
      private function onTimerUpdate(param1:TimerEvent) : void
      {
         var _loc2_:Timer;
         var _loc3_:uint;
         if(this._data && this._data.rerollTime)
         {
            _loc2_ = MainScene.getTimerInstance();
            _loc3_ = (_loc2_.currentCount);
            this.rerollTimer.setTime(Util.getTimeStringMinutes(Math.max(0,int(this._data.rerollTime / 60) - (_loc3_ - this._data.timeDelta))));
         }
      }
      
      private function onLocalization(param1:String) : void
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
      
      public function setItemControl(param1:*) : void
      {
         _itemCard = param1;
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
      
      private function craftItem(param1:ButtonEvent) : void
      {
         dispatchEvent(new Event(Events.CRAFFT_ITEM));
      }
      
      public function setInvenory(param1:IInventoryItemFactory) : void
      {
         _inventory = param1;
      }
      
      private function openModifiersWindow(param1:Event) : void
      {
         (this._inventory as IEventDispatcher).dispatchEvent(new Event(Events.OPEN_MODIFIERS));
      }
      
      public function toggleModifiersButtons() : void
      {
         _locked = !this._locked;
         this.updateButtons();
      }
      
      public function updateData(param1:ItemData) : void
      {
         var _loc4_:int = 0;
         var _loc8_:uint = 0;
         var _loc9_:uint = 0;
         var _loc10_:uint = 0;
         var _loc11_:uint = 0;
         var _loc12_:Object = null;
         this._data = param1;
         if(!this._data)
         {
            this.checkButtonEnabled();
            this.updateButtons();
            return;
         }
         var _loc2_:Boolean = this._data.canBuy;
         this._isShopItems = this._data.descriptor == 200 || this._data.descriptor == 300;
         if(!this._isShopItems)
         {
            _loc2_ = this._data.canBuy;
         }
         this._isQuickSlot = this._data.descriptor < 100 && this._data.slotId > 12;
         this._isStack = this._data.isStack;
         if(this._isShopItems)
         {
            this._cost = this._data.cost;
         }
         else
         {
            _loc8_ = this._data.isPremium ? uint(this._data.cost * PlayerInfo.premiumExchangeRate) : this._data.cost;
            _loc9_ = _loc8_ / 4 * (this._data.isStack ? 1 : this._data.value / 100);
            this._cost = _loc9_;
         }
         this._step = this._data.clipSize == 0 ? 1 : int(this._data.clipSize);
         this.btn_show_r.enabled = !this._isShopItems;
         this.btn_show_l.enabled = _loc2_;
         this.btn_show_r.alpha = this.btn_show_r.enabled ? 1 : 0.5;
         this.btn_show_l.alpha = this.btn_show_l.enabled ? 1 : 0.5;
         var _loc3_:Boolean = false;
         this._rerollable = false;
         if(!this._data.isStack && Boolean(this._data.modifiers))
         {
            _loc10_ = this._data.modifiers.length;
            _loc11_ = 0;
            while(_loc11_ < _loc10_)
            {
               _loc12_ = this._data.modifiers[_loc11_];
               if(Boolean(_loc12_) && _loc12_.state == 1)
               {
                  _loc3_ = true;
               }
               if(Boolean(_loc12_) && Boolean(_loc12_.state == 0) && !_loc12_.isDefault)
               {
                  this._rerollable = true;
               }
               _loc11_ = ++_loc11_;
            }
         }
         if(this._rerollable && Boolean(this._data.rerollTime))
         {
            if(Boolean(this._data.rerollTime))
            {
               this.onTimerUpdate(null);
               this.rerollTimer.visible = true;
               this.btn_item_reroll.label = "";
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
         this.btn_item_fix.enabled = !this._isShopItems && (this._data.value != 100 || _loc3_) && this._data.enabled;
         this.btn_item_sell.data = {"name":this.btn_item_sell.label};
         this.btn_item_buy.data = {"name":this.btn_item_buy.label};
         this.btn_item_fix.data = {"name":this.btn_item_fix.label};
         trace("ICPData:",this._data.name,_loc2_,"isShopItems::",this._isShopItems);
         this.updateButtons();
         this.btn_item_sell.tooltipRendererName = "SrvProfileTipRenderer";
         this.btn_item_buy.tooltipRendererName = "SrvProfileTipRenderer";
         this.btn_item_sell.tipData = {"name":LocalizationData.getName("cost") + ": " + this._cost.toString()};
         this.btn_item_buy.tipData = {"name":LocalizationData.getName("cost") + ": " + this._cost.toString()};
         this.btnSellBig.label = LocalizationData.getName("buy");
         if(this._isShopItems)
         {
            _loc4_ = this._data.cost < 1 ? int(Math.min(Math.max(0,int((PlayerInfo.MAX_FREE_ITEMS_COUNT - this._data.value) / this._step)),100)) : int((this._data.isPremium ? PlayerInfo.premiumMoney : PlayerInfo.genericMoney) / this._data.cost);
         }
         else
         {
            _loc4_ = Math.min(100 * this._step,this._data.isStack ? this._data.value : 1);
         }
         var _loc5_:Boolean = !this._data.canBuy && Boolean(this._data.premiumCost);
         this.btn_item_upgrade.enabled = !this._isShopItems && !this._isStack && this.isUpgradeable(this._data) && this._data.enabled;
         this.btn_item_upgrade.alpha = this.btn_item_upgrade.enabled ? 1 : 0.5;
         this.btn_item_upgrade.label = UpgradeWindow.isWeaponCategory(int(this._data.category)) ? LocalizationData.getName("scope_window_title") : LocalizationData.getName("btn_upgrade_label");
         var _loc6_:uint = this._isShopItems ? 100 : this._data.value;
         var _loc7_:Number = _loc6_ / 100;
         this.btn_item_fix.progress = _loc7_;
         this.btn_item_fix.progressLabel = _loc6_.toString() + "%";
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
      
      private function isUpgradeable(param1:Object) : Boolean
      {
         var _loc2_:int = int(param1.category);
         return (_loc2_ == 3 || _loc2_ == 1 || _loc2_ == 5 || _loc2_ == 4) && (Boolean(this._data.camoAvailable) || Boolean(this._data.decalAvailable)) || UpgradeWindow.isWeaponCategory(_loc2_) && (UpgradeWindow.hasScopes(int(param1.dictId)) || Boolean(this._data.camoAvailable) || Boolean(this._data.decalAvailable));
      }
      
      public function checkButtonEnabled(param1:Event = null) : void
      {
         if(!this._data)
         {
            this.btnSellBig.enabled = this.btn_item_sell.enabled = this.btn_item_buy.enabled = this.btn_item_upgrade.enabled = this.btn_item_fix.enabled = this.btn_item_reroll.enabled = false;
            this.btn_item_craft.enabled = false;
            this.btn_item_craft.alpha = this.btn_item_reroll.alpha = this.btn_item_upgrade.alpha = this.btn_item_buy.alpha = this.btn_item_sell.alpha = 0.5;
            return;
         }
         var _loc2_:Boolean = !this._inventory.getElelementLocked(1);
         var _loc3_:Boolean = !this._inventory.getElelementLocked(2);
         var _loc4_:Boolean = !this._data.canBuy && this._data.premiumCost;
         var _loc5_:Boolean = this._data.isPremium || !this._data.isPremium && PlayerInfo.genericMoney >= this._data.cost;
         this.btn_item_sell.enabled = this._data.enabled && _loc3_;
         this.btn_item_buy.enabled = (this._data.canBuy || _loc4_) && _loc2_ && _loc5_;
         this.btnSellBig.enabled = this.btn_item_buy.enabled;
         this.btnSellBig.alpha = this.btnSellBig.enabled ? 1 : 0.5;
         this.btn_item_buy.alpha = this.btn_item_buy.enabled ? 1 : 0.5;
         this.btn_item_sell.alpha = this.btn_item_sell.enabled ? 1 : 0.5;
         this.btn_item_reroll.alpha = this.btn_item_reroll.enabled ? 1 : 0.5;
         this.btn_item_craft.alpha = this.btn_item_craft.enabled ? 1 : 0.5;
         this.btn_item_upgrade.alpha = this.btn_item_upgrade.enabled ? 1 : 0.5;
         var _loc6_:Boolean = this._data.canBuy;
         if(!this._isShopItems)
         {
            _loc6_ = this._data.canBuy;
         }
         this.btn_show_r.enabled = !this._isShopItems && _loc2_ && _loc3_;
         this.btn_show_l.enabled = (_loc6_ || _loc4_) && _loc2_ && _loc3_;
         this.btn_show_r.alpha = this.btn_show_r.enabled ? 1 : 0.5;
         this.btn_show_l.alpha = this.btn_show_l.enabled ? 1 : 0.5;
         trace("ICP:checkButtonEnabled","canBuy",this._data.canBuy,_loc6_,"lockedPremium::",_loc4_,this.btn_show_l.enabled,this.btn_item_sell.enabled,"locked::",this._inventory.getElelementLocked(2),this._inventory.getElelementLocked(1));
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
         var _loc1_:uint;
         this.btnSellBig.x = (this.backgroundWidth - 140) / 2;
         if(this.btn_show_use.visible)
         {
            _loc1_ = 140;
            this.btn_show_l.x = int((width - _loc1_ * 3) / 2);
            this.btn_show_r.x = int(this.btn_show_l.x + _loc1_ + 10);
            this.btn_show_use.x = int(this.btn_show_r.x + _loc1_ + 10);
            this.btn_show_use.y = this.btn_show_l.y;
         }
         else
         {
            _loc1_ = this.backgroundWidth / 2;
            this.btn_show_r.x = int(_loc1_ + 5);
            this.btn_show_l.x = int(_loc1_ - 5 - this.btn_show_l.width);
         }
      }
   }
}

