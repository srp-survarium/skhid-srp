package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.interfaces.IListItemRenderer;
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
   import survarium.constants.CraftResult;
   import survarium.constants.ItemRank;
   import survarium.constants.MatchMakingState;
   import survarium.constants.MoneyType;
   
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
         var rendererClass:Class = getDefinitionByName("SrvBasePDRenderer") as Class;
         if(!rendererClass)
         {
            return;
         }
         dragItemSlot1 = new rendererClass();
         dragItemSlot2 = new rendererClass();
         dragItemSlot3 = new rendererClass();
         var slotOffsetX:int = 25;
         this.dragItemSlot1.x = 24 + slotOffsetX;
         this.dragItemSlot1.y = 44;
         this.dragItemSlot2.x = 208 + slotOffsetX;
         this.dragItemSlot2.y = 44;
         this.dragItemSlot3.x = 112 + slotOffsetX;
         this.dragItemSlot3.y = 44;
         addChildAt(this.dragItemSlot1,numChildren - 3);
         addChildAt(this.dragItemSlot2,numChildren - 3);
         addChildAt(this.dragItemSlot3,numChildren - 3);
         // The item being upgraded and the donor keep the inventory's loadout badge: the
         // list leaves them out while they are in the slots (hideSlotted).
         this.dragItemSlot1.itemRenderer.showLoadouts = true;
         this.dragItemSlot2.itemRenderer.showLoadouts = true;
      }
      
      public static function setCraftChances(chances:Array) : void
      {
         _craftChance = chances;
      }
      
      public static function setCraftRegularCost(costs:Array) : void
      {
         trace("setCraftRegularCost",costs);
         _regularCostArray = costs;
      }
      
      public static function setCraftPremiumCost(costs:Array) : void
      {
         _pemiumCostArray = costs;
      }
      
      public function setMatchState(matchState:int) : void
      {
         var slotItem:ItemData;
         var removedEquipped:Boolean;
         trace("MCI::",this._matchState);
         _matchState = matchState;
         if(!parent)
         {
            return;
         }
         if(this._matchState == MatchMakingState.SEARCHING && !this.dragItemSlot3.visible)
         {
            removedEquipped = false;
            slotItem = this.dragItemSlot1.data;
            if(slotItem && slotItem.equipped)
            {
               removedEquipped = true;
               this.dragItemSlot1.data = null;
            }
            slotItem = this.dragItemSlot2.data;
            if(slotItem && slotItem.equipped)
            {
               removedEquipped = true;
               this.dragItemSlot2.data = null;
            }
            if(removedEquipped == true)
            {
               Message.showMessage("ok",LocalizationData.getName("craft_message_equipped_item"));
            }
            this.hideSlotted();
         }
      }
      
      override protected function cleanUp(event:Event = null) : void
      {
         super.cleanUp();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         btnAccept.label = LocalizationData.getName("btn_item_craft");
         this.background.height = 415;
         this.rerollContainer.y += 1;
         this.removeFailureRow();
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
            this.dragItemSlot1.addEventListener(MouseEvent.ROLL_OVER,this.onSlotOver);
            this.dragItemSlot2.addEventListener(MouseEvent.ROLL_OVER,this.onSlotOver);
            this.dragItemSlot1.addEventListener(MouseEvent.ROLL_OUT,this.onSlotOut);
            this.dragItemSlot2.addEventListener(MouseEvent.ROLL_OUT,this.onSlotOut);
            this.dragItemSlot1.addEventListener(MouseEvent.MOUSE_DOWN,this.onSlotDown);
            this.dragItemSlot2.addEventListener(MouseEvent.MOUSE_DOWN,this.onSlotDown);
            this.dragItemSlot1.addEventListener(MouseEvent.CLICK,this.onSlotClick);
            this.dragItemSlot2.addEventListener(MouseEvent.CLICK,this.onSlotClick);
            this.dragItemSlot1.dispatchEvent(new Event(Events.DROPED_IN));
            this.dragItemSlot3.dragEnabled = false;
            this.dragItemSlot3.visible = false;
            this.checkBoxPremiumCost.toggle = false;
            this.checkBoxRegularCost.addEventListener(ButtonEvent.CLICK,this.unBtnClicked);
            this.checkBoxPremiumCost.addEventListener(ButtonEvent.CLICK,this.unBtnClicked);
            this.checkBoxRegularCost.selected = true;
            this.checkBoxPremiumCost.selected = false;
            this.checkBoxRegularCost.money_type = MoneyType.REGULAR;
            this.checkBoxPremiumCost.money_type = MoneyType.PREMIUM;
            this.checkBoxRegularCost.toggle = false;
            this.checkBoxRegularCost.value = _regularCost;
            this.checkBoxPremiumCost.value = _pemiumCost;
         }
         this.updateCraftChance();
         this.checkAcceptButton();
      }
      
      // An upgrade never takes a modification away (the owner's rule of 2026-10-23, craft-menu.md):
      // no failure ("-1 mod.") row, and the server sends its chances as 0. The chances' panel,
      // the prices under it and the window close up by the row's height; the top stays.
      private function removeFailureRow() : void
      {
         var row:Number = this.labelFailChance.y - this.labelSuccessChance.y;
         this.labelFailChance.visible = false;
         this.failChanceRegular.visible = false;
         this.failChanceGold.visible = false;
         this.rerollContainer.background.height -= row;
         this.checkBoxRegularCost.y -= row;
         this.checkBoxPremiumCost.y -= row;
         this.craftTimer.y -= row;
         this.background.height -= row;
      }
      
      private function updateLayout() : void
      {
         var buttonAreaHeight:int = 48;
         var backgroundBottom:int = this.background.height - 3;
         this.lockBackground.height = this.background.height;
         btnAccept.y = backgroundBottom + 4 - buttonAreaHeight;
         btnCancel.y = btnAccept.y;
      }
      
      private function onPaperDollSlot(event:Event) : void
      {
         var droppedItem:ItemData;
         trace("onPaperDollSlot",event.target);
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
         if(event.target.data)
         {
            droppedItem = event.target.data;
            trace("dropData::",droppedItem.name,droppedItem.equipped);
            if(droppedItem.equipped && this._matchState == MatchMakingState.SEARCHING)
            {
               Message.showMessage("ok",LocalizationData.getName("craft_message_equipped_item"));
               event.target.data = null;
            }
            else if(droppedItem.broken)
            {
               Message.showMessage("ok",LocalizationData.getName("craft_message_broken_item"));
               event.target.data = null;
            }
            else if(droppedItem.rank >= ItemRank.MAX)
            {
               Message.showMessage("ok",LocalizationData.getName("craft_message_modifers_count"));
               event.target.data = null;
            }
         }
         if(this.dragItemSlot2.data && this.dragItemSlot1.data)
         {
            if(this.dragItemSlot2.data.itemId == this.dragItemSlot1.data.itemId)
            {
               if(event.target == this.dragItemSlot1)
               {
                  this.dragItemSlot2.data = null;
               }
               else
               {
                  this.dragItemSlot1.data = null;
               }
            }
            // A donor the server refuses (ItemFactory.isCraftDonor): one
            // of another item.
            else if(!ItemFactory.isCraftDonor(this.dragItemSlot2.data.dictId,this.dragItemSlot1.data.dictId))
            {
               Message.showMessage("ok",LocalizationData.getName("cannot_craft_item"));
               this.dragItemSlot2.data = null;
            }
         }
         this.hideSlotted();
         this.updateCraftChance();
         this.checkAcceptButton();
      }
      
      protected function updateCraftChance() : void
      {
         var mainItem:ItemData = this.dragItemSlot1.data;
         var donorItem:ItemData = this.dragItemSlot2.data;
         var mainRank:uint;
         var donorRank:uint;
         var successRegular:uint = 0;
         var failRegular:uint = 0;
         var successGold:uint = 0;
         var failGold:uint = 0;
         var chanceIndex:uint;
         var costLevel:uint;
         _craftAvailable = true;
         if(!mainItem || !donorItem)
         {
            mainRank = 0;
            donorRank = 0;
            _craftAvailable = false;
         }
         else
         {
            mainRank = mainItem.rank;
            donorRank = donorItem.rank;
            if(mainRank < donorRank)
            {
               _craftAvailable = false;
               this.onWrongModifiers();
            }
            else
            {
               chanceIndex = (mainRank - donorRank) * 4;
               successRegular = _craftChance[chanceIndex];
               failRegular = _craftChance[chanceIndex + 1];
               successGold = _craftChance[chanceIndex + 2];
               failGold = _craftChance[chanceIndex + 3];
            }
         }
         this.successChanceRegular.text = successRegular + "%";
         this.failChanceRegular.text = failRegular + "%";
         this.successChanceGold.text = successGold + "%";
         this.failChanceGold.text = failGold + "%";
         if(mainItem || donorItem)
         {
            costLevel = mainItem ? mainItem.itemLevel : (donorItem ? donorItem.itemLevel : 0);
            _pemiumCost = _pemiumCostArray[costLevel];
            _regularCost = _regularCostArray[costLevel];
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
      
      public function onDrag(event:DragEvent) : void
      {
         trace("UW::onDrag",event);
         if(event == null || event.dragData == null)
         {
            return;
         }
         if(event.type == "dragStart")
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
      
      private function unBtnClicked(event:Event) : void
      {
         this.checkBoxRegularCost.selected = event.target == this.checkBoxRegularCost;
         this.checkBoxPremiumCost.selected = event.target == this.checkBoxPremiumCost;
         this.checkAcceptButton();
      }
      
      protected function checkAcceptButton(event:Event = null) : void
      {
         var canCraft:Boolean = true;
         var mainItem:ItemData;
         var donorItem:ItemData;
         if(this.dragItemSlot1.data && this.dragItemSlot2.data)
         {
            mainItem = this.dragItemSlot1.data;
            donorItem = this.dragItemSlot2.data;
            trace("checkAcceptButton::",mainItem.broken,donorItem.broken);
            if(mainItem.broken || donorItem.broken)
            {
               canCraft = false;
            }
            else if(mainItem.rank == ItemRank.MAX || donorItem.rank == ItemRank.MAX)
            {
               canCraft = false;
            }
            else if(mainItem.repairTime || donorItem.repairTime)
            {
               canCraft = false;
            }
            else if(PlayerInfo.genericMoney < _regularCost && this.checkBoxRegularCost.selected)
            {
               canCraft = false;
            }
         }
         btnAccept.enabled = this._craftAvailable && canCraft;
      }
      
      public function fillCraftData(craftData:Object) : void
      {
         this.craftTimer.setTime(Util.getTimeStringMinutes(int(craftData.craft_time / 60)));
         trace("fillCraftData::",craftData.regular_cost,craftData.premium_cost);
      }
      
      public function fillCraftedItem(craftedItem:Object, result:int, newModifier:Object) : void
      {
         trace("fillCraftedItem::",result);
         this.dragItemSlot3.data = craftedItem;
         this.dragItemSlot3.visible = true;
         this.dragItemSlot1.visible = this.dragItemSlot2.visible = false;
         if(result == CraftResult.NOTHING)
         {
            this.labelNewModifier.text = "";
         }
         else
         {
            this.labelNewModifier.text = result == CraftResult.ADDED ? LocalizationData.getName("new_modifier") : LocalizationData.getName("lost_modifier");
         }
         this.labelNewModifier.textColor = result == CraftResult.ADDED ? Colors.SOFT_GRAY : Colors.RED;
         this.newModiferRenderer.data = newModifier;
         this.newModiferRenderer.visible = newModifier != null;
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
         if(result == CraftResult.NOTHING)
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
            this.decriptionLabel.text = result == CraftResult.ADDED ? LocalizationData.getName("craft_result_success_label") : LocalizationData.getName("craft_result_fail_label");
            this.background.height = 341;
            this.updateLayout();
         }
         this.decriptionLabel.textColor = result == CraftResult.ADDED ? Colors.GOLD : Colors.SOFT_GRAY;
         this.lockWindow(false);
         if(result == CraftResult.FAILED)
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
         var messageData:Object = {
            text:LocalizationData.getName("cant_buy_premium_message"),
            onYes:this.goToStore,
            label_ok:LocalizationData.getName("btn_add_premium_message"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",messageData,true);
      }
      
      protected function dropItem() : void
      {
         this.dragItemSlot2.data = null;
         this.dragItemSlot2.dispatchEvent(new Event(Events.DROPED_IN));
      }
      
      protected function swapItems() : void
      {
         var mainItem:Object = this.dragItemSlot1.data;
         this.dragItemSlot1.data = this.dragItemSlot2.data;
         this.dragItemSlot2.data = mainItem;
         this.dragItemSlot2.dispatchEvent(new Event(Events.DROPED_IN));
      }
      
      protected function onWrongModifiers() : void
      {
         var messageData:Object = {
            text:LocalizationData.getName("craft_message_modifiers_difference"),
            onYes:this.swapItems,
            onNo:this.dropItem,
            label_ok:LocalizationData.getName("ok"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",messageData,true);
      }
      
      private function lockWindow(locked:Boolean) : void
      {
         this.wheels.playAnimation(locked);
         this.wheels.visible = locked;
         this.lockBackground.visible = locked;
         mouseEnabled = !locked;
         mouseChildren = !locked;
         if(locked)
         {
            btnAccept.enabled = false;
         }
         else
         {
            this.checkAcceptButton();
         }
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         var moneyType:uint;
         if(!btnAccept.enabled)
         {
            return;
         }
         trace("onMessageAccept",this.dragItemSlot1.data,this.dragItemSlot2.data);
         if(this.dragItemSlot1.data && this.dragItemSlot2.data && !this.dragItemSlot3.visible)
         {
            moneyType = uint(this.checkBoxPremiumCost.selected ? MoneyType.PREMIUM : MoneyType.REGULAR);
            if(moneyType == MoneyType.PREMIUM && _pemiumCost > PlayerInfo.premiumMoney)
            {
               this.onNoMoneyEnought();
            }
            else
            {
               ExternalInterface.call("craft_item",this.dragItemSlot1.data.itemId,this.dragItemSlot2.data.itemId,moneyType);
               trace("craft_item",this.dragItemSlot1.data.itemId,this.dragItemSlot2.data.itemId,moneyType);
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
            result:CraftResult.FAILED
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
         if(item == null || !item.enabled || item.rank >= ItemRank.MAX || !ItemFactory.isItemCraftable(item))
         {
            return;
         }
         list.setPrimaryItem(item);
         list.dispatchEvent(new Event(Events.CRAFFT_ITEM));
      }
      
      // The item a hovered list item is compared with while this window is open: the item being
      // upgraded (as upgraded, once the result shows); null while the window has none, and for
      // the item itself (craft-menu.md, "Comparing items").
      public function compareWith(item:Object) : Object
      {
         var main:Object = this.dragItemSlot1.data;
         if(main == null || item == null || item.itemId == main.itemId)
         {
            return null;
         }
         var result:ItemData = this.dragItemSlot3.visible ? this.dragItemSlot3.data as ItemData : null;
         if(result == null)
         {
            return main;
         }
         // The upgraded item as the inventory holds it: the result's own card is made from the
         // dictionary (no place, no id, no upgrades), which would read "Equipped" without its scope.
         var upgraded:ItemData = result.clone();
         upgraded.descriptor = main.descriptor;
         upgraded.itemId = main.itemId;
         upgraded.upgrades = main.upgrades;
         return upgraded;
      }

      // The list leaves out the items in the slots, so it shows what is moved (still owned).
      private function hideSlotted() : void
      {
         var list:InventoryList = parent != null ? parent["inventory_list"] as InventoryList : null;
         if(list == null)
         {
            return;
         }
         var ids:Array = [];
         if(this.dragItemSlot1.data)
         {
            ids.push(this.dragItemSlot1.data.itemId);
         }
         if(this.dragItemSlot2.data)
         {
            ids.push(this.dragItemSlot2.data.itemId);
         }
         list.hideCraftSlotted(ids);
      }

      // A slot's item card for the lobby (craft-menu.md, "Comparing items"): ITEM_ROLL_OVER with
      // the slot's item, slotCompareWith the other slot's.
      private function onSlotOver(e:MouseEvent) : void
      {
         var slot:MovieClip = e.currentTarget as MovieClip;
         if(slot == null || slot.data == null || this.dragItemSlot3.visible)
         {
            return;
         }
         dispatchEvent(new ListEvent(ListEvent.ITEM_ROLL_OVER,false,false,-1,-1,-1,slot as IListItemRenderer,slot.data));
      }

      private function onSlotOut(e:MouseEvent) : void
      {
         dispatchEvent(new ListEvent(ListEvent.ITEM_ROLL_OUT,false,false,-1,-1,-1,e.currentTarget as IListItemRenderer,null));
      }

      // Where a press on a slot began, in its units: a press that moved more than the drag's 3
      // (DragSlot.handleMouseMove) is a drag of the donor, not a click.
      private var _slotDownX:Number = 0;

      private var _slotDownY:Number = 0;

      private function onSlotDown(e:MouseEvent) : void
      {
         this._slotDownX = (e.currentTarget as MovieClip).mouseX;
         this._slotDownY = (e.currentTarget as MovieClip).mouseY;
      }

      // A click (either button) on a slot's item: Buy, to buy another of the kind; the window stays
      // as it is (craft-menu.md, "Buy from the slots").
      private function onSlotClick(e:MouseEvent) : void
      {
         var slot:MovieClip = e.currentTarget as MovieClip;
         var list:InventoryList = parent != null ? parent["inventory_list"] as InventoryList : null;
         if(slot == null || slot.data == null || list == null || this.dragItemSlot3.visible)
         {
            return;
         }
         if(Math.abs(slot.mouseX - this._slotDownX) > 3 || Math.abs(slot.mouseY - this._slotDownY) > 3)
         {
            return;
         }
         list.showCraftSlotMenu(slot.data,this);
      }

      // The item a slot's card is compared with: the other slot's (the donor with the item being
      // upgraded, and the other way round); null while that one is empty.
      public function slotCompareWith(slot:Object) : Object
      {
         return slot == this.dragItemSlot1 ? this.dragItemSlot2.data : (slot == this.dragItemSlot2 ? this.dragItemSlot1.data : null);
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

