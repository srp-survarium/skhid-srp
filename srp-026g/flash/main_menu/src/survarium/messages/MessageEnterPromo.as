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
      
      private function requestKeyValidation(param1:Event = null) : void
      {
         trace("requestCodeValidation::",this.textInput.text);
         this.btnAccept.enabled = false;
         this.textInput.enabled = false;
         this.loadingIndicator.gotoAndPlay(1);
         this.loadingIndicator.visible = false;
         ExternalInterface.call("request_promo_key_validation",this.textInput.text);
      }
      
      private function updatePromo(param1:Event) : void
      {
         var _loc2_:String = this.textInput.text;
         trace("updatePromo",_loc2_);
         _lastPromo = _loc2_;
         this.loadingIndicator.gotoAndStop(1);
         this.loadingIndicator.visible = false;
         this.btnAccept.enabled = this.checkLocalKeyValidation();
      }
      
      protected function checkLocalKeyValidation() : Boolean
      {
         trace("checkLocalNicknameValidation",this._lastPromo);
         return this._lastPromo.length >= this.MIN_CHARS && this._lastPromo.length <= this.MAX_CHARS;
      }
      
      public function handleCodeValidation(param1:Boolean, param2:String = null) : void
      {
         this.loadingIndicator.gotoAndStop(1);
         this.loadingIndicator.visible = false;
         this.btnAccept.enabled = this.checkLocalKeyValidation();
         this.textInput.enabled = true;
         if(param1 == false)
         {
            this.warning.showWarning(param2);
            this.warning.visible = true;
         }
         else
         {
            Message.onClick(_id,"yes",_data);
         }
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

