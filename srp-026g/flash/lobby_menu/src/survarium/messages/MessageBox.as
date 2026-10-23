package survarium.messages
{
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextFieldAutoSize;
   import flash.ui.Keyboard;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   import survarium.controls.Message;
   
   public class MessageBox extends MessageNoClose
   {
      
      public var _enter:Button = new SrvButtonSkinned();
      
      public function MessageBox()
      {
         super();
         addChild(this._enter);
         this._enter.label = "OK";
         this._enter.addEventListener(ButtonEvent.CLICK,this.close);
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         tabChildren = false;
         focusable = true;
      }
      
      override protected function cleanUp(e:Event = null) : void
      {
         super.cleanUp();
         this._enter.removeEventListener(MouseEvent.MOUSE_DOWN,this.close);
         removeEventListener(InputEvent.INPUT,this.handleInput);
      }
      
      protected function close(e:Event = null) : void
      {
         Message.close(_id);
         ExternalInterface.call("system_message_ok_clicked",_id);
      }

      // Esc is the box's OK, which tells the game
      override public function cancelByEsc() : void
      {
         this.close();
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.isDefaultPrevented())
         {
            return;
         }
         var details:InputDetails = event.details;
         switch(details.code)
         {
            case Keyboard.ENTER:
               if(details.value == InputValue.KEY_UP)
               {
                  this.close();
                  event.handled = true;
               }
               break;
            // Esc: LobbyMenu.close_active_window (Modals), the one Esc handler (cancelByEsc)
         }
      }
      
      override protected function resize() : void
      {
         if(_message.height > 400)
         {
            _message.autoSize = TextFieldAutoSize.NONE;
            _message.height = 400;
         }
         else
         {
            _message.autoSize = TextFieldAutoSize.LEFT;
         }
         var offset:uint = 23;
         _body.height = Math.max(_message.height + textOffset * 2 + this._enter.height + offset,80);
         this._enter.x = (_body.width - this._enter.width) / 2;
         this._enter.y = _body.height - this._enter.height - 10;
      }
   }
}

