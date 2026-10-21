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
   import survarium.constants.ClanState;
   import survarium.constants.ClanTeamRole;
   import survarium.constants.MessageId;
   
   public class TournamentWindow extends UIComponent
   {
      
      private static var _isCommander:Boolean = false;
      
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
      
      public function fillFoundPlayers(players:Array) : void
      {
         this.searchComponent.fillFoundPlayers(players);
      }
      
      public function init() : void
      {
         _isAdmin = ClanWindow.getState() == ClanState.MANAGE;
         this.btnDisbandTeam.label = this._isAdmin ? LocalizationData.getName("btn_disband_clan_team") : LocalizationData.getName("btn_leave_clan_team");
         ExternalInterface.call("clan_team_window_opened");
         this.updateButtons();
      }
      
      private function createClanTeam(event:Event = null) : void
      {
         ExternalInterface.call("create_clan_team");
      }
      
      private function updateButtons() : void
      {
         var isLeader:Boolean = _isCommander;
         var buttonWidth:int;
         this.btnDisbandTeam.visible = isLeader && this._fullSize;
         this.btnAddPlayer.visible = isLeader && this._fullSize;
         this.btnLeaveJoin.visible = !isLeader && this._fullSize;
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
            buttonWidth = this.btnLeaveJoin.textField.textWidth + 15;
            this.btnLeaveJoin.width = buttonWidth;
            this.btnLeaveJoin.x = (this.background.width - buttonWidth) / 2;
         }
         else
         {
            this.btnLeaveJoin.x = 170;
            this.btnLeaveJoin.width = 140;
         }
      }
      
      public function setPlayersData(players:Array) : void
      {
         _hasCommander = players.length && players[0].status == ClanTeamRole.COMMANDER;
         _isCommander = this._hasCommander && players[0].name == PlayerInfo.nickname;
         trace("TW::setPlayersData",this._hasCommander,_isCommander);
         _inTeam = false;
         var i:uint = 0;
         var status:uint;
         while(i < players.length)
         {
            status = (players[i].status);
            players[i].movableUp = _isCommander && (status == ClanTeamRole.INVITES || status == ClanTeamRole.RESERVE);
            players[i].movableDown = _isCommander && status == ClanTeamRole.SOLDIER;
            players[i].favoritable = this._isAdmin;
            if(players[i].name == PlayerInfo.nickname)
            {
               _inTeam = true;
            }
            i++;
         }
         PlayerInfo.isInTeam = this._inTeam;
         this.addSeparators(players);
         this.playersList.dataProvider = new DataProvider(players);
         this.updateButtons();
      }
      
      protected function addSeparators(players:Array) : void
      {
         var lastStatus:int = 0;
         var i:uint;
         var separator:Object;
         if(players)
         {
            i = 0;
            while(i < players.length)
            {
               if(players[i])
               {
                  if(lastStatus != players[i].status)
                  {
                     separator = {
                        status_text:LocalizationData.getName(this._statusLabels[players[i].status]),
                        isSeparator:true,
                        status:ClanTeamRole.SEPARATOR
                     };
                     lastStatus = (players[i].status);
                     players.splice(i,0,separator);
                     i++;
                  }
               }
               i++;
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
      
      private function onBtnBecomeCommander(event:ButtonEvent) : void
      {
         this.createClanTeam();
      }
      
      private function onBtnLeaveJoin(event:ButtonEvent) : void
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
      
      private function onPlayerListClicked(event:ButtonEvent) : void
      {
         var player:Object;
         trace("onPlayerListClicked::",event.target,event.currentTarget,event.target.name);
         if(event.target.name == "btnAddToFavorite" || event.target.name == "btnMovePlayerUp" || event.target.name == "btnMovePlayer")
         {
            player = event.target.parent.data;
            if(event.target.name == "btnAddToFavorite")
            {
               selectedName = player.name;
               this.moveToCommander();
            }
            else if(event.target.name == "btnMovePlayerUp" || event.target.name == "btnMovePlayer")
            {
               selectedName = player.name;
               if(player.status == ClanTeamRole.SOLDIER || player.status == ClanTeamRole.INVITES)
               {
                  this.moveToReserve();
               }
               else if(player.status == ClanTeamRole.RESERVE)
               {
                  this.moveToTeam();
               }
            }
         }
      }
      
      public function disbandTeam(event:ButtonEvent = null) : void
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
      
      private function onMouseItemClick(event:MouseEventEx) : void
      {
         var renderer:*;
         if(event.target && event.target.parent is SrvClanTeamListRenderer)
         {
            renderer = event.target.parent;
         }
         else if(event.target && event.target.parent.parent is SrvClanTeamListRenderer)
         {
            renderer = event.target.parent.parent;
         }
         else
         {
            renderer = event.target;
         }
         trace("onMouseItemClick:::",event.target,event.currentTarget,renderer);
         if(event.buttonIdx == MouseEventEx.LEFT_BUTTON || this.playersList.enabled == false)
         {
            return;
         }
         this.showContextMenu(renderer);
      }
      
      public function showContextMenu(renderer:*) : void
      {
         if(!renderer || renderer.enabled == false || !renderer.data)
         {
            return;
         }
         var player:Object = renderer.data;
         var isClanAdmin:Boolean = ClanWindow.getState() == ClanState.MANAGE;
         var isLeader:Boolean = _isCommander;
         var menuItems:Array = new Array();
         trace("isCommander::",isLeader,isClanAdmin);
         if(player.status == ClanTeamRole.COMMANDER)
         {
            menuItems.push({
               name:LocalizationData.getName("oust_commander"),
               click_function:this.oustCommander
            });
         }
         if(player.status == ClanTeamRole.SOLDIER && isLeader && this._inTeam)
         {
            menuItems.push({
               name:LocalizationData.getName("move_to_reserve"),
               click_function:this.moveToReserve
            });
            menuItems.push({
               name:LocalizationData.getName("remove_from_team"),
               click_function:this.removeFromTeam
            });
         }
         if(player.status == ClanTeamRole.RESERVE && isLeader && this._inTeam)
         {
            menuItems.push({
               name:LocalizationData.getName("move_to_team"),
               click_function:this.moveToTeam
            });
            menuItems.push({
               name:LocalizationData.getName("remove_from_team"),
               click_function:this.removeFromTeam
            });
         }
         if(player.status == ClanTeamRole.INVITES && isLeader && this._inTeam)
         {
            menuItems.push({
               name:LocalizationData.getName("move_to_reserve"),
               click_function:this.moveToReserve
            });
         }
         trace("showContextMenu::",player.status,"options",menuItems.length);
         if(menuItems.length)
         {
            SrvContextMenu.show(menuItems,0,0,this);
         }
         selectedName = player.name;
      }
      
      private function moveToTeam() : void
      {
         trace("change_status_clan_team_member",this.selectedName,ClanTeamRole.SOLDIER);
         ExternalInterface.call("change_status_clan_team_member",this.selectedName,ClanTeamRole.SOLDIER);
      }
      
      private function removeFromTeam() : void
      {
         trace("remvoe_team_member",this.selectedName);
         ExternalInterface.call("remove_team_member",this.selectedName);
      }
      
      private function moveToReserve() : void
      {
         trace("change_status_clan_team_member",this.selectedName,ClanTeamRole.RESERVE);
         ExternalInterface.call("change_status_clan_team_member",this.selectedName,ClanTeamRole.RESERVE);
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
      
      private function onTextInputFocusOut(event:FocusEvent) : void
      {
         trace("onTextInputFocusOut",FocusManager.getFocus(),stage.focus,stage.focus ? this.searchComponent.contains(stage.focus) : null);
         stage.addEventListener(Event.RENDER,this.onRender);
      }
      
      private function onRender(event:Event) : void
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
      
      private function invitePlayerByName(event:ListEvent) : void
      {
         trace("changeInputName::init");
         if(!event || (SrvContextMenu.getInstance()) && (SrvContextMenu.getInstance().parent))
         {
            return;
         }
         var selectedItem:* = event.itemData;
         trace("changeInputName::",selectedItem);
         if(selectedItem)
         {
            this.searchComponent.cleanUp();
            this.searchComponent.visible = false;
            this.searchComponent.x = 0;
            stage.focus = this;
            trace("invite_player_to_clan_team",selectedItem.name);
            ExternalInterface.call("invite_player_to_clan_team",selectedItem.name);
         }
      }
      
      private function inviteMember(event:ButtonEvent) : void
      {
         this.searchComponent.visible = true;
         stage.focus = this.searchComponent;
         var members:Array = ClanWindow.getMembersList();
         this.searchComponent.setFriendsList(members);
      }
      
      private function showFullSize(isFull:Boolean) : void
      {
         _fullSize = isFull;
         this.btnClose.visible = this.btnHide.visible = this.background.visible = this.btnAddPlayer.visible = this.btnDisbandTeam.visible = this.playersList.visible = isFull;
         this.btnBecomeCommander.visible = this.btnLeaveJoin.visible = isFull;
         if(isFull)
         {
            this.updateButtons();
         }
         this.btnShow.visible = this.smallBackground.visible = !isFull;
      }
      
      private function onShowMe(event:ButtonEvent) : void
      {
         trace("TW::onShowMe");
         ExternalInterface.call("clan_team_window_opened");
         gotoAndStop(1);
         this.showFullSize(true);
         dispatchEvent(new Event(TOGGLE_FOLD));
      }
      
      private function onCloseMe(event:ButtonEvent) : void
      {
         this.showCloseMessage();
      }
      
      protected function showCloseMessage() : void
      {
         var messageData:Object = {
            text:LocalizationData.getName(_isCommander ? "clan_window_close_message_commander" : "clan_window_close_message"),
            onYes:this.closeWindow,
            label_ok:LocalizationData.getName("ok"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",messageData,true,MessageId.AUTO,true);
      }
      
      protected function closeWindow() : void
      {
         trace("TW::closeWindow");
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      private function onHideMe(event:ButtonEvent) : void
      {
         trace("TW::onHideMe");
         gotoAndStop(2);
         this.showFullSize(false);
         dispatchEvent(new Event(TOGGLE_FOLD));
      }
   }
}

