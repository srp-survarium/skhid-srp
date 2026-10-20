package survarium.clan
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ScrollingListSmooth;
   import scaleform.clik.controls.TextInput;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ComponentEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.gfx.FocusManager;
   import scaleform.gfx.TextFieldEx;
   import survarium.Util;
   import survarium.controls.NumericStepperSrv;
   import survarium.controls.PlayerInfo;
   import survarium.controls.PlayerSearchComponent;
   import survarium.controls.SrvContextMenu;
   import survarium.controls.SrvTextInput;
   import survarium.data.LocalizationData;
   import survarium.friends.FriendsMenu;
   
   public class CreateClanWindow extends UIComponent
   {
      
      public static const CLAN_RESERVE_STATE:uint = 0;
      
      public static const CLAN_REGISTRATION_STATE:uint = 1;
      
      public static const CLAN_INVITED_STATE:uint = 2;
      
      public var textFieldLabel:TextField;
      
      public var textFieldClanName:TextField;
      
      public var textFieldAbbreviation:TextField;
      
      public var textFieldAbbreviationInfo:TextField;
      
      public var textFieldRegister:TextField;
      
      public var textFieldCostValue:TextField;
      
      public var textFieldInfo1:TextField;
      
      public var textFieldInfo2:TextField;
      
      public var textFieldStatus:TextField;
      
      public var textFieldPaid:TextField;
      
      public var textFieldBrackets2:TextField;
      
      public var textFieldBrackets:TextField;
      
      public var textInputClanName:TextInput;
      
      public var textInputClanAbbreviation:TextInput;
      
      public var btnPayRegistration:Button;
      
      public var btnAccept:Button;
      
      public var btnInvitePlayer:Button;
      
      public var btnLeave:Button;
      
      public var btnReserveClanName:Button;
      
      public var btnCancel:Button;
      
      public var moneyIcon:MovieClip;
      
      public var lockArea:MovieClip;
      
      public var warningContainer:MovieClip;
      
      public var warningContainerAbb:MovieClip;
      
      public var loadingIndicator:MovieClip;
      
      public var loadingIndicatorAbb:MovieClip;
      
      public var moneyIcon2:MovieClip;
      
      public var payRegistrationStepper:NumericStepperSrv;
      
      public var clanInvitesList:ScrollingListSmooth;
      
      public var searchComponent:PlayerSearchComponent;
      
      private var _playersRequired:uint = 10;
      
      private var _lastClanName:String = "";
      
      private var _lastClanAbbreviation:String = "";
      
      private var _state:uint = 0;
      
      private var _clanCost:uint = 0;
      
      private var _registrationMoneyProgress:uint = 0;
      
      private var _frames:int = 0;
      
      public function CreateClanWindow()
      {
         super();
         this.lockArea.alpha = 0.7;
         this.setState(CLAN_RESERVE_STATE);
      }
      
      public function set totalPlayers(param1:uint) : void
      {
         _playersRequired = param1;
      }
      
      public function set clanPlayers(param1:uint) : void
      {
         this.textFieldStatus.text = LocalizationData.getName("clan_players_ready") + param1.toString() + "/" + this._playersRequired.toString();
      }
      
      public function set registrationPaid(param1:Boolean) : void
      {
         this.textFieldPaid.visible = param1;
         this.updateClanCreationEnabled();
      }
      
      public function setState(param1:uint) : void
      {
         _state = param1;
         if(this._state < 3)
         {
            if(currentFrame != 1)
            {
               gotoAndStop(1);
            }
         }
         else if(currentFrame != 2)
         {
            gotoAndStop(2);
         }
         this.btnAccept.visible = this.btnCancel.visible = this._state != CLAN_INVITED_STATE;
         this.btnLeave.visible = this._state == CLAN_INVITED_STATE;
         this.btnCancel.enabled = this.searchComponent.textInput.enabled = this._state == CLAN_REGISTRATION_STATE;
         if(!this.btnCancel.enabled)
         {
            this.btnInvitePlayer.enabled = false;
         }
         this.textInputClanName.enabled = this.textInputClanAbbreviation.enabled = this._state == CLAN_RESERVE_STATE;
         this.clanInvitesList.enabled = this._state == CLAN_REGISTRATION_STATE;
         this.textInputClanName.alpha = this.textInputClanAbbreviation.alpha = this._state == CLAN_RESERVE_STATE ? 1 : 0.7;
         this.clanInvitesList.alpha = this.clanInvitesList.enabled ? 1 : 0.5;
         this.btnReserveClanName.enabled = false;
         this.moneyIcon2.visible = this.payRegistrationStepper.visible = this.btnPayRegistration.visible = this.textFieldCostValue.visible = this.moneyIcon.visible = this.textFieldRegister.visible = this._state != CLAN_RESERVE_STATE;
         this.btnReserveClanName.visible = this._state == CLAN_RESERVE_STATE;
         if(this._state == CLAN_RESERVE_STATE)
         {
            this.lockArea.visible = true;
            this.lockArea.y = 233;
            this.lockArea.height = height - this.lockArea.y - 82;
         }
         else
         {
            this.lockArea.visible = false;
         }
      }
      
      public function updateClanCreationEnabled() : void
      {
         trace("updateClanCreationEnabled::",this.btnPayRegistration,this.textFieldPaid);
         this.payRegistrationStepper.enabled = this._registrationMoneyProgress < this._clanCost;
         this.textFieldPaid.text = "";
         this.btnPayRegistration.enabled = !this.textFieldPaid.visible && this.payRegistrationStepper.enabled;
         this.btnPayRegistration.label = LocalizationData.getName(this.textFieldPaid.visible ? "clan_registration_paid" : "clan_buy_registration_button_label");
         this.textFieldCostValue.styleSheet = MainScene.textStyleSheet;
         this.textFieldCostValue.text = Util.styledChar(this._registrationMoneyProgress.toString(),"highlight") + " / " + this._clanCost.toString();
         this.moneyIcon.x = this.textFieldCostValue.x + this.textFieldCostValue.width - this.textFieldCostValue.textWidth - this.moneyIcon.width - 5;
         this.textFieldRegister.width = this.moneyIcon.x - 20;
         trace("textFieldRegister::",this.textFieldRegister.text,this.textFieldRegister.textWidth);
         this.textFieldRegister.x = this.moneyIcon.x - this.textFieldRegister.width - this.moneyIcon.width;
         this.updateRegistrationStepper();
      }
      
      public function setClanCreationCostProgress(param1:uint) : void
      {
         _registrationMoneyProgress = param1;
         this.updateClanCreationEnabled();
      }
      
      public function setClanCreationCost(param1:uint) : void
      {
         _clanCost = param1;
         this.updateClanCreationEnabled();
      }
      
      public function setPlayerData(param1:Array) : void
      {
         this.clanInvitesList.dataProvider = new DataProvider(param1);
         trace("setPlayerData::",param1.length);
      }
      
      public function setClanName(param1:String, param2:String) : void
      {
         this.textInputClanName.text = param1;
         this.textInputClanAbbreviation.text = param2;
         this.textInputClanAbbreviation.validateNow();
         this.updateBrackets();
      }
      
      public function setClanPlayersRequired(param1:uint) : void
      {
         _playersRequired = param1;
      }
      
      public function handleClanAbbreviationValidation(param1:Boolean, param2:String, param3:String = null) : void
      {
         trace("handleClanAbbreviationValidation::",param1,param2,"l:",this._lastClanAbbreviation,param3);
         if(this._lastClanAbbreviation != param2)
         {
            return;
         }
         this.loadingIndicatorAbb.gotoAndStop(1);
         if(param1 == false)
         {
            this.warningContainerAbb.textField.text = param3;
            this.warningContainerAbb.visible = true;
            if(this.warningContainerAbb.currentFrame == 1)
            {
               this.warningContainerAbb.gotoAndPlay(2);
            }
            else if(this.warningContainerAbb.currentFrame > 10)
            {
               this.warningContainerAbb.gotoAndPlay(10);
            }
         }
      }
      
      public function handleClanNameValidation(param1:Boolean, param2:String, param3:String = null) : void
      {
         trace("handleClanNameValidation::",param1,param2,"l:",this._lastClanName,param3);
         if(this._lastClanName != param2)
         {
            return;
         }
         this.loadingIndicator.gotoAndStop(1);
         if(param1 == false)
         {
            this.warningContainer.textField.text = param3;
            this.warningContainer.visible = true;
            if(this.warningContainer.currentFrame == 1)
            {
               this.warningContainer.gotoAndPlay(2);
            }
            else if(this.warningContainer.currentFrame > 10)
            {
               this.warningContainer.gotoAndPlay(10);
            }
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.btnInvitePlayer.enabled = false;
         this.searchComponent.playersList.addEventListener(ListEvent.ITEM_CLICK,this.changeInputName);
         this.searchComponent.addEventListener(FocusEvent.FOCUS_IN,this.onTextInputFocus);
         this.searchComponent.addEventListener(FocusEvent.FOCUS_OUT,this.onTextInputFocusOut);
         this.searchComponent.textInput.addEventListener(Event.CHANGE,this.onNicknameChange);
         this.searchComponent.textInput.addEventListener(ComponentEvent.STATE_CHANGE,this.onNicknameChange);
         this.textFieldBrackets2.mouseEnabled = this.textFieldBrackets.mouseEnabled = false;
         this.textFieldBrackets2.visible = this.textFieldBrackets.visible = false;
         this.textFieldBrackets.x += 3;
         this.textFieldBrackets2.y -= 1;
         this.textFieldBrackets.y -= 1;
         LocalizationData.addCallback(this.onLocalization,"clan_window_label");
         this.btnPayRegistration.addEventListener(ButtonEvent.CLICK,this.onBuyRegistration);
         this.btnAccept.addEventListener(ButtonEvent.CLICK,this.onRegisterClanClicked);
         this.btnInvitePlayer.addEventListener(ButtonEvent.CLICK,this.onInvitePlayerClicked);
         this.textInputClanName.textField.addEventListener(Event.CHANGE,this.requestNameValidation);
         this.textInputClanAbbreviation.textField.addEventListener(Event.CHANGE,this.requestAbbreviationValidation);
         this.textInputClanAbbreviation.addEventListener(SrvTextInput.RESET_TEXT,this.onTextUpdate);
         this.warningContainerAbb.visible = this.warningContainer.visible = false;
         this.setClanCreationCost(0);
         clanPlayers = 0;
         this.textFieldPaid.visible = false;
         this.btnPayRegistration.enabled = true;
         this.btnAccept.enabled = false;
         this.loadingIndicator.gotoAndStop(1);
         this.loadingIndicatorAbb.gotoAndStop(1);
         this.btnReserveClanName.addEventListener(ButtonEvent.CLICK,this.reserveClanName);
         this.btnLeave.addEventListener(ButtonEvent.CLICK,this.leaveClan);
         this.btnCancel.addEventListener(ButtonEvent.CLICK,this.cancelClanRegistration);
         this.payRegistrationStepper.editable = true;
         this.payRegistrationStepper.minimum = 0;
         this.payRegistrationStepper.maximum = 1000;
         this.payRegistrationStepper.stepSize = 1000;
         this.payRegistrationStepper.value = 0;
         this.updateRegistrationStepper();
      }
      
      private function onNicknameChange(param1:Event) : void
      {
         this.btnInvitePlayer.enabled = this.searchComponent.textInput.text != "" && this.searchComponent.textInput.text != this.searchComponent.textInput.defaultText;
      }
      
      private function onTextUpdate(param1:Event) : void
      {
         trace("onTextUpdate",this.textInputClanAbbreviation.textField.text,this.textInputClanAbbreviation.text);
         this.updateBrackets();
      }
      
      private function onTextInputFocusOut(param1:FocusEvent) : void
      {
         trace("onTextInputFocusOut::event");
         _frames = 0;
         stage.addEventListener(Event.EXIT_FRAME,this.onRednerAfterFocus);
      }
      
      private function inputNamePress(param1:Event) : void
      {
         stage.removeEventListener(Event.EXIT_FRAME,this.onRednerAfterFocus);
      }
      
      private function onRednerAfterFocus(param1:Event) : void
      {
         trace("onRednerAfterFocus::",this._frames,stage.focus);
         ++this._frames;
         if(this._frames < 20 && !stage.focus)
         {
            return;
         }
         _frames = 0;
         stage.removeEventListener(Event.EXIT_FRAME,this.onRednerAfterFocus);
         trace("onTextInputFocusOut",FocusManager.getFocus(),stage.focus,stage.focus ? this.searchComponent.contains(stage.focus) : null,this.searchComponent.hasFocus,this.searchComponent.focused,this.searchComponent.focusable);
         if((SrvContextMenu.getInstance()) && (SrvContextMenu.getInstance().parent))
         {
            return;
         }
         if(stage.focus && this.searchComponent.contains(stage.focus))
         {
            return;
         }
         this.searchComponent.cleanUp(false);
      }
      
      private function onTextInputFocus(param1:FocusEvent) : void
      {
         trace("searchComponent::focusIn",stage.focus,param1.target,param1.currentTarget);
         this.searchComponent.setFriendsList(FriendsMenu.getFriendsList());
      }
      
      private function changeInputName(param1:ListEvent) : void
      {
         trace("changeInputName::click");
         if(!param1 || (SrvContextMenu.getInstance()) && (SrvContextMenu.getInstance().parent))
         {
            return;
         }
         var _loc2_:* = param1.itemData;
         var _loc3_:Object;
         trace("changeInputName::",_loc2_);
         stage.removeEventListener(Event.EXIT_FRAME,this.onRednerAfterFocus);
         this.searchComponent.removeEventListener(FocusEvent.FOCUS_OUT,this.onTextInputFocusOut);
         this.searchComponent.cleanUp();
         if(_loc2_)
         {
            _loc3_ = _loc2_;
            trace("value::",_loc3_.name);
            this.searchComponent.textInput.text = _loc3_.name;
            this.btnInvitePlayer.enabled = true;
         }
         this.searchComponent.addEventListener(FocusEvent.FOCUS_OUT,this.onTextInputFocusOut);
      }
      
      private function cancelClanRegistration(param1:ButtonEvent) : void
      {
         ExternalInterface.call("cancel_clan_registration");
      }
      
      private function updateRegistrationStepper() : void
      {
         var _loc1_:int = Math.min(this._clanCost - this._registrationMoneyProgress,PlayerInfo.genericMoney);
         trace("updateRegistrationStepper",_loc1_,this._clanCost,this._registrationMoneyProgress);
         this.payRegistrationStepper.maximum = _loc1_;
      }
      
      private function leaveClan(param1:ButtonEvent) : void
      {
         ExternalInterface.call("leave_clan");
      }
      
      private function reserveClanName(param1:ButtonEvent) : void
      {
         trace("reserveClanName::",this.textInputClanName.text,this.textInputClanAbbreviation.text);
         ExternalInterface.call("reserve_clan_name",this.textInputClanName.text,this.textInputClanAbbreviation.text);
      }
      
      private function onBuyRegistration(param1:ButtonEvent) : void
      {
         trace("onBuyRegistration::",this.payRegistrationStepper.value);
         var _loc2_:uint = this.payRegistrationStepper.value;
         ExternalInterface.call("buy_clan_registration",_loc2_);
      }
      
      private function requestAbbreviationValidation(param1:Event) : void
      {
         _lastClanAbbreviation = this.textInputClanAbbreviation.textField.text;
         this.updateBrackets();
         trace("_lastClanAbbreviation::",this._lastClanAbbreviation);
         this.loadingIndicatorAbb.gotoAndPlay(1);
         this.btnReserveClanName.enabled = false;
         ExternalInterface.call("request_clan_abbreviation_validation",this._lastClanAbbreviation);
      }
      
      private function updateBrackets() : void
      {
         var _loc1_:String = this.textInputClanAbbreviation.textField.text;
         this.textFieldBrackets2.x = this.textInputClanAbbreviation.x + this.textInputClanAbbreviation.textField.textWidth + 12;
         this.textFieldBrackets2.visible = this.textFieldBrackets.visible = _loc1_ != "" && _loc1_ != this.textInputClanAbbreviation.defaultText;
      }
      
      private function requestNameValidation(param1:Event) : void
      {
         _lastClanName = this.textInputClanName.textField.text;
         trace("_lastClanName::",this._lastClanName);
         this.loadingIndicator.gotoAndPlay(1);
         this.btnReserveClanName.enabled = false;
         ExternalInterface.call("request_clan_name_validation",this._lastClanName);
      }
      
      private function onInvitePlayerClicked(param1:ButtonEvent) : void
      {
         trace("invite_player_to_clan::",this.searchComponent.textInput.text);
         ExternalInterface.call("invite_player_to_clan",this.searchComponent.textInput.text);
         this.searchComponent.cleanUp();
         this.btnInvitePlayer.enabled = false;
      }
      
      private function onRegisterClanClicked(param1:ButtonEvent) : void
      {
         ExternalInterface.call("create_clan");
      }
      
      private function onLocalization(param1:String = null) : void
      {
         this.btnCancel.label = LocalizationData.getName("clan_cancel_button_label");
         this.btnLeave.label = LocalizationData.getName("clan_leave_button_label");
         this.btnReserveClanName.label = LocalizationData.getName("clan_reserve_name_button_label");
         this.textFieldLabel.text = param1;
         this.textFieldClanName.text = LocalizationData.getName("clan_name");
         this.textFieldAbbreviation.text = LocalizationData.getName("clan_abbreviation");
         this.textFieldAbbreviationInfo.text = LocalizationData.getName("clan_abbreviation_info");
         this.textFieldRegister.multiline = true;
         TextFieldEx.setVerticalAlign(this.textFieldRegister,TextFieldEx.VALIGN_CENTER);
         this.textFieldRegister.text = LocalizationData.getName("clan_registration_cost");
         trace("textFieldRegister",LocalizationData.getName("clan_registration_cost"));
         this.textFieldInfo1.text = LocalizationData.getName("clan_creation_info_label");
         this.textFieldInfo2.text = LocalizationData.getName("clan_creation_info_description");
         this.btnInvitePlayer.label = LocalizationData.getName("clan_invite_player_button_label");
         this.textInputClanName.defaultText = LocalizationData.getName("clan_name_default_text");
         this.textInputClanAbbreviation.defaultText = LocalizationData.getName("clan_abbreviation_default_text");
         this.btnAccept.label = LocalizationData.getName("clan_accept_button_label");
         this.btnCancel.label = LocalizationData.getName("clan_cancel_button_label");
      }
   }
}

