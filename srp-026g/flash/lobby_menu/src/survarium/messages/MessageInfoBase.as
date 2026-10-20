package survarium.messages
{
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.ui.Keyboard;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   import survarium.controls.Message;
   
   public class MessageInfoBase extends MessageBase
   {
      
      public function MessageInfoBase()
      {
         super();
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         ExternalInterface.call("sound_play",13);
      }
      
      override protected function cleanUp(param1:Event = null) : void
      {
         super.cleanUp();
         removeEventListener(InputEvent.INPUT,this.handleInput);
      }
      
      override public function handleInput(param1:InputEvent) : void
      {
         if(param1.isDefaultPrevented())
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
                  this.onMessageAccept();
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
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
      }
      
      protected function onMessageAccept(param1:ButtonEvent = null) : void
      {
         Message.onClick(_id);
         if(_data && _data.acceptCallback is Function)
         {
            _data.acceptCallback();
         }
      }
   }
}

