package survarium.messages
{
   import flash.display.MovieClip;
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
   
   public class MessageEnterPromo extends MessageBase
   {
      
      public var btnAccept:Button;
      
      public var textInput:SrvTextInput;
      
      public var loadingIndicator:MovieClip;
      
      public var warning:MovieClip;
      
      private var _lastPromo:String = null;
      
      protected const MIN_CHARS:int = 3;
      
      protected const MAX_CHARS:int = 20;
      
      public function MessageEnterPromo()
      {
         super();
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         ExternalInterface.call("sound_play",13);
         this.warning.visible = false;
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
      
      public function close() : void
      {
         this.onMessageCancel(null);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.btnAccept.label = _data.label_ok;
         this.btnAccept.addEventListener(ButtonEvent.CLICK,this.onMessageAccept,false,0,true);
         this.textInput.addEventListener(Event.CHANGE,this.updatePromo);
         tabEnabled = true;
         this.textInput.maxChars = this.MAX_CHARS;
         this.textInput.defaultText = _data.default_text;
         this.btnAccept.enabled = false;
         this.loadingIndicator.gotoAndStop(1);
         this.loadingIndicator.visible = false;
      }
      
      private function requestKeyValidation(event:Event = null) : void
      {
         trace("requestCodeValidation::",this.textInput.text);
         this.btnAccept.enabled = false;
         this.textInput.enabled = false;
         this.loadingIndicator.gotoAndPlay(1);
         this.loadingIndicator.visible = false;
         ExternalInterface.call("request_promo_key_validation",this.textInput.text);
      }
      
      private function updatePromo(event:Event) : void
      {
         var promo:String = this.textInput.text;
         trace("updatePromo",promo);
         _lastPromo = promo;
         this.loadingIndicator.gotoAndStop(1);
         this.loadingIndicator.visible = false;
         this.btnAccept.enabled = this.checkLocalKeyValidation();
      }
      
      protected function checkLocalKeyValidation() : Boolean
      {
         trace("checkLocalNicknameValidation",this._lastPromo);
         return this._lastPromo.length >= this.MIN_CHARS && this._lastPromo.length <= this.MAX_CHARS;
      }
      
      public function handleCodeValidation(isValid:Boolean, warningText:String = null) : void
      {
         this.loadingIndicator.gotoAndStop(1);
         this.loadingIndicator.visible = false;
         this.btnAccept.enabled = this.checkLocalKeyValidation();
         this.textInput.enabled = true;
         if(isValid == false)
         {
            this.warning.showWarning(warningText);
            this.warning.visible = true;
         }
         else
         {
            Message.onClick(_id,"yes",_data);
         }
      }
      
      protected function onMessageCancel(event:ButtonEvent) : void
      {
         Message.onClick(_id,"no");
         removeEventListener(InputEvent.INPUT,this.handleInput);
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

