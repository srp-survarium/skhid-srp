package survarium.messages
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.DropdownMenu;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ListEvent;
   import survarium.Util;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;
   import survarium.constants.MessageId;
   
   public class MessagePremiumProfile extends UIComponent
   {
      
      public var dropDownlist:DropdownMenu;
      
      public var moneyBackContainer:Sprite;
      
      public var textField:TextField;
      
      public var textFieldValue:TextField;
      
      public var coin:MovieClip;
      
      public var btnAccept:Button;
      
      public var data:Object;
      
      public function MessagePremiumProfile()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.coin.gotoAndStop(2);
         this.textFieldValue.mouseEnabled = false;
         tabEnabled = true;
         this.btnAccept.enabled = false;
         this.textFieldValue.text = "0";
         LocalizationData.addTextField(this.textField,"premium_profile_unlock_description");
         LocalizationData.addButton(this.btnAccept,"buy");
         this.btnAccept.addEventListener(ButtonEvent.CLICK,this.onMessageAccept);
      }
      
      public function setData(newData:Object) : void
      {
         var selectedOption:int;
         var options:Array;
         var i:uint;
         var option:Object;
         var affordable:Boolean;
         var discountText:String;
         data = newData;
         if(this.data)
         {
            selectedOption = -1;
            options = this.data.options;
            i = 0;
            while(i < options.length)
            {
               option = options[i];
               affordable = PlayerInfo.premiumMoney >= option.value;
               option.valid = affordable;
               option.enabled = true;
               trace("configPM::",option.label,"|",option.value2,option.discount_value);
               Util.logObject(option);
               if(option.text == undefined)
               {
                  option.text = option.label;
               }
               if(option.discount)
               {
                  discountText = Util.styledChar(LocalizationData.getName("discount"),"white") + " " + Util.styledChar(option.discount,"yellow");
               }
               else
               {
                  discountText = "";
               }
               option.label = Util.styledChar(option.text,"yellow") + " " + Util.styledChar(option.value2,"dark_yellow") + " " + discountText;
               if(affordable && selectedOption == -1)
               {
                  selectedOption = i;
               }
               i++;
            }
            selectedOption = int(options.length / 2);
            this.btnAccept.enabled = true;
         }
         if(this.dropDownlist)
         {
            this.dropDownlist.menuWidth = 200;
            this.dropDownlist.dataProvider = new DataProvider(options);
            this.dropDownlist.addEventListener(ListEvent.INDEX_CHANGE,this.onPremiumSelected);
            this.dropDownlist.autoSize = "left";
            this.dropDownlist.selectedIndex = selectedOption;
            this.dropDownlist.validateNow();
            this.dropDownlist.autoSize = "none";
            this.moneyBackContainer.x = this.dropDownlist.x + this.dropDownlist.width + 3;
            this.coin.x = this.moneyBackContainer.x + 36;
            this.textFieldValue.x = this.moneyBackContainer.x + 58;
            this.onPremiumSelected();
         }
      }
      
      private function onPremiumSelected(event:ListEvent = null) : void
      {
         if(this.dropDownlist.selectedIndex == -1)
         {
            return;
         }
         var option:Object = this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex];
         this.textFieldValue.text = Util.getMoneyString(option.value);
      }
      
      protected function onNoMoneyEnought() : void
      {
         var messageData:Object = {
            text:LocalizationData.getName("cant_buy_premium_message"),
            onYes:this.goToStore,
            label_ok:LocalizationData.getName("btn_add_premium_message"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",messageData,true,MessageId.AUTO,true);
      }
      
      protected function goToStore() : void
      {
         MessageMoneyExchange.buyGold();
      }
      
      protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         if(!this.btnAccept.enabled)
         {
            return;
         }
         var affordable:Boolean = PlayerInfo.premiumMoney >= this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].value;
         var premiumId:uint;
         if(affordable)
         {
            this.btnAccept.enabled = false;
            premiumId = this.dropDownlist.dataProvider[this.dropDownlist.selectedIndex].id;
            trace("accept_premium:",premiumId);
            ExternalInterface.call("accept_premium",premiumId);
         }
         else
         {
            this.onNoMoneyEnought();
         }
      }
   }
}

