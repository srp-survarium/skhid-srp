package survarium.messages
{
   import flash.display.MovieClip;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.events.ButtonEvent;
   import survarium.buttons.CheckBoxValue;
   import survarium.data.LocalizationData;
   
   public class MessageAskReroll extends MessageAccept
   {
      
      public var textField:TextField;
      
      public var checkBoxRegularCost:CheckBoxValue;
      
      public var checkBoxPremiumCost:CheckBoxValue;
      
      public var background:MovieClip;
      
      public function MessageAskReroll()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.textField.text = LocalizationData.getName("message_reroll_text");
         if(_data)
         {
            trace("MessageAskReroll",_data.price,_data.price_premium,"|",_data.points);
            this.checkBoxRegularCost.money_type = 0;
            this.checkBoxPremiumCost.money_type = 1;
            this.checkBoxRegularCost.selected = true;
            this.checkBoxRegularCost.value = int(_data.price * _data.points);
            this.checkBoxPremiumCost.value = int(_data.price_premium * _data.points);
            this.checkBoxPremiumCost.enabled = this.checkBoxPremiumCost.value > 0;
            this.checkBoxRegularCost.toggle = false;
            this.checkBoxPremiumCost.toggle = false;
            this.checkBoxRegularCost.addEventListener(ButtonEvent.CLICK,this.onCheckBoxChanged);
            this.checkBoxPremiumCost.addEventListener(ButtonEvent.CLICK,this.onCheckBoxChanged);
         }
      }
      
      private function onCheckBoxChanged(event:ButtonEvent) : void
      {
         if(event.target == this.checkBoxRegularCost)
         {
            this.checkBoxRegularCost.selected = true;
            this.checkBoxPremiumCost.selected = false;
         }
         else
         {
            this.checkBoxRegularCost.selected = false;
            this.checkBoxPremiumCost.selected = true;
         }
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         super.onMessageAccept();
         var moneyType:uint = this.checkBoxRegularCost.selected ? 0 : 1;
         trace("reroll_ok_clicked",uint(_data.branch));
         ExternalInterface.call("reroll_ok_clicked",uint(_data.branch),moneyType);
      }
   }
}

