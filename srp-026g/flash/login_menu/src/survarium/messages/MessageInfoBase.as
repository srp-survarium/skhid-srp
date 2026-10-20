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

   config namespace SOUND;
   SOUND const MENU_MESSAGE:int = 13;
   
   public class MessageInfoBase extends MessageBase
   {
      
      public function MessageInfoBase()
      {
         super();
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         ExternalInterface.call("sound_play",SOUND::MENU_MESSAGE);
      }
      
      override protected function cleanUp(e:Event = null) : void
      {
         super.cleanUp();
         removeEventListener(InputEvent.INPUT,this.handleInput);
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.isDefaultPrevented())
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
                  this.onMessageAccept();
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
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
      }
      
      protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         Message.onClick(_id);
         if(_data && _data.acceptCallback is Function)
         {
            _data.acceptCallback();
         }
      }
   }
}

