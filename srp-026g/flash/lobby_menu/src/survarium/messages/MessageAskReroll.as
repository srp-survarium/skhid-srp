package survarium.messages
{
   import flash.display.MovieClip;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.events.ButtonEvent;
   import survarium.buttons.CheckBoxValue;
   import survarium.data.LocalizationData;
   import survarium.constants.MoneyType;
   
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
            trace("MessageAskReroll",_data.price,"|",_data.points);
            // Paid in silver only: the game has no gold (the owner, 2026-11-08).
            this.checkBoxRegularCost.money_type = MoneyType.REGULAR;
            this.checkBoxRegularCost.selected = true;
            this.checkBoxRegularCost.value = int(_data.price * _data.points);
            this.checkBoxRegularCost.toggle = false;
            this.checkBoxPremiumCost.visible = this.checkBoxPremiumCost.enabled = false;
         }
      }

      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         super.onMessageAccept();
         trace("reroll_ok_clicked",uint(_data.branch));
         ExternalInterface.call("reroll_ok_clicked",uint(_data.branch),uint(MoneyType.REGULAR));
      }
   }
}

