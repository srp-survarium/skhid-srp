package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.gfx.TextFieldEx;
   import survarium.Util;
   import survarium.buttons.CheckBoxValue;
   import survarium.controls.ButtonBarDynam;
   import survarium.controls.FixedText;
   import survarium.controls.IconRenderer;
   import survarium.controls.ItemRendererBase;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.LocalizationData;
   import survarium.constants.ModificationState;
   import survarium.constants.MoneyType;
   import survarium.constants.RerollChoice;
   import survarium.rooms.RoomsText;

   public class MessageRerollItem extends MessageAccept
   {

      // The cancel button while the three are offered: the item keeps its modification (the change
      // stays paid). English, Ukrainian, Russian, German, Polish.
      private static const KEEP_OLD_LABEL:Array = ["Keep old","Не міняти","Не менять","Behalten","Zostaw stary"];

      public var lockedHint:ToolTip;
      
      public var buttonReroll:Button;
      
      public var buttonCraft:Button;
      
      public var sellerIcon:MovieClip;
      
      public var background:MovieClip;
      
      public var lockBackground:MovieClip;
      
      public var rankIcon:MovieClip;
      
      public var corners:MovieClip;
      
      public var buttonsBack:MovieClip;
      
      public var rerollContainer:MovieClip;
      
      public var wheels:MovieClip;
      
      public var checkBoxRegularCost:CheckBoxValue;
      
      public var checkBoxPremiumCost:CheckBoxValue;
      
      public var iconSlot:IconRenderer;
      
      public var labelText:TextField;
      
      public var labelItemName:TextField;
      
      public var itemName:FixedText;
      
      public var labelChooseModifier:TextField;
      
      public var labelChooseModifier2:TextField;
      
      public var decriptionLabel:TextField;
      
      public var decriptionTimerLabel:TextField;
      
      public var modifiersList:ButtonBarDynam;
      
      public var modifiersList2:ButtonBarDynam;
      
      public var itemRenderer:ItemRendererBase;
      
      private var _rerollTimer:int = 0;
      
      protected var _rerollCost:int = 0;
      
      protected var _rerollCostGold:int = 0;
      
      protected var _itemId:uint = 0;
      
      protected var _modifierSelection:*;
      
      public function MessageRerollItem()
      {
         trace("MessageRerollItem",this.modifiersList,this.modifiersList2,this.checkBoxRegularCost,this.checkBoxPremiumCost,this.wheels,this.rerollContainer,this.buttonReroll,this.buttonCraft,this.lockedHint);
         super();
      }
      
      override protected function cleanUp(event:Event = null) : void
      {
         this.itemRenderer.data = null;
         super.cleanUp();
      }
      
      override protected function configUI() : void
      {
         var rendererClass:Class = getDefinitionByName("SrvItemRendererFull") as Class;
         var modifiers:Array;
         var modifierCount:uint;
         var options:Array;
         var iconContainer:IconContainer;
         var i:uint;
         var modifier:Object;
         var modifierIcon:Icon;
         var option:Object;
         itemRenderer = new rendererClass();
         this.itemRenderer.x = 10;
         this.itemRenderer.y = 42;
         addChildAt(this.itemRenderer,numChildren - 3);
         super.configUI();
         this.modifiersList.setMaxFit(true);
         btnAccept.label = LocalizationData.getName("reroll");
         this.lockedHint.tooltipRendererName = "SrvProfileTipRenderer";
         this.lockedHint.tipData = {name:LocalizationData.getName("reroll_modifier_lock_hint")};
         this.lockedHint.visible = false;
         tabEnabled = true;
         btnAccept.enabled = false;
         if(_data)
         {
            this.lockWindow(true);
            TextFieldEx.setVerticalAlign(this.decriptionLabel,TextFieldEx.VALIGN_CENTER);
            trace("in:data::",this.labelText,this.labelChooseModifier,this.decriptionLabel,this.buttonCraft,this.buttonReroll,this.checkBoxRegularCost,this.checkBoxPremiumCost,this.sellerIcon);
            this.labelText.text = LocalizationData.getName("item_modification");
            this.labelChooseModifier.text = LocalizationData.getName("choose_modifier");
            this.decriptionLabel.text = LocalizationData.getName("modifier_reroll_description");
            // Skhid: no "only one attempt in 6 hours" line: silver rerolls have no wait
            // (items_rules: Reroll, the owner's rule of 2026-10-22)
            this.decriptionTimerLabel.text = "";
            this.labelChooseModifier2.text = LocalizationData.getName("choose_modifier");
            this.checkBoxRegularCost.selected = true;
            this.checkBoxPremiumCost.selected = false;
            this.checkBoxRegularCost.money_type = MoneyType.REGULAR;
            this.checkBoxPremiumCost.money_type = MoneyType.PREMIUM;
            this.checkBoxRegularCost.toggle = false;
            this.checkBoxPremiumCost.toggle = false;
            this.checkBoxRegularCost.addEventListener(ButtonEvent.CLICK,this.unBtnClicked);
            this.checkBoxPremiumCost.addEventListener(ButtonEvent.CLICK,this.unBtnClicked);
            this.checkBoxRegularCost.value = 0;
            this.checkBoxPremiumCost.value = 0;
            this.labelChooseModifier2.visible = this.modifiersList2.visible = false;
            _rerollTimer = _data.rerollTime;
            this.itemRenderer.data = _data;
            _itemId = _data.itemId;
            modifiers = _data.modifiers as Array;
            if(modifiers)
            {
               modifierCount = modifiers.length;
               options = [];
               iconContainer = IconContainer.getInstance("modifiers");
               i = 0;
               while(i < modifierCount)
               {
                  modifier = modifiers[i];
                  if(!modifier.isDefault && modifier.state != ModificationState.DEFECT)
                  {
                     modifierIcon = iconContainer.getIcon(modifier.icon);
                     option = {
                        id:modifier.id,
                        icon:modifierIcon,
                        label:modifier.textValue
                     };
                     options.push(option);
                  }
                  i++;
               }
               trace("modifiersToReroll",modifierCount,options.length);
               this.modifiersList.dataProvider = new DataProvider(options);
            }
            this.modifiersList.addEventListener(IndexEvent.INDEX_CHANGE,this.checkAcceptButton);
            this.modifiersList2.addEventListener(IndexEvent.INDEX_CHANGE,this.checkAcceptButton);
            this.setChooseReollOptionState(false);
         }
         this.checkAcceptButton();
      }
      
      public function updateTimer(rerollInfo:Object) : void
      {
         if(_data && rerollInfo && _data.itemId == rerollInfo.itemId)
         {
            _rerollTimer = rerollInfo.rerollTime;
            this.checkAcceptButton();
         }
      }
      
      public function updateReroll(newModifiers:Array) : void
      {
         this.lockWindow(false);
         var selectedModifier:uint = (this.modifiersList.selectedIndex);
         var modifiers:Array = newModifiers;
         var modifierCount:uint = modifiers.length;
         var iconContainer:IconContainer = IconContainer.getInstance("modifiers");
         var options:Array = [];
         var i:uint = 0;
         var modifier:Object;
         var modifierIcon:Icon;
         var option:Object;
         while(i < modifierCount)
         {
            modifier = modifiers[i];
            modifierIcon = iconContainer.getIcon(modifier.icon);
            option = {
               id:modifier.id,
               icon:modifierIcon,
               label:modifier.textValue
            };
            options.push(option);
            i++;
         }
         this.showModifierSelection(selectedModifier,options);
      }
      
      public function updateModifiers(rerollData:Object) : void
      {
         trace("modifiersToRerollUpgrade",rerollData);
         closable = true;
         Util.logObject(rerollData);
         if(!this.rerollContainer.visible)
         {
            this.setChooseReollOptionState(false);
         }
         var modifiers:Array = rerollData.modifiers as Array;
         var modifierCount:uint;
         var options:Array;
         var iconContainer:IconContainer;
         var noneRerolled:Boolean;
         var rerolledIndex:int;
         var modifier:Object;
         var i:uint;
         var modifierIcon:Icon;
         var option:Object;
         this.lockWindow(false);
         _rerollCost = rerollData.regular_cost;
         _rerollCostGold = rerollData.premium_cost;
         this.checkBoxRegularCost.value = this._rerollCost;
         this.checkBoxPremiumCost.value = this._rerollCostGold;
         if(modifiers)
         {
            modifierCount = modifiers.length;
            options = [];
            iconContainer = IconContainer.getInstance("modifiers");
            noneRerolled = modifierCount > 0;
            rerolledIndex = -1;
            i = 0;
            while(i < modifierCount)
            {
               modifier = modifiers[i];
               if(modifier.was_rerolled_before)
               {
                  noneRerolled = false;
                  rerolledIndex = (i);
               }
               i++;
            }
            i = 0;
            while(i < modifierCount)
            {
               modifier = modifiers[i];
               modifierIcon = iconContainer.getIcon(modifier.icon);
               option = {
                  id:modifier.id,
                  icon:modifierIcon,
                  label:modifier.textValue,
                  reroll_available:!modifier.was_rerolled_before,
                  locked:rerolledIndex == i && noneRerolled == false
               };
               options.push(option);
               i++;
            }
            trace("modifiersToReroll",modifierCount,options.length,rerolledIndex,noneRerolled);
            this.modifiersList.dataProvider = new DataProvider(options);
            if(!noneRerolled && rerolledIndex != -1)
            {
               this.modifiersList.selectedIndex = rerolledIndex;
               this.modifiersList.enabled = false;
               this.modifiersList.mouseEnabled = false;
               this.modifiersList.mouseChildren = false;
               this.lockedHint.visible = true;
            }
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
         var selectedModifier:int;
         var moneyType:uint;
         if(this.rerollContainer.visible)
         {
            moneyType = this.getSelectedIndex();
            selectedModifier = this.modifiersList.selectedIndex;
            if(selectedModifier == -1)
            {
               btnAccept.enabled = false;
               return;
            }
            if(moneyType == MoneyType.REGULAR)
            {
               btnAccept.enabled = this._rerollCost <= PlayerInfo.genericMoney && !this._rerollTimer;
            }
            else if(moneyType == MoneyType.PREMIUM)
            {
               btnAccept.enabled = true;
            }
         }
         else
         {
            selectedModifier = this.modifiersList2.selectedIndex;
            if(selectedModifier == -1)
            {
               btnAccept.enabled = false;
            }
            else
            {
               btnAccept.enabled = true;
            }
         }
      }
      
      protected function getSelectedIndex() : uint
      {
         var moneyType:uint;
         if(this.checkBoxRegularCost.selected)
         {
            moneyType = MoneyType.REGULAR;
         }
         else
         {
            moneyType = MoneyType.PREMIUM;
         }
         return moneyType;
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
      
      protected function showModifierSelection(selectedModifier:uint, options:Array) : void
      {
         this.setChooseReollOptionState(true);
         this.lockWindow(false);
         this.modifiersList2.dataProvider = new DataProvider(options);
         btnAccept.enabled = false;
      }
      
      private function setChooseReollOptionState(choosing:Boolean) : void
      {
         this.rerollContainer.visible = !choosing;
         this.labelChooseModifier2.visible = choosing;
         this.modifiersList2.selectedIndex = -1;
         this.modifiersList2.visible = choosing;
         this.decriptionLabel.visible = !choosing;
         this.checkBoxRegularCost.visible = !choosing;
         this.checkBoxPremiumCost.visible = !choosing;
         this.modifiersList.enabled = !choosing;
         this.decriptionTimerLabel.visible = !choosing;
         this.background.height = !choosing ? 392 : 337;
         this.lockBackground.height = this.background.height;
         var buttonAreaHeight:int = 48;
         var backgroundBottom:int = this.background.height - 3;
         btnAccept.y = backgroundBottom + 4 - buttonAreaHeight;
         btnCancel.y = btnAccept.y;
         btnAccept.label = LocalizationData.getName(!choosing ? "btn_reroll_change_label" : "btn_reroll_choose_label");
         btnCancel.label = (!choosing ? LocalizationData.getName("cancel") : String(KEEP_OLD_LABEL[RoomsText.language()])).toUpperCase();
         btnClose.enabled = !choosing;
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
      
      protected function testOptions() : void
      {
         this.updateReroll(this.modifiersList.dataProvider as Array);
      }
      
      protected function testOptions2() : void
      {
         this.updateModifiers(this.modifiersList.dataProvider as Array);
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         if(!btnAccept.enabled)
         {
            return;
         }
         var selectedModifier:uint = this.modifiersList.selectedIndex;
         var moneyType:uint;
         var modifierIndex:uint;
         if(this.rerollContainer.visible)
         {
            moneyType = this.getSelectedIndex();
            modifierIndex = selectedModifier;
            trace("reroll_item_modifier",uint(moneyType));
            if(moneyType == MoneyType.PREMIUM && this._rerollCostGold > PlayerInfo.premiumMoney)
            {
               this.onNoMoneyEnought();
            }
            else
            {
               trace("reroll_item_modifier",uint(_data.itemId),uint(modifierIndex));
               ExternalInterface.call("reroll_item_modifier",_data.itemId,modifierIndex,uint(moneyType));
               closable = false;
               this.lockWindow(true);
            }
         }
         else
         {
            ExternalInterface.call("modifier_choosed",uint(_data.itemId),uint(selectedModifier),uint(this.modifiersList2.selectedIndex));
            this.lockWindow(true);
         }
      }

      // While the three are offered the cancel button keeps the old modification: the server's
      // answer (83 46 08, the old one into itself) fills the window again, as after a choice.
      override protected function onMessageCancel(event:ButtonEvent = null) : void
      {
         if(this.rerollContainer.visible)
         {
            super.onMessageCancel(event);
            return;
         }
         if(event == null || event.currentTarget != btnCancel || this.modifiersList.selectedIndex < 0)
         {
            return;
         }
         ExternalInterface.call("modifier_choosed",uint(_data.itemId),uint(this.modifiersList.selectedIndex),RerollChoice.KEEP_OLD);
         this.lockWindow(true);
      }
   }
}

