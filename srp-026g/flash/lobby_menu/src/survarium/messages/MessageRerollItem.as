package survarium.messages
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
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
   import survarium.controls.SilverPrice;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.LocalizationData;
   import survarium.item.ModificationRolls;
   import survarium.constants.MoneyType;
   import survarium.constants.RerollChoice;

   public class MessageRerollItem extends MessageAccept
   {

      // The item's picture hovered and left: the lobby shows cardItem's card, the inventory's
      // (LobbyMenu.showRerollItemHint), and takes it away.
      public static const SHOW_CARD:String = "rerollShowCard";

      public static const HIDE_CARD:String = "rerollHideCard";

      // The original movie places the game's lock hint (one modification an item) on this window;
      // since any modification may be changed it is not used (configUI hides it).
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

      // The price, in place of the silver checkbox (SilverPrice): made with the window's data.
      private var _price:SilverPrice;
      
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
      
      protected var _itemId:uint = 0;
      
      protected var _modifierSelection:*;
      
      public function MessageRerollItem()
      {
         trace("MessageRerollItem",this.modifiersList,this.modifiersList2,this.checkBoxRegularCost,this.checkBoxPremiumCost,this.wheels,this.rerollContainer,this.buttonReroll,this.buttonCraft);
         super();
      }
      
      override protected function cleanUp(event:Event = null) : void
      {
         // closed under the pointer (Enter, Esc): its card goes with it
         dispatchEvent(new Event(HIDE_CARD,true));
         this.itemRenderer.data = null;
         super.cleanUp();
      }

      // The item the window shows, null before it is filled.
      public function get cardItem() : Object
      {
         return this.itemRenderer.data;
      }

      private function onItemOver(e:MouseEvent) : void
      {
         if(this.itemRenderer.data != null)
         {
            dispatchEvent(new Event(SHOW_CARD,true));
         }
      }

      private function onItemOut(e:MouseEvent) : void
      {
         dispatchEvent(new Event(HIDE_CARD,true));
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
         // Hovering the item's picture shows its card, the inventory's, its modifications applied
         // (the owner, 2026-10-23; at the inventory's size, 2026-11-06). The picture's own back is
         // a fill of alpha 0, which takes no hover: a 0.01 one over it.
         this.itemRenderer.addEventListener(MouseEvent.ROLL_OVER,this.onItemOver);
         this.itemRenderer.addEventListener(MouseEvent.ROLL_OUT,this.onItemOut);
         this.itemRenderer.graphics.beginFill(0,0.01);
         this.itemRenderer.graphics.drawRect(this.itemRenderer.iconRenderer.x,this.itemRenderer.iconRenderer.y,this.itemRenderer.iconRenderer.width,this.itemRenderer.iconRenderer.height);
         this.itemRenderer.graphics.endFill();
         addChildAt(this.itemRenderer,numChildren - 3);
         super.configUI();
         this.modifiersList.setMaxFit(true);
         btnAccept.label = LocalizationData.getName("reroll");
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
            // The window's rule line, in place of the game's st_modifier_reroll_description ("only one
            // modifier"): any modifier may be changed (the owner's rule of 2026-10-22).
            this.decriptionLabel.text = LocalizationData.getName("st_srp_reroll_description");
            // Skhid: no "only one attempt in 6 hours" line: silver rerolls have no wait
            // (items_rules: Reroll, the owner's rule of 2026-10-22)
            this.decriptionTimerLabel.text = "";
            this.labelChooseModifier2.text = LocalizationData.getName("choose_modifier");
            // A change is paid in silver only: the game has no gold (the owner, 2026-11-08). Its
            // price, centred, in place of the silver checkbox (SilverPrice).
            this.fitDescription();
            this._price = new SilverPrice(this.checkBoxRegularCost,this.background.x + this.background.width / 2);
            this.checkBoxPremiumCost.visible = false;
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
                  if(!modifier.isDefault)
                  {
                     modifierIcon = iconContainer.getIcon(modifier.icon);
                     option = {
                        id:modifier.id,
                        icon:modifierIcon,
                        label:modifier.textValue
                     };
                     options.push(ModificationRolls.withRange(option,modifier));
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
            options.push(ModificationRolls.withRange(option,modifier));
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
         if(this._price)
         {
            this._price.cost = this._rerollCost;
         }
         if(modifiers)
         {
            // The card's item: its modifications as the server has them now (after a choice
            // too), with the item kind's own, as the inventory composes them.
            _data.modifiers = modifiers.concat(_data.modifiersDefault ? _data.modifiersDefault : []);
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
               // Skhid: every modifier may be changed, each any number of times (the owner's rule
               // of 2026-10-22); the one changed last is selected.
               option = {
                  id:modifier.id,
                  icon:modifierIcon,
                  label:modifier.textValue
               };
               options.push(ModificationRolls.withRange(option,modifier));
               i++;
            }
            trace("modifiersToReroll",modifierCount,options.length,rerolledIndex,noneRerolled);
            this.modifiersList.dataProvider = new DataProvider(options);
            if(!noneRerolled && rerolledIndex != -1)
            {
               this.modifiersList.selectedIndex = rerolledIndex;
            }
         }
      }
      
      protected function checkAcceptButton(event:Event = null) : void
      {
         var selectedModifier:int;
         if(this.rerollContainer.visible)
         {
            selectedModifier = this.modifiersList.selectedIndex;
            if(selectedModifier == -1)
            {
               btnAccept.enabled = false;
               return;
            }
            btnAccept.enabled = this._rerollCost <= PlayerInfo.genericMoney && !this._rerollTimer;
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
      
      protected function showModifierSelection(selectedModifier:uint, options:Array) : void
      {
         this.setChooseReollOptionState(true);
         this.lockWindow(false);
         this.modifiersList2.dataProvider = new DataProvider(options);
         btnAccept.enabled = false;
      }
      
      // The description's panel fits its text, DESCRIPTION_MARGIN above and below (the owner, 2026-11-08:
      // "too much padding for that text", "from above and below"); the price under it moves up by what the
      // panel gave back. The panel is rerollContainer's `background`; without one, nothing moves.
      private static const DESCRIPTION_MARGIN:Number = 12;

      private function fitDescription() : void
      {
         var panel:DisplayObject = this.rerollContainer.getChildByName("background");
         if(panel == null)
         {
            return;
         }
         var top:Number = this.rerollContainer.y + panel.y;
         var oldBottom:Number = top + panel.height;
         var textHeight:Number = this.decriptionLabel.textHeight + 4;
         this.decriptionLabel.height = textHeight;
         this.decriptionLabel.y = top + DESCRIPTION_MARGIN;
         panel.height = textHeight + 2 * DESCRIPTION_MARGIN;
         var up:Number = oldBottom - (top + panel.height);
         this.decriptionTimerLabel.y -= up;
         this.checkBoxRegularCost.y -= up;
      }

      private function setChooseReollOptionState(choosing:Boolean) : void
      {
         this.rerollContainer.visible = !choosing;
         this.labelChooseModifier2.visible = choosing;
         this.modifiersList2.selectedIndex = -1;
         this.modifiersList2.visible = choosing;
         this.decriptionLabel.visible = !choosing;
         this._price.visible = !choosing;
         this.modifiersList.enabled = !choosing;
         this.decriptionTimerLabel.visible = !choosing;
         // While the price shows, the window ends SilverPrice.GAP under it, as the upgrade window does (the
         // owner, 2026-11-08: "there is too much empty space after 200 silver"): the stock 392 left room for
         // the taller money choice.
         this.background.height = !choosing && this._price ? this._price.windowHeight : (!choosing ? 392 : 337);
         this.lockBackground.height = this.background.height;
         var buttonAreaHeight:int = 48;
         var backgroundBottom:int = this.background.height - 3;
         btnAccept.y = backgroundBottom + 4 - buttonAreaHeight;
         btnCancel.y = btnAccept.y;
         btnAccept.label = LocalizationData.getName(!choosing ? "btn_reroll_change_label" : "btn_reroll_choose_label");
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
         var modifierIndex:uint;
         if(this.rerollContainer.visible)
         {
            modifierIndex = selectedModifier;
            trace("reroll_item_modifier",uint(_data.itemId),uint(modifierIndex));
            ExternalInterface.call("reroll_item_modifier",_data.itemId,modifierIndex,uint(MoneyType.REGULAR));
            closable = false;
            this.lockWindow(true);
         }
         else
         {
            ExternalInterface.call("modifier_choosed",uint(_data.itemId),uint(selectedModifier),uint(this.modifiersList2.selectedIndex));
            this.lockWindow(true);
         }
      }

      // While the three are offered the game's Cancel button (the stock label MessageAccept sets)
      // takes none of them: the item keeps its modification, the change stays paid. The server's
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

