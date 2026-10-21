package survarium.messages
{
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.data.LocalizationData;
   
   public class MessageInfoAccept extends MessageInfoBase
   {
      
      public var btnAccept:Button;
      
      public var textFieldLabel:TextField;
      
      public var textField:TextField;
      
      public function MessageInfoAccept()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         this.btnAccept.label = LocalizationData.getName("no_faction_button_label");
         this.textFieldLabel.text = LocalizationData.getName("no_faction_message_label");
         super.configUI();
         this.btnAccept.addEventListener(ButtonEvent.CLICK,onMessageAccept,false,0,true);
         this.textField.text = LocalizationData.getName("no_faction_message_text");
      }
   }
}

