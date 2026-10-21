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
   import survarium.messages.MessageMoneyExchange;
   import survarium.upgrade.UpgradeWindow;
   
   public class ItemCard extends MovieClip
   {
      
      protected const BORDER_OFFSET:uint = 9;
      
      protected const BUTTON_MARGIN:uint = 4;
      
      protected var _cost:Number = 0;
      
      protected var _data:ItemData;
      
      protected var isShopItems:Boolean;
      
      protected var isQuickSlot:Boolean;
      
      protected var isStack:Boolean;
      
      protected var _money_type:int = 0;
      
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
      
      public function upgradeItem(param1:ButtonEvent = null) : void
      {
         trace("upgradeItem::",this._data.itemId,this._data.icon,this._data.messIcon);
         if(this._data.broken)
         {
            Message.showMessage("ok",LocalizationData.getName("reroll_message_broken_item"));
            return;
         }
         var _loc2_:ItemData = this._data.clone();
         _loc2_.itemId = this._data.itemId;
         _loc2_.name = this._data.name;
         _loc2_.icon = this._data.messIcon;
         _loc2_.rank = this._data.rank;
         _loc2_.value = this._data.value;
         _loc2_.factionId = this._data.factionId;
         _loc2_.rerollTime = this._data.rerollTime;
         upgradeMessage = Message.showMessage("upgrade_item",_loc2_,true);
         ExternalInterface.call("item_reroll_opened",this._data.itemId);
      }
      
      private function fillItemModifiersTest() : void
      {
         var _loc1_:Object = new Object();
         _loc1_.modifiers = this._data.modifiers;
         this.fillItemModifiers(_loc1_);
      }
      
      public function fillItemModifiers(param1:Object) : void
      {
         var _loc2_:Array = param1.modifiers;
         trace("fillItemModifiers",_loc2_);
         var _loc3_:ItemData = new ItemData();
         _loc3_.modifiersDefault = [];
         _loc3_.modifiers = _loc2_;
         this._inventory.fillModifiersDescription(_loc3_);
         this.upgradeMessage.updateModifiers(param1);
      }
      
      public function fillItemRerollOptions(param1:Array) : void
      {
         var _loc2_:ItemData = new ItemData();
         _loc2_.modifiersDefault = [];
         _loc2_.modifiers = param1;
         this._inventory.fillModifiersDescription(_loc2_);
         this.upgradeMessage.updateReroll(param1);
      }
      
      public function repairItem(param1:ButtonEvent = null) : void
      {
         trace("repairItem::",this._data.itemId,this._data.icon,this._data.messIcon);
         var _loc2_:ItemData = new ItemData();
         _loc2_.itemId = this._data.itemId;
         _loc2_.name = this._data.name;
         _loc2_.sellerIcon = this._data.sellerIcon ? this._data.sellerIcon.clone() : null;
         _loc2_.icon = this._data.messIcon;
         _loc2_.value = this._data.value;
         _loc2_.modifiers = this.getFailModifiers();
         _loc2_.repairCostRatio = this._data.repairCostRatio;
         _loc2_.cost = this._data.cost;
         var _loc3_:IPlayerProfile = parent["player_profile"];
         Message.showMessage("repair",_loc2_,true);
      }
      
      public function openModifiersWindow(param1:ButtonEvent = null) : void
      {
         dispatchEvent(new Event(Events.OPEN_MODIFIERS));
      }
      
      private function onRollOut(param1:ListEvent) : void
      {
      }
      
      private function onRollOver(param1:ListEvent) : void
      {
         if(param1.itemData)
         {
         }
      }
      
      public function updateSelectedItem(param1:Object) : void
      {
         _selectedItemData = param1.data;
         trace("itemCardUpdateSelectedItemSt::",param1.data);
         if(!this._selectedItemData)
         {
            return;
         }
         trace("updateValueTo::",this._data.name,this._data.itemId,this._selectedItemData.value);
         dispatchEvent(new Event(Event.COMPLETE));
         trace("itemCardUpdateSelectedItem::",this._selectedItemData);
      }
      
      protected function closeSelection(param1:Event) : void
      {
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      protected function cancelAction(param1:Event) : void
      {
      }
      
      public function onBtnClick1() : void
      {
         var _loc1_:Object;
         var _loc2_:IPlayerProfile;
         var _loc3_:int;
         trace("IemCard::onBtnClick1",this._data);
         if(this._data)
         {
            _loc1_ = this._data;
            _loc2_ = parent["player_profile"];
            _loc3_ = _loc1_.category == 100 ? (_loc1_.cost) : (GangShop.getInstance().getItemPrice(_loc1_.dictId));
            GangShop.buyFunction(_loc2_.profileId,_loc1_.itemId,_loc1_.dictId,_loc1_.descriptor,100,_loc1_.count,_loc3_,_loc1_.seller,!_loc1_.isStack ? this.onItemBuyAccept : null);
         }
      }
      
      protected function onItemUse(param1:* = null) : void
      {
         ExternalInterface.call("item_use",this._data.itemId);
      }
      
      public function onBtnClick3() : void
      {
         var _loc1_:Object;
         if(this._data.usable)
         {
            trace("useElement::",this._data.type,parent);
            if(this._data.type == 0)
            {
               if(parent)
               {
                  parent["statusPanel"].showChangeNameMessage();
               }
               return;
            }
            _loc1_ = {
               text:LocalizationData.getName("use_item_message"),
               onYes:this.onItemUse,
               label_ok:LocalizationData.getName("btn_use_item"),
               label_cancel:LocalizationData.getName("no")
            };
            Message.showMessage("apply",_loc1_);
         }
      }
      
      public function onBtnClick2() : void
      {
         if(!this._data)
         {
            return;
         }
         var _loc1_:ItemData = this._data;
         trace("Try to Sell pr:" + " uid:" + _loc1_.dictId + " cnt:" + _loc1_.value + " iss:" + _loc1_.isStack + " price:" + int(GangShop.getInstance().getItemPrice(_loc1_.dictId)) + " jj:" + _loc1_.itemId);
         var _loc2_:Object = new Object();
         if(!_loc1_.isStack)
         {
            _loc2_.count = 1;
         }
         else
         {
            _loc2_.count = int(_loc1_.value);
         }
         _loc2_.price = this._cost;
         _loc2_.id = _loc1_.itemId;
         _loc2_.uid = _loc1_.dictId;
         _loc2_.sell = true;
         _loc2_.name = _loc1_.name;
         _loc2_.is_stack = _loc1_.isStack;
         _loc2_.weight = _loc1_.weight;
         if(_loc1_.clipSize)
         {
            _loc2_.clip_size = _loc1_.clipSize;
         }
         trace("sellFromIC::",_loc1_.name,_loc1_.isPremium,this._cost);
         _loc2_.isPremium = _loc1_.isPremium && this.isShopItems;
         _loc2_.icon = (_loc1_.icon as Icon).clone();
         Message.showMessage("asks",_loc2_,true);
      }
      
      public function getItemData() : ItemData
      {
         return this._data;
      }
      
      public function setInventory(param1:InventoryList) : void
      {
         _inventory = param1;
      }
      
      public function setItemData(param1:ItemData) : void
      {
         _data = param1;
         if(this._data != null)
         {
            trace("IC:updateItemData",param1.name,param1.broken);
            alpha = 1;
            this.updateData();
         }
         else
         {
            alpha = 0;
         }
         dispatchEvent(new Event(Event.ACTIVATE));
      }
      
      protected function onSlotUpdate(param1:ListEvent = null) : void
      {
         this.slotUpdate(param1.itemData);
      }
      
      protected function slotUpdate(param1:Object) : void
      {
         var _loc2_:uint;
         var _loc3_:Number;
         if(this.isShopItems)
         {
            _cost = this._data.cost;
         }
         else
         {
            _loc2_ = this._data.isPremium ? (this._data.cost * PlayerInfo.premiumExchangeRate) : this._data.cost;
            trace("IC::itemCost::",_loc2_,this._data.cost,PlayerInfo.premiumExchangeRate);
            _loc3_ = _loc2_ / 4 * (this._data.isStack ? 1 : this._data.value / 100);
            _cost = _loc3_;
         }
         _step = param1.clip_size == undefined ? 1 : (param1.clip_size);
         trace("onSlotUpdate",this._cost,param1.cost);
      }
      
      protected function onItemBuyAccept() : void
      {
         dispatchEvent(new Event(Events.BUY));
      }
      
      protected function buyCurrentItem() : void
      {
         trace("buyCurrentItem");
         dispatchEvent(new Event(Events.BUY));
         ExternalInterface.call("buy_ok_clicked",this._data.dictId,uint(1),uint(this._data.factionId),this._money_type);
      }
      
      public function onBtnClick4(param1:Event = null) : void
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
      
      public function acceptAction(param1:Event = null) : void
      {
         trace("acceptAction");
         if(this.isShopItems)
         {
            if(!this._data.isStack)
            {
               if(this._data.isPremium)
               {
                  if(PlayerInfo.premiumMoney < this._data.cost)
                  {
                     this.onNoMoneyEnought();
                     return;
                  }
               }
               if(this._inventory && !this._inventory.hasItem(this._data.dictId))
               {
                  dispatchEvent(new Event(Events.BUY));
                  ExternalInterface.call("buy_ok_clicked",this._data.dictId,uint(1),uint(this._data.factionId),this._money_type);
               }
               else
               {
                  Message.showMessage("apply",{
                     text:LocalizationData.getName("already_in_inventory"),
                     onYes:this.buyCurrentItem
                  });
               }
            }
            else
            {
               dispatchEvent(new Event(Events.BUY));
               ExternalInterface.call("buy_ok_clicked",this._data.dictId,uint(1),uint(this._data.factionId),this._money_type);
            }
         }
         else
         {
            this.onBtnClick2();
         }
      }
      
      protected function onNoMoneyEnought() : void
      {
         var _loc1_:Object = {
            text:LocalizationData.getName("cant_buy_premium_message"),
            onYes:this.goToStore,
            label_ok:LocalizationData.getName("btn_add_premium_message"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",_loc1_,true);
      }
      
      protected function goToStore() : void
      {
         MessageMoneyExchange.buyGold();
      }
      
      public function checkButtonEnabled(param1:Event = null) : void
      {
         if(!this._data)
         {
            return;
         }
      }
      
      protected function size() : void
      {
      }
      
      protected function updateLayout(param1:Event = null) : void
      {
      }
      
      public function setPropsDescriptor(param1:PropertiesDescriptor) : void
      {
         trace("IC:setPropsDescriptor::",param1);
         _propsDescriptor = param1;
      }
      
      public function setModifiersIcons(param1:IconContainer) : void
      {
         _modifiersIconContainer = param1;
      }
      
      protected function getFailModifiers() : Array
      {
         var _loc1_:Array = this._data.modifiers != null ? this._data.modifiers : [];
         var _loc2_:Array = [];
         var _loc3_:uint = 0;
         var _loc4_:Icon;
         while(_loc3_ < _loc1_.length)
         {
            if(_loc1_[_loc3_].state == 1)
            {
               _loc4_ = this._modifiersIconContainer.getIcon(_loc1_[_loc3_].icon);
               _loc2_.push({
                  name:_loc1_[_loc3_].name,
                  props:_loc1_[_loc3_].props,
                  icon:_loc4_,
                  state:_loc1_[_loc3_].state,
                  fix_time_general:_loc1_[_loc3_].fix_time_general,
                  fix_cost_general:_loc1_[_loc3_].fix_cost_general,
                  fix_cost_gold:_loc1_[_loc3_].fix_cost_gold
               });
            }
            _loc3_++;
         }
         this.fillModifiersNames(_loc2_);
         return _loc2_;
      }
      
      protected function fillModifiersNames(param1:Array) : void
      {
         var _loc2_:uint = 0;
         var _loc3_:String;
         var _loc4_:Object;
         var _loc5_:Object;
         var _loc6_:Object;
         var _loc7_:String;
         var _loc8_:String;
         var _loc9_:Number;
         var _loc10_:String;
         while(_loc2_ < param1.length)
         {
            _loc3_ = "";
            _loc4_ = param1[_loc2_].props;
            trace("props::",_loc3_,_loc4_,this._propsDescriptor);
            for each(_loc5_ in _loc4_)
            {
               _loc6_ = this._propsDescriptor.getPropertyDescription(_loc5_.prop_id);
               if(_loc6_)
               {
                  if(_loc6_.postfix == undefined)
                  {
                     _loc6_.postfix = "";
                  }
                  _loc7_ = _loc6_.modifier_type == 0 ? _loc6_.postfix : "%";
                  _loc8_ = _loc5_.prop_value >= 0 ? "+" : "";
                  if(_loc6_.modifier_type == undefined)
                  {
                     _loc6_.modifier_type = 2;
                     _loc6_.modifier_type = _loc5_.modifier_type;
                  }
                  if(_loc6_.modifier_type == 2)
                  {
                     throw Error("modifier_type at modifier " + param1[_loc2_].name + " for prop_id " + _loc5_.prop_id + " is invalid");
                  }
                  trace("propValue::",_loc6_.direction,_loc5_.direction);
                  _loc9_ = _loc6_.modifier_type == 0 ? (_loc5_.prop_value) : _loc5_.prop_value * 100;
                  _loc10_ = _loc6_.direction == 0 && _loc5_.prop_value < 0 || _loc6_.direction == 1 && _loc5_.prop_value > 0 ? "green" : "red";
                  _loc3_ += _loc6_.prop_name + ": " + Util.styledChar(_loc8_ + (int(_loc9_) == _loc9_ ? _loc9_.toString() : _loc9_.toFixed(1)) + _loc7_,_loc10_) + "\n";
               }
            }
            param1[_loc2_].name = _loc3_;
            _loc2_++;
         }
      }
      
      protected function updateData() : void
      {
         var _loc1_:Boolean = this._data.canBuy;
         var _loc2_:uint;
         var _loc3_:Number;
         isShopItems = this._data.descriptor == 200;
         if(!this.isShopItems)
         {
            _loc1_ = GangShop.getInstance().getItemBuyable(this._data.dictId);
         }
         isQuickSlot = this._data.descriptor < 100 && this._data.slotId > 12;
         isStack = this._data.isStack;
         if(this.isShopItems)
         {
            _cost = this._data.cost;
         }
         else
         {
            _loc2_ = this._data.isPremium ? (this._data.cost * PlayerInfo.premiumExchangeRate) : this._data.cost;
            _loc3_ = _loc2_ / 4 * (this._data.isStack ? 1 : this._data.value / 100);
            trace("IC::itemCost::",_loc2_,this._data.isPremium,this._data.cost,PlayerInfo.premiumExchangeRate,"sellCost::",_loc3_);
            _cost = _loc3_;
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
      
      private function isUpgradeable(param1:Object) : Boolean
      {
         var _loc2_:int = int(param1.category);
         return (_loc2_ == 3 || _loc2_ == 1 || _loc2_ == 5 || _loc2_ == 4) && (Boolean(this._data.camoAvailable) || Boolean(this._data.decalAvailable)) || UpgradeWindow.isWeaponCategory(_loc2_) && (UpgradeWindow.hasScopes(int(param1.dictId)) || Boolean(this._data.camoAvailable));
      }
   }
}

