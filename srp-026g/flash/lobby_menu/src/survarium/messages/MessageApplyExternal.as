package survarium.messages
{
   import flash.external.ExternalInterface;
   import scaleform.clik.events.ButtonEvent;
   
   public class MessageApplyExternal extends MessageApply
   {
      
      public function MessageApplyExternal()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         _data = {text:_data};
         super.configUI();
      }
      
      override protected function onMessageCancel(event:ButtonEvent = null) : void
      {
         super.onMessageCancel();
         ExternalInterface.call("system_message_clicked",_id,false);
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         super.onMessageAccept();
         ExternalInterface.call("system_message_clicked",_id,true);
      }
   }
}

