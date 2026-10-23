package survarium.messages
{
   import flash.display.MovieClip;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.data.LocalizationData;
   import survarium.constants.ImportantMessageAnswer;
   
   public class MessageAskInvite extends MessageAccept
   {
      
      public var textFieldName:TextField;
      
      public var textField:TextField;
      
      public var btnHide:Button;
      
      public var background:MovieClip;
      
      protected var messageId:uint;
      
      protected var messageType:uint;
      
      public function MessageAskInvite() : void
      {
         super();
      }
      
      override public function setMessageData(id:int, messageData:Object) : void
      {
         trace("SrvMessageInvited::",messageData,this.textFieldName,this.textField);
         super.setMessageData(id,messageData);
         if(!messageData)
         {
            return;
         }
         messageId = messageData.id;
         messageType = messageData.type;
         this.textFieldName.multiline = true;
         var nameFormat:TextFormat = this.textFieldName.getTextFormat();
         nameFormat.leading = 0;
         this.textFieldName.defaultTextFormat = nameFormat;
         this.textFieldName.autoSize = TextFieldAutoSize.LEFT;
         this.textFieldName.text = messageData.name;
         this.textField.text = messageData.text;
         if(this.textField.numLines > 1 || this.textField.text == "")
         {
            this.textFieldName.y = this.textField.y + this.textField.textHeight + 5;
            btnAccept.y = btnCancel.y = this.textFieldName.y + this.textFieldName.textHeight + 12;
            this.background.height = btnAccept.y + btnAccept.height + 8;
         }
         btnAccept.enabled = messageData.acceptable;
      }
      
      private function hideMe(event:ButtonEvent) : void
      {
         super.onMessageCancel();
      }

      // Esc is the box's own hide (btnHide), not its Cancel: the box closes, nothing is sent
      // (no DECLINE) and the invite stays in the list.
      override public function cancelByEsc() : void
      {
         this.hideMe(null);
      }

      override protected function configUI() : void
      {
         super.configUI();
         LocalizationData.addButton(btnAccept,"confirm");
         LocalizationData.addButton(btnCancel,"cancel");
         this.btnHide.addEventListener(ButtonEvent.CLICK,this.hideMe);
      }
      
      override protected function onMessageCancel(event:ButtonEvent = null) : void
      {
         trace("important_message_read",this.messageId,uint(ImportantMessageAnswer.DECLINE),this.messageType);
         ExternalInterface.call("important_message_read",this.messageId,uint(ImportantMessageAnswer.DECLINE),this.messageType);
         super.onMessageCancel();
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         trace("important_message_read",this.messageId,uint(ImportantMessageAnswer.ACCEPT),this.messageType);
         ExternalInterface.call("important_message_read",this.messageId,uint(ImportantMessageAnswer.ACCEPT),this.messageType);
         super.onMessageAccept();
      }
   }
}

