package survarium.messages
{
   import flash.external.ExternalInterface;
   import scaleform.clik.events.ButtonEvent;
   
   public class MessageReassign extends MessageApply
   {
      
      public function MessageReassign()
      {
         super();
      }
      
      override protected function onMessageCancel(event:ButtonEvent = null) : void
      {
         super.onMessageCancel();
         ExternalInterface.call("reassign_cancel_clicked");
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         super.onMessageAccept();
         ExternalInterface.call("reassign_ok_clicked");
      }
   }
}

