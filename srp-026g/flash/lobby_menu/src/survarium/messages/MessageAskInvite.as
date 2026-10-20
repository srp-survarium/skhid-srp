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
      
      override public function setMessageData(param1:int, param2:Object) : void
      {
         trace("SrvMessageInvited::",param2,this.textFieldName,this.textField);
         super.setMessageData(param1,param2);
         if(!param2)
         {
            return;
         }
         messageId = param2.id;
         messageType = param2.type;
         this.textFieldName.multiline = true;
         var _loc3_:TextFormat = this.textFieldName.getTextFormat();
         _loc3_.leading = 0;
         this.textFieldName.defaultTextFormat = _loc3_;
         this.textFieldName.autoSize = TextFieldAutoSize.LEFT;
         this.textFieldName.text = param2.name;
         this.textField.text = param2.text;
         if(this.textField.numLines > 1 || this.textField.text == "")
         {
            this.textFieldName.y = this.textField.y + this.textField.textHeight + 5;
            btnAccept.y = btnCancel.y = this.textFieldName.y + this.textFieldName.textHeight + 12;
            this.background.height = btnAccept.y + btnAccept.height + 8;
         }
         btnAccept.enabled = param2.acceptable;
      }
      
      private function hideMe(param1:ButtonEvent) : void
      {
         super.onMessageCancel();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         LocalizationData.addButton(btnAccept,"confirm");
         LocalizationData.addButton(btnCancel,"cancel");
         this.btnHide.addEventListener(ButtonEvent.CLICK,this.hideMe);
      }
      
      override protected function onMessageCancel(param1:ButtonEvent = null) : void
      {
         trace("important_message_read",this.messageId,uint(1),this.messageType);
         ExternalInterface.call("important_message_read",this.messageId,uint(1),this.messageType);
         super.onMessageCancel();
      }
      
      override protected function onMessageAccept(param1:ButtonEvent = null) : void
      {
         trace("important_message_read",this.messageId,uint(0),this.messageType);
         ExternalInterface.call("important_message_read",this.messageId,uint(0),this.messageType);
         super.onMessageAccept();
      }
   }
}

