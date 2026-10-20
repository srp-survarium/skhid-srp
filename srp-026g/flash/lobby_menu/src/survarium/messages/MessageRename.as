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
         ExternalInterface.call("sound_play",13);
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
                  this.onMessageAccept(null);
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
      
      private function onFocusRender(param1:Event) : void
      {
         removeEventListener(param1.type,this.onFocusRender);
         stage.focus = this.textInput;
         this.textInput.textField.setSelection(0,this.textInput.textField.text.length);
      }
      
      private function requestKeyValidation(param1:Event = null) : void
      {
         this.btnAccept.enabled = false;
         this.textInput.enabled = false;
         trace("requestRenameValidation",_data.externalInterface,_data.externalData,this.textInput.text);
         ExternalInterface.call(_data.externalInterface,_data.externalData,this.textInput.text);
         Message.onClick(_id,"yes");
      }
      
      private function updatePromo(param1:Event) : void
      {
         var _loc2_:String = this.textInput.text;
         this.btnAccept.enabled = this.checkLocalKeyValidation();
      }
      
      protected function checkLocalKeyValidation() : Boolean
      {
         var _loc1_:String = this.textInput.text;
         trace("checkLocalNicknameValidation",_loc1_);
         return _loc1_.length >= this.MIN_CHARS && _loc1_.length <= this.MAX_CHARS;
      }
      
      protected function onMessageCancel(param1:ButtonEvent) : void
      {
         Message.onClick(_id,"no");
         removeEventListener(InputEvent.INPUT,this.handleInput);
      }
      
      protected function onMessageAccept(param1:ButtonEvent) : void
      {
         if(!this.btnAccept.enabled)
         {
            return;
         }
         this.requestKeyValidation();
      }
   }
}

