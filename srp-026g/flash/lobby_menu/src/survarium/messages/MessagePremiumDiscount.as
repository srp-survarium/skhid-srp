package survarium.messages
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.DropdownMenu;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ListEvent;
   import survarium.Util;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;
   
   public class MessagePremiumDiscount extends MessageAccept
   {
      
      public var dropDownlist:DropdownMenu;
      
      public var background:Sprite;
      
      public var moneyBackContainer:Sprite;
      
      public var textFieldValueOption:TextField;
      
      public var textFieldDiscountValue:TextField;
      
      public var textFieldPremium:TextField;
      
      public var textFieldAccount:TextField;
      
      public var textFieldChooseSub:TextField;
      
      public var premiumBonus1:MovieClip;
      
      public var premiumBonus2:MovieClip;
      
      public var premiumBonus3:MovieClip;
      
      public var premiumBonus4:MovieClip;
      
      public var premiumBonus5:MovieClip;
      
      public var textFieldValue:TextField;
      
      public var textFieldValueOld:TextField;
      
      public var coin:MovieClip;
      
      public var coinOld:MovieClip;
      
      public var btnAddPremumMoney:Button;
      
      public var line:MovieClip;
      
      public var discountContainer:MovieClip;
      
      public var premiumOption:Object;
      
      protected const LIST_OFFSET_TOP:uint = 97;
      
      protected const LIST_OFFSET_BOTTOM:uint = 82;
      
      public function MessagePremiumDiscount()
      {
         super();
      }
      
      protected function updateTextFormat(textField:TextField, text:String, tabStops:int = 55) : void
      {
         var string:String = text;
         var spaceIndex:int = string.indexOf(" ");
         var beforeSpace:String = string.substr(0,spaceIndex);
         var afterSpace:String = string.substr(spaceIndex + 1,string.length);
         var finalString:String = "\t" + beforeSpace + "\n" + afterSpace;
         var textFormat:TextFormat = textField.defaultTextFormat;
         textFormat.leftMargin = 0;
         textFormat.tabStops = [tabStops];
         textField.defaultTextFormat = textFormat;
         textField.setTextFormat(textFormat);
         textField.text = finalString;
      }
      
      override protected function configUI() : void
      {
         var startIndex:int;
         var array:Array;
         var i:uint;
         var premiumOption:Object;
         var enabled:Boolean;
         var dicountStr:String;
         super.configUI();
         this.coin.gotoAndStop(2);
         this.coinOld.gotoAndStop(2);
         btnAccept.label = LocalizationData.getName("buy");
         this.textFieldValue.mouseEnabled = false;
         if(this.premiumBonus1)
         {
            this.premiumBonus3.textField.width += 10;
            this.updateTextFormat(this.premiumBonus1.textField,LocalizationData.getName("premium_bonus_experience").toUpperCase());
            this.updateTextFormat(this.premiumBonus2.textField,LocalizationData.getName("premium_bonus_money").toUpperCase());
            this.updateTextFormat(this.premiumBonus3.textField,LocalizationData.getName("premium_bonus_reputation").toUpperCase());
            this.updateTextFormat(this.premiumBonus4.textField,LocalizationData.getName("premium_bonus_item_search").toUpperCase());
            this.updateTextFormat(this.premiumBonus5.textField,LocalizationData.getName("premium_bonus_profile").toUpperCase(),25);
         }
         tabEnabled = true;
         btnAccept.enabled = false;
         this.textFieldValue.text = "0";
         this.textFieldValueOld.text = "0";
         if(_data)
         {
            startIndex = -1;
            array = _data.options;
            for(i = 0; i < array.length; i++)
            {
               premiumOption = array[i];
               enabled = PlayerInfo.premiumMoney >= premiumOption.value;
               premiumOption.valid = enabled;
               premiumOption.enabled = true;
               trace("configPM::",premiumOption.label,"|",premiumOption.value2,premiumOption.discount_value);
               Util.logObject(premiumOption);
               if(premiumOption.text == undefined)
               {
                  premiumOption.text = premiumOption.label;
               }
               if(premiumOption.discount)
               {
                  dicountStr = Util.styledChar(LocalizationData.getName("discount"),"white") + " " + Util.styledChar(premiumOption.discount,"yellow");
               }
               else
               {
                  dicountStr = "";
               }
               premiumOption.label = Util.styledChar(premiumOption.text,"yellow") + " " + Util.styledChar(premiumOption.value2,"dark_yellow") + " " + dicountStr;
               if(enabled && startIndex == -1)
               {
                  startIndex = i;
               }
            }
            startIndex = int(array.length / 2);
            btnAccept.enabled = true;
         }
         if(this.dropDownlist)
         {
            this.dropDownlist.menuWidth = 200;
            this.textFieldChooseSub.text = LocalizationData.getName("buy_premium_with_discount");
            this.dropDownlist.dataProvider = new DataProvider(array);
            this.dropDownlist.addEventListener(ListEvent.INDEX_CHANGE,this.onPremiumSelected);
            this.dropDownlist.autoSize = "left";
            this.dropDownlist.selectedIndex = startIndex;
            this.dropDownlist.validateNow();
            this.dropDownlist.autoSize = "none";
            this.moneyBackContainer.x = this.dropDownlist.x + this.dropDownlist.width + 3;
            this.coin.x = this.moneyBackContainer.x + 36;
            this.textFieldValue.x = this.moneyBackContainer.x + 55;
            this.textFieldValueOld.text = Util.getMoneyString(premiumOption.value_old);
            this.line.x = this.coin.x - 10;
            this.line.width = this.textFieldValueOld.textWidth + 40;
            this.textFieldDiscountValue.text = "-" + String(int(premiumOption.discount_value * 100));
            this.coinOld.x = this.coin.x;
            this.textFieldValueOld.x = this.textFieldValue.x;
            this.onPremiumSelected();
         }
      }
      
      private function onPremiumSelected(e:ListEvent = null) : void
      {
         if(this.dropDownlist.selectedIndex == -1)
         {
            return;
         }
         var data:Object = this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex];
         this.textFieldValue.text = Util.getMoneyString(data.value);
         this.textFieldValueOld.text = Util.getMoneyString(data.value_old);
         this.line.width = this.textFieldValueOld.textWidth + 40;
      }
      
      protected function onNoMoneyEnought() : void
      {
         var object:Object = {
            text:LocalizationData.getName("cant_buy_premium_message"),
            onYes:this.goToStore,
            label_ok:LocalizationData.getName("btn_add_premium_message"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",object,true,0,true);
      }
      
      protected function goToStore() : void
      {
         MessageMoneyExchange.buyGold();
      }
      
      override protected function onMessageCancel(event:ButtonEvent = null) : void
      {
         super.onMessageCancel(event);
         trace("accept_premium_with_discount:",-1,-1);
         ExternalInterface.call("accept_premium_with_discount",-1,-1);
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         if(!btnAccept.enabled)
         {
            return;
         }
         var selectedEnabled:Boolean = PlayerInfo.premiumMoney >= this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].value;
         var selectedId:uint;
         var selectedDiscount:Number;
         var fn:Function;
         if(selectedEnabled)
         {
            super.onMessageAccept();
            selectedId = (this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].id);
            selectedDiscount = (this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].discount_value);
            trace("accept_premium_with_discount:",selectedId,selectedDiscount);
            ExternalInterface.call("accept_premium_with_discount",selectedId,selectedDiscount);
            fn = _data.onYes as Function;
            _data.onNo = null;
            _data.onYes = null;
            if(fn is Function)
            {
               fn(_data);
            }
         }
         else
         {
            this.onNoMoneyEnought();
         }
      }
   }
}

