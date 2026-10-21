package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.DragEvent;
   import scaleform.gfx.TextFieldEx;
   import survarium.Util;
   import survarium.buttons.CheckBoxValue;
   import survarium.buttons.ModificatorListRenderer;
   import survarium.constants.Colors;
   import survarium.constants.Events;
   import survarium.controls.InventoryList;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.item.ItemFactory;
   
   public class MessageCraftItem extends MessageAccept
   {
      
      protected static var _craftChance:Array = [1,2,3,4,5,6,7,8,9,10,11,12];
      
      protected static var _pemiumCostArray:Array = [];
      
      protected static var _pemiumCost:uint;
      
      protected static var _regularCostArray:Array = [];
      
      protected static var _regularCost:uint;
      
      public var background:MovieClip;
      
      public var buttonsBack:MovieClip;
      
      public var dragItemSlot1:MovieClip;
      
      public var dragItemSlot2:MovieClip;
      
      public var dragItemSlot3:MovieClip;
      
      public var lockBackground:MovieClip;
      
      public var rerollContainer:MovieClip;
      
      public var wheels:MovieClip;
      
      public var corners:MovieClip;
      
      public var craftTimer:MovieClip;
      
      public var checkBoxRegularCost:CheckBoxValue;
      
      public var checkBoxPremiumCost:CheckBoxValue;
      
      public var labelText:TextField;
      
      public var labelSuccessChance:TextField;
      
      public var labelFailChance:TextField;
      
      public var labelRegularMoney:TextField;
      
      public var labelGoldMoney:TextField;
      
      public var successChanceRegular:TextField;
      
      public var failChanceRegular:TextField;
      
      public var successChanceGold:TextField;
      
      public var failChanceGold:TextField;
      
      public var decriptionLabel:TextField;
      
      private var _matchState:int = 0;
      
      protected var _craftAvailable:Boolean = false;
      
      public var labelNewModifier:TextField;
      
      public var newModiferRenderer:ModificatorListRenderer;
      
      public function MessageCraftItem()
      {
         super();
         var _loc1_:Class = getDefinitionByName("SrvBasePDRenderer") as Class;
         if(!_loc1_)
         {
            return;
         }
         dragItemSlot1 = new _loc1_();
         dragItemSlot2 = new _loc1_();
         dragItemSlot3 = new _loc1_();
         var _loc2_:int = 25;
         this.dragItemSlot1.x = 24 + _loc2_;
         this.dragItemSlot1.y = 44;
         this.dragItemSlot2.x = 208 + _loc2_;
         this.dragItemSlot2.y = 44;
         this.dragItemSlot3.x = 112 + _loc2_;
         this.dragItemSlot3.y = 44;
         addChildAt(this.dragItemSlot1,numChildren - 3);
         addChildAt(this.dragItemSlot2,numChildren - 3);
         addChildAt(this.dragItemSlot3,numChildren - 3);
      }
      
      public static function setCraftChances(param1:Array) : void
      {
         _craftChance = param1;
      }
      
      public static function setCraftRegularCost(param1:Array) : void
      {
         trace("setCraftRegularCost",param1);
         _regularCostArray = param1;
      }
      
      public static function setCraftPremiumCost(param1:Array) : void
      {
         _pemiumCostArray = param1;
      }
      
      public function setMatchState(param1:int) : void
      {
         var _loc2_:ItemData;
         var _loc3_:Boolean;
         trace("MCI::",this._matchState);
         _matchState = param1;
         if(!parent)
         {
            return;
         }
         if(this._matchState == 1 && !this.dragItemSlot3.visible)
         {
            _loc3_ = false;
            _loc2_ = this.dragItemSlot1.data;
            if(_loc2_ && _loc2_.equipped)
            {
               _loc3_ = true;
               this.dragItemSlot1.data = null;
            }
            _loc2_ = this.dragItemSlot2.data;
            if(_loc2_ && _loc2_.equipped)
            {
               _loc3_ = true;
               this.dragItemSlot2.data = null;
            }
            if(_loc3_ == true)
            {
               Message.showMessage("ok",LocalizationData.getName("craft_message_equipped_item"));
            }
         }
      }
      
      override protected function cleanUp(param1:Event = null) : void
      {
         super.cleanUp();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         btnAccept.label = LocalizationData.getName("btn_item_craft");
         this.background.height = 415;
         this.rerollContainer.y += 1;
         this.updateLayout();
         tabEnabled = true;
         btnAccept.enabled = false;
         if(_data)
         {
            trace("in:data::",this.newModiferRenderer,this.labelNewModifier);
            this.lockWindow(false);
            this.craftTimer.visible = false;
            this.newModiferRenderer.visible = false;
            this.labelNewModifier.visible = false;
            TextFieldEx.setVerticalAlign(this.decriptionLabel,TextFieldEx.VALIGN_CENTER);
            if(this.dragItemSlot1)
            {
               this.dragItemSlot1.label = LocalizationData.getName("item_craft_main_slot");
            }
            if(this.dragItemSlot2)
            {
               this.dragItemSlot2.label = LocalizationData.getName("item_craft_donor_slot");
            }
            this.dragItemSlot1.dragEnabled = true;
            this.labelText.text = LocalizationData.getName("item_craft_label");
            this.decriptionLabel.text = LocalizationData.getName("item_craft_description");
            this.labelSuccessChance.text = LocalizationData.getName("item_craft_success_label");
            this.labelFailChance.text = LocalizationData.getName("item_craft_fail_label");
            this.labelRegularMoney.text = LocalizationData.getName("regular_money");
            this.labelGoldMoney.text = LocalizationData.getName("premium_money");
            this.dragItemSlot1.selected = false;
            this.dragItemSlot2.selected = false;
            this.dragItemSlot3.selected = false;
            this.dragItemSlot2.data = null;
            this.dragItemSlot1.addEventListener(Events.DROPED_IN,this.onPaperDollSlot);
            this.dragItemSlot2.addEventListener(Events.DROPED_IN,this.onPaperDollSlot);
            this.dragItemSlot1.dispatchEvent(new Event(Events.DROPED_IN));
            this.dragItemSlot3.dragEnabled = false;
            this.dragItemSlot3.visible = false;
            this.checkBoxPremiumCost.toggle = false;
            this.checkBoxRegularCost.addEventListener(ButtonEvent.CLICK,this.unBtnClicked);
            this.checkBoxPremiumCost.addEventListener(ButtonEvent.CLICK,this.unBtnClicked);
            this.checkBoxRegularCost.selected = true;
            this.checkBoxPremiumCost.selected = false;
            this.checkBoxRegularCost.money_type = 0;
            this.checkBoxPremiumCost.money_type = 1;
            this.checkBoxRegularCost.toggle = false;
            this.checkBoxRegularCost.value = _regularCost;
            this.checkBoxPremiumCost.value = _pemiumCost;
         }
         this.updateCraftChance();
         this.checkAcceptButton();
      }
      
      private function updateLayout() : void
      {
         var _loc1_:int = 48;
         var _loc2_:int = this.background.height - 3;
         this.lockBackground.height = this.background.height;
         btnAccept.y = _loc2_ + 4 - _loc1_;
         btnCancel.y = btnAccept.y;
      }
      
      private function onPaperDollSlot(param1:Event) : void
      {
         var _loc2_:ItemData;
         trace("onPaperDollSlot",param1.target);
         trace("items",this.dragItemSlot1.data ? this.dragItemSlot1.data.itemId : null,this.dragItemSlot2.data ? this.dragItemSlot2.data.itemId : null);
         if(!this.dragItemSlot3.visible)
         {
            if(this.dragItemSlot1.data && this.dragItemSlot2.data)
            {
               this.decriptionLabel.text = LocalizationData.getName("item_craft_description");
            }
            else
            {
               this.decriptionLabel.text = LocalizationData.getName(this.dragItemSlot1.data ? "item_craft_description_choose_donor" : "item_craft_description_choose_item");
            }
         }
         if(param1.target.data)
         {
            _loc2_ = param1.target.data;
            trace("dropData::",_loc2_.name,_loc2_.equipped);
            if(_loc2_.equipped && this._matchState == 1)
            {
               Message.showMessage("ok",LocalizationData.getName("craft_message_equipped_item"));
               param1.target.data = null;
            }
            else if(_loc2_.broken)
            {
               Message.showMessage("ok",LocalizationData.getName("craft_message_broken_item"));
               param1.target.data = null;
            }
            else if(_loc2_.rank >= 3)
            {
               Message.showMessage("ok",LocalizationData.getName("craft_message_modifers_count"));
               param1.target.data = null;
            }
         }
         if(this.dragItemSlot2.data && this.dragItemSlot1.data)
         {
            if(this.dragItemSlot2.data.itemId == this.dragItemSlot1.data.itemId)
            {
               if(param1.target == this.dragItemSlot1)
               {
                  this.dragItemSlot2.data = null;
               }
               else
               {
                  this.dragItemSlot1.data = null;
               }
            }
         }
         this.updateCraftChance();
         this.checkAcceptButton();
      }
      
      protected function updateCraftChance() : void
      {
         var _loc1_:ItemData = this.dragItemSlot1.data;
         var _loc2_:ItemData = this.dragItemSlot2.data;
         var _loc3_:uint;
         var _loc4_:uint;
         var _loc5_:uint = 0;
         var _loc6_:uint = 0;
         var _loc7_:uint = 0;
         var _loc8_:uint = 0;
         var _loc9_:uint;
         var _loc10_:uint;
         _craftAvailable = true;
         if(!_loc1_ || !_loc2_)
         {
            _loc3_ = 0;
            _loc4_ = 0;
            _craftAvailable = false;
         }
         else
         {
            _loc3_ = _loc1_.rank;
            _loc4_ = _loc2_.rank;
            if(_loc3_ < _loc4_)
            {
               _craftAvailable = false;
               this.onWrongModifiers();
            }
            else
            {
               _loc9_ = (_loc3_ - _loc4_) * 4;
               _loc5_ = _craftChance[_loc9_];
               _loc6_ = _craftChance[_loc9_ + 1];
               _loc7_ = _craftChance[_loc9_ + 2];
               _loc8_ = _craftChance[_loc9_ + 3];
            }
         }
         this.successChanceRegular.text = _loc5_ + "%";
         this.failChanceRegular.text = _loc6_ + "%";
         this.successChanceGold.text = _loc7_ + "%";
         this.failChanceGold.text = _loc8_ + "%";
         if(_loc1_ || _loc2_)
         {
            _loc10_ = _loc1_ ? _loc1_.itemLevel : (_loc2_ ? _loc2_.itemLevel : 0);
            _pemiumCost = _pemiumCostArray[_loc10_];
            _regularCost = _regularCostArray[_loc10_];
         }
         else
         {
            _pemiumCost = 0;
            _regularCost = 0;
         }
         this.checkBoxRegularCost.value = _regularCost;
         this.checkBoxRegularCost.invalidateState();
         this.checkBoxRegularCost.validateNow();
         this.checkBoxPremiumCost.value = _pemiumCost;
         this.checkBoxPremiumCost.invalidateState();
         this.checkBoxPremiumCost.validateNow();
      }
      
      public function onDrag(param1:DragEvent) : void
      {
         trace("UW::onDrag",param1);
         if(param1 == null || param1.dragData == null)
         {
            return;
         }
         if(param1.type == "dragStart")
         {
            this.dragItemSlot2.highlighted = true;
            this.dragItemSlot1.highlighted = true;
         }
         else
         {
            this.dragItemSlot2.highlighted = false;
            this.dragItemSlot1.highlighted = false;
         }
      }
      
      private function unBtnClicked(param1:Event) : void
      {
         this.checkBoxRegularCost.selected = param1.target == this.checkBoxRegularCost;
         this.checkBoxPremiumCost.selected = param1.target == this.checkBoxPremiumCost;
         this.checkAcceptButton();
      }
      
      protected function checkAcceptButton(param1:Event = null) : void
      {
         var _loc2_:Boolean = true;
         var _loc3_:ItemData;
         var _loc4_:ItemData;
         if(this.dragItemSlot1.data && this.dragItemSlot2.data)
         {
            _loc3_ = this.dragItemSlot1.data;
            _loc4_ = this.dragItemSlot2.data;
            trace("checkAcceptButton::",_loc3_.broken,_loc4_.broken);
            if(_loc3_.broken || _loc4_.broken)
            {
               _loc2_ = false;
            }
            else if(_loc3_.rank == 3 || _loc4_.rank == 3)
            {
               _loc2_ = false;
            }
            else if(_loc3_.repairTime || _loc4_.repairTime)
            {
               _loc2_ = false;
            }
            else if(PlayerInfo.genericMoney < _regularCost && this.checkBoxRegularCost.selected)
            {
               _loc2_ = false;
            }
         }
         btnAccept.enabled = this._craftAvailable && _loc2_;
      }
      
      public function fillCraftData(param1:Object) : void
      {
         this.craftTimer.setTime(Util.getTimeStringMinutes(int(param1.craft_time / 60)));
         trace("fillCraftData::",param1.regular_cost,param1.premium_cost);
      }
      
      public function fillCraftedItem(param1:Object, param2:int, param3:Object) : void
      {
         trace("fillCraftedItem::",param2);
         this.dragItemSlot3.data = param1;
         this.dragItemSlot3.visible = true;
         this.dragItemSlot1.visible = this.dragItemSlot2.visible = false;
         if(param2 == 0)
         {
            this.labelNewModifier.text = "";
         }
         else
         {
            this.labelNewModifier.text = param2 == 1 ? LocalizationData.getName("new_modifier") : LocalizationData.getName("lost_modifier");
         }
         this.labelNewModifier.textColor = param2 == 1 ? Colors.SOFT_GRAY : Colors.RED;
         this.newModiferRenderer.data = param3;
         this.newModiferRenderer.visible = param3 != null;
         this.labelNewModifier.visible = true;
         this.labelSuccessChance.visible = false;
         this.labelFailChance.visible = false;
         this.labelRegularMoney.visible = false;
         this.labelGoldMoney.visible = false;
         this.successChanceRegular.visible = false;
         this.failChanceRegular.visible = false;
         this.failChanceGold.visible = false;
         this.successChanceGold.visible = false;
         this.checkBoxRegularCost.visible = false;
         this.craftTimer.visible = false;
         this.checkBoxPremiumCost.visible = false;
         btnCancel.visible = false;
         btnAccept.x = (width - btnAccept.width) / 2;
         btnAccept.label = LocalizationData.getName("btn_craft_accept");
         if(param2 == 0)
         {
            this.rerollContainer.background.height = 55;
            this.labelNewModifier.visible = false;
            this.background.height = 307;
            this.decriptionLabel.text = LocalizationData.getName("craft_result_neutral_label");
            this.updateLayout();
         }
         else
         {
            this.rerollContainer.background.height = 51;
            this.decriptionLabel.text = param2 == 1 ? LocalizationData.getName("craft_result_success_label") : LocalizationData.getName("craft_result_fail_label");
            this.background.height = 341;
            this.updateLayout();
         }
         this.decriptionLabel.textColor = param2 == 1 ? Colors.GOLD : Colors.SOFT_GRAY;
         this.lockWindow(false);
         if(param2 == -2)
         {
            Message.showMessage("ok",LocalizationData.getName("cannot_craft_item"));
            dispatchEvent(new Event("closeCraft"));
         }
         else
         {
            this.offerContinue();
         }
      }

      protected function goToStore() : void
      {
         MessageMoneyExchange.buyGold();
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
      
      protected function dropItem() : void
      {
         this.dragItemSlot2.data = null;
         this.dragItemSlot2.dispatchEvent(new Event(Events.DROPED_IN));
      }
      
      protected function swapItems() : void
      {
         var _loc1_:Object = this.dragItemSlot1.data;
         this.dragItemSlot1.data = this.dragItemSlot2.data;
         this.dragItemSlot2.data = _loc1_;
         this.dragItemSlot2.dispatchEvent(new Event(Events.DROPED_IN));
      }
      
      protected function onWrongModifiers() : void
      {
         var _loc1_:Object = {
            text:LocalizationData.getName("craft_message_modifiers_difference"),
            onYes:this.swapItems,
            onNo:this.dropItem,
            label_ok:LocalizationData.getName("ok"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",_loc1_,true);
      }
      
      private function lockWindow(param1:Boolean) : void
      {
         this.wheels.playAnimation(param1);
         this.wheels.visible = param1;
         this.lockBackground.visible = param1;
         mouseEnabled = !param1;
         mouseChildren = !param1;
         if(param1)
         {
            btnAccept.enabled = false;
         }
         else
         {
            this.checkAcceptButton();
         }
      }
      
      override protected function onMessageAccept(param1:ButtonEvent = null) : void
      {
         var _loc2_:uint;
         if(!btnAccept.enabled)
         {
            return;
         }
         trace("onMessageAccept",this.dragItemSlot1.data,this.dragItemSlot2.data);
         if(this.dragItemSlot1.data && this.dragItemSlot2.data && !this.dragItemSlot3.visible)
         {
            _loc2_ = uint(this.checkBoxPremiumCost.selected ? 1 : 0);
            if(_loc2_ == 1 && _pemiumCost > PlayerInfo.premiumMoney)
            {
               this.onNoMoneyEnought();
            }
            else
            {
               ExternalInterface.call("craft_item",this.dragItemSlot1.data.itemId,this.dragItemSlot2.data.itemId,_loc2_);
               trace("craft_item",this.dragItemSlot1.data.itemId,this.dragItemSlot2.data.itemId,_loc2_);
               this.lockWindow(true);
            }
            return;
         }
      }
      
      private function testResult() : void
      {
         parent["fill_crafted_item"]({
            dict_id:this.dragItemSlot1.data.dictId,
            new_modifier:null,
            rarity_level:1,
            result:-2
         });
      }
      
      // The result had one button, "Ready", which closed the window. It has
      // two: Exit (Cancel, which the lobby wires to close it, as Ready did)
      // and Continue (the Ready button: the window again, for the same item,
      // upgraded, with an empty donor slot).
      private function offerContinue() : void
      {
         // The game's words: "Exit" (btn_exit_label) and "Continue"
         // (btn_go_to_online_store, the store dialog's own Continue).
         btnCancel.label = LocalizationData.getName("btn_exit_label");
         btnAccept.label = LocalizationData.getName("btn_go_to_online_store");
         btnCancel.visible = true;
         btnAccept.enabled = true;
         // The game's order (MessageApply, MessageAsk, MessageExit): the
         // accepting button left, Cancel right.
         var middle:Number = width / 2;
         btnAccept.x = middle - btnAccept.width - 4;
         btnCancel.x = middle + 4;
         btnAccept.addEventListener(ButtonEvent.CLICK,this.onContinue);
      }
      
      private function onContinue(e:ButtonEvent = null) : void
      {
         var main:Object = this.dragItemSlot1.data;
         var list:InventoryList = parent != null ? parent["inventory_list"] as InventoryList : null;
         dispatchEvent(new Event("closeCraft"));
         if(main == null || list == null)
         {
            return;
         }
         // The item as the inventory has it now, upgraded; one with three
         // modifications, or that cannot be crafted, ends here as Exit.
         var item:ItemData = list.getInventoryItem(main.itemId);
         // As the game's own Upgrade entry: owned and enabled.
         if(item == null || !item.enabled || item.rank >= 3 || !ItemFactory.isItemCraftable(item))
         {
            return;
         }
         list.setPrimaryItem(item);
         list.dispatchEvent(new Event(Events.CRAFFT_ITEM));
      }
      
      // The item being upgraded or the donor.
      public function holds(itemId:uint) : Boolean
      {
         var main:Object = this.dragItemSlot1.data;
         var donor:Object = this.dragItemSlot2.data;
         return main != null && main.itemId == itemId || donor != null && donor.itemId == itemId;
      }
   }
}

