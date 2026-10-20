package survarium.clan
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ScrollingListSmoothDynam;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.gfx.FocusManager;
   import scaleform.gfx.MouseEventEx;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.controls.PlayerSearchComponent;
   import survarium.controls.SrvContextMenu;
   import survarium.data.LocalizationData;
   
   public class TournamentWindow extends UIComponent
   {
      
      private static var _isCommander:Boolean = false;
      
      public static const STATE_COMMANDER:uint = 0;
      
      public static const STATE_SOLDIER:uint = 1;
      
      public static const STATE_RESERVE:uint = 2;
      
      public static const STATE_INVITES:uint = 3;
      
      public static const TOGGLE_FOLD:String = "toggleFold";
      
      public var textFieldLabel:TextField;
      
      public var playersList:ScrollingListSmoothDynam;
      
      public var icon:MovieClip;
      
      public var btnHide:Button;
      
      public var btnAddPlayer:Button;
      
      public var btnDisbandTeam:Button;
      
      public var btnLeaveJoin:Button;
      
      public var btnShow:Button;
      
      public var btnBecomeCommander:Button;
      
      public var btnClose:Button;
      
      public var searchComponent:PlayerSearchComponent;
      
      public var background:MovieClip;
      
      public var smallBackground:MovieClip;
      
      private var _hasCommander:Boolean = false;
      
      private var _inTeam:Boolean = false;
      
      protected var _statusLabels:Array = ["clan_team_commander","clan_team_soldier","clan_team_reserve","clan_team_invites",""];
      
      protected var _isAdmin:Boolean = false;
      
      protected var selectedName:String = null;
      
      private var renderFrames:uint = 0;
      
      private var _fullSize:Boolean = true;
      
      public function TournamentWindow()
      {
         super();
         this.showFullSize(true);
      }
      
      public static function isCommander() : Boolean
      {
         return _isCommander;
      }
      
      public function isFold() : Boolean
      {
         return !this._fullSize;
      }
      
      public function fold() : void
      {
         if(this._fullSize)
         {
            this.onHideMe(null);
         }
      }
      
      public function fillFoundPlayers(param1:Array) : void
      {
         this.searchComponent.fillFoundPlayers(param1);
      }
      
      public function init() : void
      {
         _isAdmin = ClanWindow.getState() == 4;
         this.btnDisbandTeam.label = this._isAdmin ? LocalizationData.getName("btn_disband_clan_team") : LocalizationData.getName("btn_leave_clan_team");
         ExternalInterface.call("clan_team_window_opened");
         this.updateButtons();
      }
      
      private function createClanTeam(param1:Event = null) : void
      {
         ExternalInterface.call("create_clan_team");
      }
      
      private function updateButtons() : void
      {
         var _loc1_:Boolean = _isCommander;
         var _loc2_:int;
         this.btnDisbandTeam.visible = _loc1_ && this._fullSize;
         this.btnAddPlayer.visible = _loc1_ && this._fullSize;
         this.btnLeaveJoin.visible = !_loc1_ && this._fullSize;
         this.btnDisbandTeam.enabled = _isCommander;
         this.btnBecomeCommander.visible = PlayerInfo.isTeamCommander && !this._hasCommander && this._fullSize;
         this.btnBecomeCommander.label = LocalizationData.getName("btn_become_commander");
         trace("updateButtons::",PlayerInfo.isTeamCommander,this._hasCommander,this._inTeam);
         this.btnLeaveJoin.enabled = true;
         if(PlayerInfo.isTeamCommander)
         {
            if(_isCommander)
            {
               this.btnLeaveJoin.label = LocalizationData.getName("btn_create_clan_team");
            }
            else if(!this._inTeam)
            {
               this.btnLeaveJoin.enabled = !PlayerInfo.isInSquad;
               this.btnLeaveJoin.label = LocalizationData.getName("btn_join_team");
            }
            else
            {
               this.btnLeaveJoin.label = LocalizationData.getName("btn_leave_clan_team");
            }
         }
         else if(!this._inTeam)
         {
            this.btnLeaveJoin.enabled = !PlayerInfo.isInSquad;
            this.btnLeaveJoin.label = LocalizationData.getName("btn_join_team");
         }
         else
         {
            this.btnLeaveJoin.label = LocalizationData.getName("btn_leave_clan_team");
         }
         this.btnLeaveJoin.validateNow();
         if(!this.btnBecomeCommander.visible)
         {
            _loc2_ = this.btnLeaveJoin.textField.textWidth + 15;
            this.btnLeaveJoin.width = _loc2_;
            this.btnLeaveJoin.x = (this.background.width - _loc2_) / 2;
         }
         else
         {
            this.btnLeaveJoin.x = 170;
            this.btnLeaveJoin.width = 140;
         }
      }
      
      public function setPlayersData(param1:Array) : void
      {
         _hasCommander = param1.length && param1[0].status == 0;
         _isCommander = this._hasCommander && param1[0].name == PlayerInfo.nickname;
         trace("TW::setPlayersData",this._hasCommander,_isCommander);
         _inTeam = false;
         var _loc2_:uint = 0;
         var _loc3_:uint;
         while(_loc2_ < param1.length)
         {
            _loc3_ = (param1[_loc2_].status);
            param1[_loc2_].movableUp = _isCommander && (_loc3_ == STATE_INVITES || _loc3_ == STATE_RESERVE);
            param1[_loc2_].movableDown = _isCommander && _loc3_ == STATE_SOLDIER;
            param1[_loc2_].favoritable = this._isAdmin;
            if(param1[_loc2_].name == PlayerInfo.nickname)
            {
               _inTeam = true;
            }
            _loc2_++;
         }
         PlayerInfo.isInTeam = this._inTeam;
         this.addSeparators(param1);
         this.playersList.dataProvider = new DataProvider(param1);
         this.updateButtons();
      }
      
      protected function addSeparators(param1:Array) : void
      {
         var _loc2_:int = 0;
         var _loc3_:uint;
         var _loc4_:Object;
         if(param1)
         {
            _loc3_ = 0;
            while(_loc3_ < param1.length)
            {
               if(param1[_loc3_])
               {
                  if(_loc2_ != param1[_loc3_].status)
                  {
                     _loc4_ = {
                        status_text:LocalizationData.getName(this._statusLabels[param1[_loc3_].status]),
                        isSeparator:true,
                        status:-1
                     };
                     _loc2_ = (param1[_loc3_].status);
                     param1.splice(_loc3_,0,_loc4_);
                     _loc3_++;
                  }
               }
               _loc3_++;
            }
         }
      }
      
      override protected function configUI() : void
      {
         this.playersList.addEventListener(MouseEvent.CLICK,this.onMouseItemClick);
         this.btnHide.addEventListener(ButtonEvent.CLICK,this.onHideMe);
         this.btnClose.addEventListener(ButtonEvent.CLICK,this.onCloseMe);
         this.btnAddPlayer.addEventListener(ButtonEvent.CLICK,this.inviteMember);
         this.btnDisbandTeam.addEventListener(ButtonEvent.CLICK,this.disbandTeam);
         this.btnShow.addEventListener(ButtonEvent.CLICK,this.onShowMe);
         this.searchComponent.visible = false;
         this.searchComponent.setDropDown(false);
         this.searchComponent.playersList.addEventListener(ListEvent.ITEM_CLICK,this.invitePlayerByName);
         this.searchComponent.addEventListener(FocusEvent.FOCUS_OUT,this.onTextInputFocusOut);
         this.playersList.addEventListener(ButtonEvent.CLICK,this.onPlayerListClicked);
         this.btnLeaveJoin.addEventListener(ButtonEvent.CLICK,this.onBtnLeaveJoin);
         this.btnBecomeCommander.addEventListener(ButtonEvent.CLICK,this.onBtnBecomeCommander);
         this.textFieldLabel.mouseEnabled = false;
         this.textFieldLabel.text = LocalizationData.getName("clan_team_window_label");
         this.icon.x = this.textFieldLabel.getLineMetrics(0).x + this.textFieldLabel.x - this.icon.width - 2;
         LocalizationData.addButton(this.btnAddPlayer,"btn_clan_team_invite_memeber");
      }
      
      private function onBtnBecomeCommander(param1:ButtonEvent) : void
      {
         this.createClanTeam();
      }
      
      private function onBtnLeaveJoin(param1:ButtonEvent) : void
      {
         if(this.btnLeaveJoin.label == LocalizationData.getName("btn_create_clan_team"))
         {
            this.createClanTeam();
         }
         if(this.btnLeaveJoin.label == LocalizationData.getName("btn_join_team"))
         {
            ExternalInterface.call("request_to_join_clan_team");
         }
         if(this.btnLeaveJoin.label == LocalizationData.getName("btn_leave_clan_team"))
         {
            ExternalInterface.call("leave_clan_team");
         }
      }
      
      private function onPlayerListClicked(param1:ButtonEvent) : void
      {
         var _loc2_:Object;
         trace("onPlayerListClicked::",param1.target,param1.currentTarget,param1.target.name);
         if(param1.target.name == "btnAddToFavorite" || param1.target.name == "btnMovePlayerUp" || param1.target.name == "btnMovePlayer")
         {
            _loc2_ = param1.target.parent.data;
            if(param1.target.name == "btnAddToFavorite")
            {
               selectedName = _loc2_.name;
               this.moveToCommander();
            }
            else if(param1.target.name == "btnMovePlayerUp" || param1.target.name == "btnMovePlayer")
            {
               selectedName = _loc2_.name;
               if(_loc2_.status == STATE_SOLDIER || _loc2_.status == STATE_INVITES)
               {
                  this.moveToReserve();
               }
               else if(_loc2_.status == STATE_RESERVE)
               {
                  this.moveToTeam();
               }
            }
         }
      }
      
      public function disbandTeam(param1:ButtonEvent = null) : void
      {
         if(this._isAdmin || _isCommander)
         {
            ExternalInterface.call("disband_clan_team");
         }
         else
         {
            ExternalInterface.call("leave_clan_team");
         }
      }
      
      private function onMouseItemClick(param1:MouseEventEx) : void
      {
         var _loc2_:*;
         if(param1.target && param1.target.parent is SrvClanTeamListRenderer)
         {
            _loc2_ = param1.target.parent;
         }
         else if(param1.target && param1.target.parent.parent is SrvClanTeamListRenderer)
         {
            _loc2_ = param1.target.parent.parent;
         }
         else
         {
            _loc2_ = param1.target;
         }
         trace("onMouseItemClick:::",param1.target,param1.currentTarget,_loc2_);
         if(param1.buttonIdx == 0 || this.playersList.enabled == false)
         {
            return;
         }
         this.showContextMenu(_loc2_);
      }
      
      public function showContextMenu(param1:*) : void
      {
         if(!param1 || param1.enabled == false || !param1.data)
         {
            return;
         }
         var _loc2_:Object = param1.data;
         var _loc3_:Boolean = ClanWindow.getState() == 4;
         var _loc4_:Boolean = _isCommander;
         var _loc5_:Array = new Array();
         trace("isCommander::",_loc4_,_loc3_);
         if(_loc2_.status == STATE_COMMANDER)
         {
            _loc5_.push({
               name:LocalizationData.getName("oust_commander"),
               click_function:this.oustCommander
            });
         }
         if(_loc2_.status == STATE_SOLDIER && _loc4_ && this._inTeam)
         {
            _loc5_.push({
               name:LocalizationData.getName("move_to_reserve"),
               click_function:this.moveToReserve
            });
            _loc5_.push({
               name:LocalizationData.getName("remove_from_team"),
               click_function:this.removeFromTeam
            });
         }
         if(_loc2_.status == STATE_RESERVE && _loc4_ && this._inTeam)
         {
            _loc5_.push({
               name:LocalizationData.getName("move_to_team"),
               click_function:this.moveToTeam
            });
            _loc5_.push({
               name:LocalizationData.getName("remove_from_team"),
               click_function:this.removeFromTeam
            });
         }
         if(_loc2_.status == STATE_INVITES && _loc4_ && this._inTeam)
         {
            _loc5_.push({
               name:LocalizationData.getName("move_to_reserve"),
               click_function:this.moveToReserve
            });
         }
         trace("showContextMenu::",_loc2_.status,"options",_loc5_.length);
         if(_loc5_.length)
         {
            SrvContextMenu.show(_loc5_);
         }
         selectedName = _loc2_.name;
      }
      
      private function moveToTeam() : void
      {
         trace("change_status_clan_team_member",this.selectedName,STATE_SOLDIER);
         ExternalInterface.call("change_status_clan_team_member",this.selectedName,STATE_SOLDIER);
      }
      
      private function removeFromTeam() : void
      {
         trace("remvoe_team_member",this.selectedName);
         ExternalInterface.call("remove_team_member",this.selectedName);
      }
      
      private function moveToReserve() : void
      {
         trace("change_status_clan_team_member",this.selectedName,STATE_RESERVE);
         ExternalInterface.call("change_status_clan_team_member",this.selectedName,STATE_RESERVE);
      }
      
      private function moveToCommander() : void
      {
         trace("set_another_clan_team_commander",this.selectedName);
         ExternalInterface.call("set_another_clan_team_commander",this.selectedName);
      }
      
      private function oustCommander() : void
      {
         trace("oust_commander");
         ExternalInterface.call("oust_commander");
      }
      
      private function onTextInputFocusOut(param1:FocusEvent) : void
      {
         trace("onTextInputFocusOut",FocusManager.getFocus(),stage.focus,stage.focus ? this.searchComponent.contains(stage.focus) : null);
         stage.addEventListener(Event.RENDER,this.onRender);
      }
      
      private function onRender(param1:Event) : void
      {
         ++this.renderFrames;
         if(this.renderFrames == 12 || stage.focus)
         {
            renderFrames = 0;
            stage.removeEventListener(Event.RENDER,this.onRender);
            trace("onRender",this.renderFrames,FocusManager.getFocus(),stage.focus,stage.focus ? this.searchComponent.contains(stage.focus) : null);
            if((SrvContextMenu.getInstance()) && (SrvContextMenu.getInstance().parent))
            {
               return;
            }
            if(stage.focus && this.searchComponent.contains(stage.focus))
            {
               return;
            }
            this.searchComponent.cleanUp();
            this.searchComponent.visible = false;
            this.searchComponent.x = 0;
         }
      }
      
      private function invitePlayerByName(param1:ListEvent) : void
      {
         trace("changeInputName::init");
         if(!param1 || (SrvContextMenu.getInstance()) && (SrvContextMenu.getInstance().parent))
         {
            return;
         }
         var _loc2_:* = param1.itemData;
         trace("changeInputName::",_loc2_);
         if(_loc2_)
         {
            this.searchComponent.cleanUp();
            this.searchComponent.visible = false;
            this.searchComponent.x = 0;
            stage.focus = this;
            trace("invite_player_to_clan_team",_loc2_.name);
            ExternalInterface.call("invite_player_to_clan_team",_loc2_.name);
         }
      }
      
      private function inviteMember(param1:ButtonEvent) : void
      {
         this.searchComponent.visible = true;
         stage.focus = this.searchComponent;
         var _loc2_:Array = ClanWindow.getMembersList();
         this.searchComponent.setFriendsList(_loc2_);
      }
      
      private function showFullSize(param1:Boolean) : void
      {
         _fullSize = param1;
         this.btnClose.visible = this.btnHide.visible = this.background.visible = this.btnAddPlayer.visible = this.btnDisbandTeam.visible = this.playersList.visible = param1;
         this.btnBecomeCommander.visible = this.btnLeaveJoin.visible = param1;
         if(param1)
         {
            this.updateButtons();
         }
         this.btnShow.visible = this.smallBackground.visible = !param1;
      }
      
      private function onShowMe(param1:ButtonEvent) : void
      {
         trace("TW::onShowMe");
         ExternalInterface.call("clan_team_window_opened");
         gotoAndStop(1);
         this.showFullSize(true);
         dispatchEvent(new Event(TOGGLE_FOLD));
      }
      
      private function onCloseMe(param1:ButtonEvent) : void
      {
         this.showCloseMessage();
      }
      
      protected function showCloseMessage() : void
      {
         var _loc1_:Object = {
            text:LocalizationData.getName(_isCommander ? "clan_window_close_message_commander" : "clan_window_close_message"),
            onYes:this.closeWindow,
            label_ok:LocalizationData.getName("ok"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",_loc1_,true,0,true);
      }
      
      protected function closeWindow() : void
      {
         trace("TW::closeWindow");
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      private function onHideMe(param1:ButtonEvent) : void
      {
         trace("TW::onHideMe");
         gotoAndStop(2);
         this.showFullSize(false);
         dispatchEvent(new Event(TOGGLE_FOLD));
      }
   }
}

