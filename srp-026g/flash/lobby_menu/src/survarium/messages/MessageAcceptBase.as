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
      
      override public function handleInput(param1:InputEvent) : void
      {
         trace("MAB::handleInput",param1.handled);
         if(param1.isDefaultPrevented() || param1.handled)
         {
            return;
         }
         var _loc2_:InputDetails = param1.details;
         var _loc3_:uint = _loc2_.controllerIndex;
         switch(_loc2_.code)
         {
            case Keyboard.ENTER:
               if(_loc2_.value == InputValue.KEY_UP)
               {
                  onMessageAccept(null);
                  param1.handled = true;
               }
               break;
            case Keyboard.ESCAPE:
               if(_loc2_.value == InputValue.KEY_UP)
               {
                  if(stage.focus is TextField)
                  {
                     stage.focus = this;
                  }
               }
               break;
            case Keyboard.BACKSPACE:
               if(_loc2_.value == InputValue.KEY_UP)
               {
                  if(!stage.focus is TextField)
                  {
                     this.onMessageCancel(null);
                     param1.handled = true;
                  }
               }
               break;
         }
      }
      
      protected function onMessageCancel(param1:ButtonEvent = null) : void
      {
         trace("BASE:CANCEL");
         Message.onClick(_id);
      }
   }
}

