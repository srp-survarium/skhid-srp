package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.TextEvent;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.ui.Keyboard;
   import flash.utils.Timer;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   import survarium.buttons.CheckBoxValue;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;
   
   public class MessageEnterName extends MessageBase
   {
      
      public var btnAccept:Button;
      
      public var btnCancel:Button;
      
      public var btnClose:Button;
      
      public var background:MovieClip;
      
      public var radioButtonPremium:CheckBoxValue;
      
      public var radioButtonVouchers:CheckBoxValue;
      
      public var textFieldLabel:TextField;
      
      public var nickName:TextField;
      
      public var warning:MovieClip;
      
      public var warningGlow:MovieClip;
      
      protected var _nickname:String;
      
      private var lastNickName:String = null;
      
      private var lastCaretIndex:int = -1;
      
      private const BANNED_CHARS:String = "[]><|?*/\\:\".\'";
      
      protected var warning_text:String = "[ ] > < | ? * /  : \".";
      
      protected const MIN_CHARS:int = 3;
      
      protected const MAX_CHARS:int = 20;
      
      public var progressIndicator:MovieClip;
      
      protected var timer:Timer = new Timer(1000,1);
      
      protected var timerRequest:Timer = new Timer(6000,1);
      
      protected var _validNickname:Boolean = false;
      
      public function MessageEnterName()
      {
         super();
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         ExternalInterface.call("sound_play",13);
      }
      
      override public function get width() : Number
      {
         return this.background.width;
      }
      
      override public function get height() : Number
      {
         return this.background.height;
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
            case Keyboard.BACK:
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
         trace("configMessage::",this.btnAccept,this.btnCancel,this.textFieldLabel,this.nickName,this.warning);
         this.btnAccept.label = LocalizationData.getName("change");
         this.btnCancel.label = LocalizationData.getName("cancel");
         this.textFieldLabel.text = LocalizationData.getName("change_name_label");
         super.configUI();
         this.btnAccept.addEventListener(ButtonEvent.CLICK,this.onMessageAccept,false,0,true);
         this.btnCancel.addEventListener(ButtonEvent.CLICK,this.onMessageCancel,false,0,true);
         this.nickName.addEventListener(Event.CHANGE,this.updateNickname);
         this.nickName.addEventListener(TextEvent.TEXT_INPUT,this.updateCaretIndex);
         tabEnabled = true;
         this.nickName.maxChars = this.MAX_CHARS;
         _nickname = _data.nickname;
         var bracketIndex:int = this._nickname.indexOf("]");
         if(bracketIndex != -1)
         {
            _nickname = this._nickname.substr(bracketIndex + 1,this._nickname.length);
         }
         this.nickName.text = this._nickname;
         lastNickName = this._nickname;
         this.warning.visible = false;
         this.radioButtonPremium.money_type = 1;
         this.radioButtonVouchers.money_type = 9;
         this.radioButtonPremium.value = _data.premiumCost;
         this.radioButtonVouchers.value = _data.vouchersCost;
         this.radioButtonVouchers.selected = _data.selectVoucher;
         this.radioButtonPremium.selected = !this.radioButtonVouchers.selected;
         this.btnAccept.enabled = false;
         this.timer.addEventListener(TimerEvent.TIMER,this.requestNameValidation);
         this.timerRequest.addEventListener(TimerEvent.TIMER,this.handleRequestFailed);
         this.radioButtonPremium.addEventListener(ButtonEvent.CLICK,this.handlePayTypeSelection);
         this.radioButtonVouchers.addEventListener(ButtonEvent.CLICK,this.handlePayTypeSelection);
         this.progressIndicator.gotoAndStop(1);
         this.progressIndicator.visible = false;
         this.timer.delay = _data.changeNameDelay;
         this.timerRequest.delay = _data.changeNameFailedDelay;
         this.radioButtonPremium.toggle = false;
         this.radioButtonVouchers.toggle = false;
         if(stage)
         {
            stage.focus = this.nickName;
            this.nickName.setSelection(0,this.nickName.text.length);
         }
      }
      
      private function handlePayTypeSelection(e:ButtonEvent) : void
      {
         if(e.target == this.radioButtonPremium)
         {
            this.radioButtonPremium.selected = true;
            this.radioButtonVouchers.selected = false;
         }
         else
         {
            this.radioButtonVouchers.selected = true;
            this.radioButtonPremium.selected = false;
         }
         if(this._validNickname)
         {
            this.btnAccept.enabled = this.checkLocalNicknameValidation();
         }
      }
      
      private function handleRequestFailed(e:TimerEvent = null) : void
      {
         this.timerRequest.stop();
         this.progressIndicator.gotoAndStop(1);
         this.progressIndicator.visible = false;
      }
      
      private function requestNameValidation(e:TimerEvent = null) : void
      {
         trace("request_nickname::",this.lastNickName);
         this.timerRequest.reset();
         this.timerRequest.start();
         ExternalInterface.call("request_nickname_validation",this.lastNickName);
      }
      
      private function updateCaretIndex(e:Event) : void
      {
         lastCaretIndex = this.nickName.caretIndex;
      }
      
      private function updateNickname(e:Event) : void
      {
         var text:String = this.nickName.text;
         trace("updateNickname",text);
         var useOfBannedChars:Boolean = false;
         lastNickName = text;
         this.progressIndicator.gotoAndStop(1);
         this.progressIndicator.visible = false;
         if(this.lastNickName != this._nickname)
         {
            this.timer.reset();
            this.timer.start();
            this.progressIndicator.gotoAndPlay(1);
            this.progressIndicator.visible = true;
            _validNickname = false;
         }
         this.btnAccept.enabled = false;
      }
      
      protected function checkLocalNicknameValidation() : Boolean
      {
         trace("_data.vouchersCost",_data.vouchersCost,PlayerInfo.getVoucherCount(0),this.radioButtonPremium.selected,_data.voucherId);
         return this.lastNickName.length >= this.MIN_CHARS && this.lastNickName.length <= this.MAX_CHARS && (this.radioButtonPremium.selected ? _data.premiumCost <= PlayerInfo.premiumMoney : _data.vouchersCost <= PlayerInfo.getVoucherCount(0));
      }
      
      public function handleNicknameValidation(result:Boolean, nickname:String, message:String = null) : void
      {
         if(this.lastNickName != nickname)
         {
            return;
         }
         this.timerRequest.stop();
         this.progressIndicator.gotoAndStop(1);
         this.progressIndicator.visible = false;
         _validNickname = result;
         this.btnAccept.enabled = result && this.checkLocalNicknameValidation();
         if(result == false)
         {
            this.warning.visible = true;
            this.warning.showWarning(message);
         }
      }
      
      protected function onMessageCancel(event:ButtonEvent) : void
      {
         Message.onClick(_id,"no");
         removeEventListener(InputEvent.INPUT,this.handleInput);
         this.timerRequest.stop();
         this.timer.stop();
      }
      
      protected function onMessageAccept(event:ButtonEvent) : void
      {
         if(!this.btnAccept.enabled)
         {
            return;
         }
         Message.onClick(_id,"yes",_data);
         ExternalInterface.call("change_player_nickname",this.nickName.text,uint(this.radioButtonPremium.selected ? 0 : _data.voucherId));
         this.timerRequest.stop();
         this.timer.stop();
      }
   }
}

