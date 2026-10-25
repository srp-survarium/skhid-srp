package survarium.messages
{
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.ui.Keyboard;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   import survarium.controls.Message;
   import survarium.controls.SrvTextInput;
   import survarium.constants.UiSound;
   
   public class MessageRename extends MessageBase
   {
      
      public var textFieldLabel:TextField;
      
      public var btnAccept:Button;
      
      public var btnCancel:Button;
      
      public var textInput:SrvTextInput;
      
      protected const MIN_CHARS:int = 3;
      
      protected const MAX_CHARS:int = 16;
      
      public function MessageRename()
      {
         super();
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         ExternalInterface.call("sound_play",UiSound.MENU_MESSAGE);
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.isDefaultPrevented())
         {
            return;
         }
         var details:InputDetails = event.details;
         var controllerIndex:uint = details.controllerIndex;
         switch(details.code)
         {
            case Keyboard.ENTER:
               if(details.value == InputValue.KEY_UP)
               {
                  this.onMessageAccept(null);
                  event.handled = true;
               }
               break;
            // Esc: LobbyMenu.close_active_window (Modals), the one Esc handler
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
      
      public function close() : void
      {
         this.onMessageCancel(null);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.textFieldLabel.text = _data.windowLabel;
         this.btnAccept.label = _data.acceptLabel;
         this.btnCancel.label = _data.cancelLabel;
         this.btnAccept.addEventListener(ButtonEvent.CLICK,this.onMessageAccept,false,0,true);
         this.btnCancel.addEventListener(ButtonEvent.CLICK,this.onMessageCancel,false,0,true);
         this.textInput.addEventListener(Event.CHANGE,this.updatePromo);
         tabEnabled = true;
         this.textInput.maxChars = this.MAX_CHARS;
         this.textInput.defaultText = _data.defaultText;
         this.textInput.text = _data.currentName;
         this.btnAccept.enabled = false;
         addEventListener(Event.RENDER,this.onFocusRender);
      }
      
      private function onFocusRender(event:Event) : void
      {
         removeEventListener(event.type,this.onFocusRender);
         stage.focus = this.textInput;
         this.textInput.textField.setSelection(0,this.textInput.textField.text.length);
      }
      
      private function requestKeyValidation(event:Event = null) : void
      {
         this.btnAccept.enabled = false;
         this.textInput.enabled = false;
         trace("requestRenameValidation",_data.externalInterface,_data.externalData,this.textInput.text);
         ExternalInterface.call(_data.externalInterface,_data.externalData,this.textInput.text);
         Message.onClick(_id,"yes");
      }
      
      private function updatePromo(event:Event) : void
      {
         var newName:String = this.textInput.text;
         this.btnAccept.enabled = this.checkLocalKeyValidation();
      }
      
      protected function checkLocalKeyValidation() : Boolean
      {
         var newName:String = this.textInput.text;
         trace("checkLocalNicknameValidation",newName);
         return newName.length >= this.MIN_CHARS && newName.length <= this.MAX_CHARS;
      }
      
      protected function onMessageCancel(event:ButtonEvent) : void
      {
         Message.onClick(_id,"no");
         removeEventListener(InputEvent.INPUT,this.handleInput);
      }

      // Esc is the Cancel
      override public function cancelByEsc() : void
      {
         this.onMessageCancel(null);
      }
      
      protected function onMessageAccept(event:ButtonEvent) : void
      {
         if(!this.btnAccept.enabled)
         {
            return;
         }
         this.requestKeyValidation();
      }
   }
}

