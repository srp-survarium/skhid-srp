package survarium.messages
{
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.utils.Constraints;
   import survarium.data.LocalizationData;
   
   public class MessageAnnounce extends MessageInfoBase
   {
      
      public var textField:TextField;
      
      public var btnAccept:*;
      
      public function MessageAnnounce()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         constraints.addElement("textField",this.textField,Constraints.ALL);
         constraints.addElement("yesBtn",this.btnAccept,Constraints.RIGHT | Constraints.BOTTOM);
         this.textField.autoSize = TextFieldAutoSize.LEFT;
         this.textField.wordWrap = false;
         LocalizationData.addButton(this.btnAccept,"ok");
         tabEnabled = true;
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.DATA))
         {
            if(_data)
            {
               this.textField.text = _data.text;
            }
         }
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         super.onMessageAccept();
         ExternalInterface.call("announce_ok_clicked");
      }
   }
}

