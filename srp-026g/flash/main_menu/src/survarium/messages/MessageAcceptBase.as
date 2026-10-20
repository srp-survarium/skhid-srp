package survarium.messages
{
   import flash.text.TextField;
   import flash.ui.Keyboard;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   import survarium.controls.Message;
   
   public class MessageAcceptBase extends MessageInfoBase
   {
      
      public function MessageAcceptBase()
      {
         super();
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         trace("MAB::handleInput",event.handled);
         if(event.isDefaultPrevented() || event.handled)
         {
            return;
         }
         var details:InputDetails = event.details;
         var index:uint = details.controllerIndex;
         switch(details.code)
         {
            case Keyboard.ENTER:
               if(details.value == InputValue.KEY_UP)
               {
                  onMessageAccept(null);
                  event.handled = true;
               }
               break;
            case Keyboard.ESCAPE:
               if(details.value == InputValue.KEY_UP)
               {
                  if(stage.focus is TextField)
                  {
                     stage.focus = this;
                  }
               }
               break;
            case Keyboard.BACKSPACE:
               if(details.value == InputValue.KEY_UP)
               {
                  if(!stage.focus is TextField)
                  {
                     this.onMessageCancel(null);
                     event.handled = true;
                  }
               }
               break;
         }
      }
      
      protected function onMessageCancel(event:ButtonEvent = null) : void
      {
         trace("BASE:CANCEL");
         Message.onClick(_id);
      }
   }
}

