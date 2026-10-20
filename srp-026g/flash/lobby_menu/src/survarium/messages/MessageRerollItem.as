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
   
   public class MessageRerollItem extends MessageAccept
   {
      
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
      
      override protected function cleanUp(param1:Event = null) : void
      {
         this.itemRenderer.data = null;
         super.cleanUp();
      }
      
      override protected function configUI() : void
      {
         var _loc1_:Class = getDefinitionByName("SrvItemRendererFull") as Class;
         var _loc2_:Array;
         var _loc3_:uint;
         var _loc4_:Array;
         var _loc5_:IconContainer;
         var _loc6_:uint;
         var _loc7_:Object;
         var _loc8_:Icon;
         var _loc9_:Object;
         itemRenderer = new _loc1_();
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
            this.decriptionTimerLabel.text = LocalizationData.getName("modifier_reroll_timer_description");
            this.labelChooseModifier2.text = LocalizationData.getName("choose_modifier");
            this.checkBoxRegularCost.selected = true;
            this.checkBoxPremiumCost.selected = false;
            this.checkBoxRegularCost.money_type = 0;
            this.checkBoxPremiumCost.money_type = 1;
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
            _loc2_ = _data.modifiers as Array;
            if(_loc2_)
            {
               _loc3_ = _loc2_.length;
               _loc4_ = [];
               _loc5_ = IconContainer.getInstance("modifiers");
               _loc6_ = 0;
               while(_loc6_ < _loc3_)
               {
                  _loc7_ = _loc2_[_loc6_];
                  if(!_loc7_.isDefault && _loc7_.state != 1)
                  {
                     _loc8_ = _loc5_.getIcon(_loc7_.icon);
                     _loc9_ = {
                        id:_loc7_.id,
                        icon:_loc8_,
                        label:_loc7_.textValue
                     };
                     _loc4_.push(_loc9_);
                  }
                  _loc6_++;
               }
               trace("modifiersToReroll",_loc3_,_loc4_.length);
               this.modifiersList.dataProvider = new DataProvider(_loc4_);
            }
            this.modifiersList.addEventListener(IndexEvent.INDEX_CHANGE,this.checkAcceptButton);
            this.modifiersList2.addEventListener(IndexEvent.INDEX_CHANGE,this.checkAcceptButton);
            this.setChooseReollOptionState(false);
         }
         this.checkAcceptButton();
      }
      
      public function updateTimer(param1:Object) : void
      {
         if(_data && param1 && _data.itemId == param1.itemId)
         {
            _rerollTimer = param1.rerollTime;
            this.checkAcceptButton();
         }
      }
      
      public function updateReroll(param1:Array) : void
      {
         this.lockWindow(false);
         var _loc2_:uint = (this.modifiersList.selectedIndex);
         var _loc3_:Array = param1;
         var _loc4_:uint = _loc3_.length;
         var _loc5_:IconContainer = IconContainer.getInstance("modifiers");
         var _loc6_:Array = [];
         var _loc7_:uint = 0;
         var _loc8_:Object;
         var _loc9_:Icon;
         var _loc10_:Object;
         while(_loc7_ < _loc4_)
         {
            _loc8_ = _loc3_[_loc7_];
            _loc9_ = _loc5_.getIcon(_loc8_.icon);
            _loc10_ = {
               id:_loc8_.id,
               icon:_loc9_,
               label:_loc8_.textValue
            };
            _loc6_.push(_loc10_);
            _loc7_++;
         }
         this.showModifierSelection(_loc2_,_loc6_);
      }
      
      public function updateModifiers(param1:Object) : void
      {
         trace("modifiersToRerollUpgrade",param1);
         closable = true;
         Util.logObject(param1);
         if(!this.rerollContainer.visible)
         {
            this.setChooseReollOptionState(false);
         }
         var _loc2_:Array = param1.modifiers as Array;
         var _loc3_:uint;
         var _loc4_:Array;
         var _loc5_:IconContainer;
         var _loc6_:Boolean;
         var _loc7_:int;
         var _loc8_:Object;
         var _loc9_:uint;
         var _loc10_:Icon;
         var _loc11_:Object;
         this.lockWindow(false);
         _rerollCost = param1.regular_cost;
         _rerollCostGold = param1.premium_cost;
         this.checkBoxRegularCost.value = this._rerollCost;
         this.checkBoxPremiumCost.value = this._rerollCostGold;
         if(_loc2_)
         {
            _loc3_ = _loc2_.length;
            _loc4_ = [];
            _loc5_ = IconContainer.getInstance("modifiers");
            _loc6_ = _loc3_ > 0;
            _loc7_ = -1;
            _loc9_ = 0;
            while(_loc9_ < _loc3_)
            {
               _loc8_ = _loc2_[_loc9_];
               if(_loc8_.was_rerolled_before)
               {
                  _loc6_ = false;
                  _loc7_ = (_loc9_);
               }
               _loc9_++;
            }
            _loc9_ = 0;
            while(_loc9_ < _loc3_)
            {
               _loc8_ = _loc2_[_loc9_];
               _loc10_ = _loc5_.getIcon(_loc8_.icon);
               _loc11_ = {
                  id:_loc8_.id,
                  icon:_loc10_,
                  label:_loc8_.textValue,
                  reroll_available:!_loc8_.was_rerolled_before,
                  locked:_loc7_ == _loc9_ && _loc6_ == false
               };
               _loc4_.push(_loc11_);
               _loc9_++;
            }
            trace("modifiersToReroll",_loc3_,_loc4_.length,_loc7_,_loc6_);
            this.modifiersList.dataProvider = new DataProvider(_loc4_);
            if(!_loc6_ && _loc7_ != -1)
            {
               this.modifiersList.selectedIndex = _loc7_;
               this.modifiersList.enabled = false;
               this.modifiersList.mouseEnabled = false;
               this.modifiersList.mouseChildren = false;
               this.lockedHint.visible = true;
            }
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
         var _loc2_:int;
         var _loc3_:uint;
         if(this.rerollContainer.visible)
         {
            _loc3_ = this.getSelectedIndex();
            _loc2_ = this.modifiersList.selectedIndex;
            if(_loc2_ == -1)
            {
               btnAccept.enabled = false;
               return;
            }
            if(_loc3_ == 0)
            {
               btnAccept.enabled = this._rerollCost <= PlayerInfo.genericMoney && !this._rerollTimer;
            }
            else if(_loc3_ == 1)
            {
               btnAccept.enabled = true;
            }
         }
         else
         {
            _loc2_ = this.modifiersList2.selectedIndex;
            if(_loc2_ == -1)
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
         var _loc1_:uint;
         if(this.checkBoxRegularCost.selected)
         {
            _loc1_ = 0;
         }
         else
         {
            _loc1_ = 1;
         }
         return _loc1_;
      }
      
      protected function goToStore() : void
      {
         ExternalInterface.call("go_to_store");
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
      
      protected function showModifierSelection(param1:uint, param2:Array) : void
      {
         this.setChooseReollOptionState(true);
         this.lockWindow(false);
         this.modifiersList2.dataProvider = new DataProvider(param2);
         btnAccept.enabled = false;
      }
      
      private function setChooseReollOptionState(param1:Boolean) : void
      {
         this.rerollContainer.visible = !param1;
         this.labelChooseModifier2.visible = param1;
         this.modifiersList2.selectedIndex = -1;
         this.modifiersList2.visible = param1;
         this.decriptionLabel.visible = !param1;
         this.checkBoxRegularCost.visible = !param1;
         this.checkBoxPremiumCost.visible = !param1;
         this.modifiersList.enabled = !param1;
         this.decriptionTimerLabel.visible = !param1;
         this.background.height = !param1 ? 392 : 337;
         this.lockBackground.height = this.background.height;
         var _loc2_:int = 48;
         var _loc3_:int = this.background.height - 3;
         btnAccept.y = _loc3_ + 4 - _loc2_;
         btnCancel.y = btnAccept.y;
         btnAccept.label = LocalizationData.getName(!param1 ? "btn_reroll_change_label" : "btn_reroll_choose_label");
         btnCancel.enabled = !param1;
         btnClose.enabled = !param1;
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
      
      protected function testOptions() : void
      {
         this.updateReroll(this.modifiersList.dataProvider as Array);
      }
      
      protected function testOptions2() : void
      {
         this.updateModifiers(this.modifiersList.dataProvider as Array);
      }
      
      override protected function onMessageAccept(param1:ButtonEvent = null) : void
      {
         if(!btnAccept.enabled)
         {
            return;
         }
         var _loc2_:uint = this.modifiersList.selectedIndex;
         var _loc3_:uint;
         var _loc4_:uint;
         if(this.rerollContainer.visible)
         {
            _loc3_ = this.getSelectedIndex();
            _loc4_ = _loc2_;
            trace("reroll_item_modifier",uint(_loc3_));
            if(_loc3_ == 1 && this._rerollCostGold > PlayerInfo.premiumMoney)
            {
               this.onNoMoneyEnought();
            }
            else
            {
               trace("reroll_item_modifier",uint(_data.itemId),uint(_loc4_));
               ExternalInterface.call("reroll_item_modifier",_data.itemId,_loc4_,uint(_loc3_));
               closable = false;
               this.lockWindow(true);
            }
         }
         else
         {
            ExternalInterface.call("modifier_choosed",uint(_data.itemId),uint(_loc2_),uint(this.modifiersList2.selectedIndex));
            this.lockWindow(true);
         }
      }
   }
}

