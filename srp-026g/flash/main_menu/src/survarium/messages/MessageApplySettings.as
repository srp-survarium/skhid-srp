package survarium.messages
{
   import flash.external.ExternalInterface;
   import scaleform.clik.events.ButtonEvent;
   
   public class MessageApplySettings extends MessageApply
   {
      
      public function MessageApplySettings()
      {
         super();
      }
      
      override protected function onMessageCancel(event:ButtonEvent = null) : void
      {
         super.onMessageCancel();
         if(_data)
         {
            ExternalInterface.call("cancel_changes",_data.selectedIndex);
         }
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         if(_data)
         {
            ExternalInterface.call("accept_changes",_data.selectedIndex);
         }
         super.onMessageAccept();
      }
   }
}

