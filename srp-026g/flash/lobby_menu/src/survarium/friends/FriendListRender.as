package survarium.friends
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.utils.getTimer;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.gfx.MouseEventEx;
   import survarium.clan.ClanWindow;
   import survarium.controls.FixedTextDots;
   import survarium.controls.ListItemBase;
   import survarium.controls.PlayerInfo;
   import survarium.controls.SrvContextMenu;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsWindow;
   import survarium.player.LocalPlayerProfile;
   import survarium.constants.ClaimReason;
   import survarium.constants.CommentKind;
   import survarium.constants.FriendStatus;
   import survarium.constants.ImportantMessageAnswer;

   public class FriendListRender extends ListItemBase
   {
      
      public var status_tx:TextField;
      
      public var titul_tx:TextField;
      
      public var name_tx:FixedTextDots;
      
      public var menuButton:MovieClip;
      
      public var btn_ignore:MovieClip;
      
      public var btn_acccept:Button;
      
      public var btn_cancel:Button;
      
      public var background:MovieClip;
      
      public var status_indicator:MovieClip;
      
      public var separator:MovieClip;

      // The menu shown for a friend, waiting for the lobby to say whether
      // the friend is in a room (askFriendRoom): its items, the friend's
      // account and when it was asked; then the room its "Join room" joins.
      private var _roomItems:Array;

      private var _roomFriend:Number = 0;

      private var _roomAskedAt:int = 0;

      private var _friendRoom:Object;

      public function FriendListRender()
      {
         super();
      }
      
      public function removeFriend() : void
      {
         ExternalInterface.call("remove_friend",data.id);
      }
      
      override public function set selected(value:Boolean) : void
      {
      }
      
      public function set highlighted(value:Boolean) : void
      {
         super.selected = value;
      }
      
      public function handleCrossClick(event:Event = null) : void
      {
         if(data.ignored)
         {
            this.removeIgnored();
         }
         else
         {
            this.removeFriend();
         }
      }
      
      public function addToFriends() : void
      {
         ExternalInterface.call("add_friend",data.id);
      }
      
      public function removeIgnored() : void
      {
         ExternalInterface.call("remove_ignored",data.id);
      }
      
      public function addToIgnore() : void
      {
         ExternalInterface.call("add_ignore",data.id);
      }
      
      public function askToJoin() : void
      {
         ExternalInterface.call("ask_join_game",data.id);
      }
      
      public function askToJoinSquad() : void
      {
         ExternalInterface.call("invite_to_squad",data.name);
      }
      
      public function sendMessage() : void
      {
         var playerName:String = data.name;
         ExternalInterface.call("start_friend_message",playerName);
         dispatchEvent(new Event(Event.CONNECT,true));
      }
      
      override public function get height() : Number
      {
         if(!_data || _data.request == undefined)
         {
            return 27;
         }
         return 54;
      }
      
      override protected function setState(newState:String) : void
      {
         if(_data && _data.isSeparator)
         {
            return;
         }
         super.setState(newState);
      }
      
      override protected function updateText() : void
      {
         if(!data)
         {
            visible = false;
            return;
         }
         visible = true;
         var separatorRow:Boolean = _data.isSeparator;
         this.separator.visible = separatorRow;
         this.status_indicator.visible = this.status_tx.visible = this.name_tx.visible = !this.separator.visible;
         if(separatorRow)
         {
            this.separator.textField.text = _data.status_text;
            this.menuButton.visible = false;
         }
         else
         {
            this.menuButton.visible = true;
         }
         this.name_tx.text = data.abbr ? "[" + data.abbr + "] " + data.name : data.name;
         this.name_tx.validateNow();
         this.menuButton.x = this.name_tx.textField.textWidth + this.name_tx.x + 7;
         var isRequest:Boolean = data.request != undefined;
         var canAnswer:Boolean;
         if(this.background)
         {
            this.background.height = isRequest ? 54 : 27;
         }
         this.btn_acccept.y = this.btn_cancel.y = isRequest ? 27 : 0;
         if(isRequest)
         {
            this.status_indicator.gotoAndStop("invite");
         }
         else if(_data.ignored)
         {
            this.status_indicator.gotoAndStop("offline");
         }
         else if(_data.status == FriendStatus.LOBBY)
         {
            this.status_indicator.gotoAndStop("online");
         }
         else if(_data.status == FriendStatus.IN_GAME)
         {
            this.status_indicator.gotoAndStop("ingame");
         }
         else
         {
            this.status_indicator.gotoAndStop("offline");
         }
         this.status_tx.text = "";
         this.status_tx.visible = false;
         if(isRequest && enabled)
         {
            this.btn_acccept.visible = true;
            this.btn_cancel.visible = true;
            canAnswer = false;
            if(_data.sended != true)
            {
               canAnswer = true;
            }
            if(this.btn_acccept.enabled != canAnswer)
            {
               this.btn_acccept.enabled = canAnswer;
               this.btn_acccept.validateNow();
            }
            if(this.btn_cancel.enabled != canAnswer)
            {
               this.btn_cancel.enabled = canAnswer;
               this.btn_cancel.validateNow();
            }
         }
         else
         {
            this.btn_acccept.visible = false;
            this.btn_cancel.visible = false;
            this.btn_acccept.enabled = false;
            this.btn_cancel.enabled = false;
         }
         this.name_tx.buttonMode = enabled;
         selectable = false;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         addEventListener(MouseEvent.CLICK,this.onClick);
         if(this.btn_ignore)
         {
            this.btn_ignore.addEventListener(ButtonEvent.CLICK,this.handleCrossClick);
         }
         this.name_tx.mouseEnabled = false;
         this.name_tx.mouseChildren = false;
         this.name_tx.textField.mouseEnabled = false;
         this.btn_acccept.addEventListener(ButtonEvent.CLICK,this.handleButtonClick);
         this.btn_cancel.addEventListener(ButtonEvent.CLICK,this.handleButtonClick);
         this.menuButton.addEventListener(ButtonEvent.CLICK,this.showContextMenu);
         LocalizationData.addButton(this.btn_acccept,"add");
         LocalizationData.addButton(this.btn_cancel,"cancel");
         selectable = false;
      }
      
      protected function showContextMenu(event:Event = null) : void
      {
         var menuItems:Array;
         var playerName:String;
         var alreadyIgnored:Boolean;
         var inFriendList:Boolean;
         var canMessage:Boolean;
         var canInvite:Boolean;
         var isStranger:Boolean;
         var alreadyFriend:Boolean;
         var claimOptions:Array;
         if(!data.ignored)
         {
            if(_data.status == FriendStatus.SEPARATOR)
            {
               return;
            }
            menuItems = [];
            playerName = data.name;
            alreadyIgnored = FriendsMenu.isIgnored(playerName);
            inFriendList = _data.status == FriendStatus.LOBBY || _data.status == FriendStatus.IN_GAME || _data.status == FriendStatus.OFFLINE || _data.status == FriendStatus.AWAY;
            canMessage = _data.status == FriendStatus.LOBBY || _data.status == FriendStatus.IN_GAME || _data.status == FriendStatus.INVITE;
            canInvite = _data.status == FriendStatus.LOBBY || _data.status == FriendStatus.IN_GAME || _data.status == FriendStatus.AWAY;
            isStranger = _data.status == undefined;
            if(isStranger)
            {
               alreadyFriend = FriendsMenu.isFriend(playerName);
               if(!alreadyFriend)
               {
                  menuItems.push({
                     name:LocalizationData.getName("add_to_friends"),
                     click_function:this.addToFriends
                  });
               }
               if(!alreadyIgnored)
               {
                  menuItems.push({
                     name:LocalizationData.getName("add_to_ignore"),
                     click_function:this.addToIgnore
                  });
               }
            }
            if(canMessage)
            {
               menuItems.push({
                  name:LocalizationData.getName("send_message"),
                  click_function:this.sendMessage
               });
            }
            if(inFriendList)
            {
               menuItems.push({
                  name:LocalizationData.getName("remove_from_friends"),
                  click_function:this.removeFriend
               });
               if(!alreadyIgnored)
               {
                  menuItems.push({
                     name:LocalizationData.getName("add_to_ignore"),
                     click_function:this.addToIgnore
                  });
               }
               claimOptions = [];
               claimOptions.push({
                  name:LocalizationData.getName("bot_or_cheater_claim"),
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:CommentKind.CLAIM,
                  subIndex:ClaimReason.BOT_OR_CHEATER
               });
               claimOptions.push({
                  name:LocalizationData.getName("swearing_claim"),
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:CommentKind.CLAIM,
                  subIndex:ClaimReason.SWEARING
               });
               claimOptions.push({
                  name:LocalizationData.getName("non_sport_behavior_claim"),
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:CommentKind.CLAIM,
                  subIndex:ClaimReason.NON_SPORT_BEHAVIOR
               });
               claimOptions.push({
                  name:LocalizationData.getName("account_trading_claim"),
                  click_function:this.flagPlayer,
                  callBack:true,
                  index:CommentKind.CLAIM,
                  subIndex:ClaimReason.ACCOUNT_TRADING
               });
               menuItems.push({
                  name:LocalizationData.getName("claim_player"),
                  options:claimOptions
               });
               trace("showContextMenu::",_data.status,canInvite);
            }
            if(PlayerInfo.commander && !PlayerInfo.isSquadFull && (inFriendList && canInvite || isStranger) && !SquadList.isInSquad(playerName))
            {
               menuItems.push({
                  name:LocalizationData.getName("invite_to_squad"),
                  click_function:this.askToJoinSquad
               });
            }
            if((PlayerInfo.isClanManager || PlayerInfo.isClanIviter) && !ClanWindow.isInClan(playerName))
            {
               menuItems.push({
                  name:LocalizationData.getName("invite_to_clan"),
                  click_function:this.askToJoinClan
               });
            }
            highlighted = true;
            SrvContextMenu.show(menuItems,0,0,this);
            SrvContextMenu.getInstance().addEventListener(Event.REMOVED_FROM_STAGE,this.deselect);
            this.askFriendRoom(menuItems);
         }
         else
         {
            highlighted = true;
            SrvContextMenu.show([{
               name:LocalizationData.getName("remove_from_ignore"),
               click_function:this.removeIgnored
            }],0,0,this);
            SrvContextMenu.getInstance().addEventListener(Event.REMOVED_FROM_STAGE,this.deselect);
         }
      }
      
      private function askToJoinClan() : void
      {
         trace("askToJoinClan",data.name);
         ExternalInterface.call("invite_player_to_clan",data.name);
      }
      
      protected function flagPlayer(option:Object) : void
      {
         trace("comment_about_user",data.name,option.index,option.subIndex);
         ExternalInterface.call("comment_about_user",data.name,option.index,option.subIndex);
      }
      
      protected function handleButtonClick(event:ButtonEvent) : void
      {
         var answer:uint = event.target == this.btn_acccept ? ImportantMessageAnswer.ACCEPT : ImportantMessageAnswer.DECLINE;
         this.btn_acccept.enabled = false;
         this.btn_cancel.enabled = false;
         _data.sended = true;
         dispatchEvent(new Event(Event.CONNECT,true));
         var messageType:uint = (_data.type);
         if(data.id != LobbyMenu.TEST_IMPORTANT_MESSAGE_ID)
         {
            ExternalInterface.call("important_message_read",data.id,answer,messageType);
         }
      }
      
      protected function deselect(event:Event = null) : void
      {
         highlighted = false;
         SrvContextMenu.getInstance().removeEventListener(Event.REMOVED_FROM_STAGE,this.deselect);
      }
      
      protected function onClick(event:MouseEventEx) : void
      {
         if(!data)
         {
            return;
         }
         if(event.buttonIdx == MouseEventEx.RIGHT_BUTTON && _data.name)
         {
            this.showContextMenu();
         }
      }

      // "Join room" in a friend's menu while that friend is a member of a
      // room (the owner's of 2026-10-20: "it should also be possible to rmb
      // on friend and join his room, if he is in a room"). The menu opens as
      // before; the lobby is asked which friends are in a room (`S1;Y`,
      // lobby.md 2.16.3), and the item comes at the menu's end while it is
      // still open ("Queue for room" for a full room). Friends only, not a
      // search's players.
      protected function askFriendRoom(items:Array) : void
      {
         this._roomItems = null;
         if(RoomsWindow.me == null || !data || !FriendsMenu.isFriend(String(data.name)))
         {
            return;
         }
         this._roomItems = items;
         this._roomFriend = Number(data.id);
         this._roomAskedAt = getTimer();
         RoomsWindow.me.askFriendRooms(this.takeFriendRooms);
      }

      // The friends in a room: {account, key, players, size, pin} each. An
      // answer later than 5 s is for a menu long gone; a menu closed or
      // replaced meanwhile is not changed (SrvContextMenu.addItem).
      private function takeFriendRooms(rooms:Array) : void
      {
         var items:Array = this._roomItems;
         this._roomItems = null;
         if(items == null || getTimer() - this._roomAskedAt > 5000)
         {
            return;
         }
         for(var i:int = 0; i < rooms.length; i++)
         {
            if(Number(rooms[i].account) != this._roomFriend)
            {
               continue;
            }
            this._friendRoom = rooms[i];
            // A reason holds (kits that can not play, a maintenance): no Join
            // (the server refuses it; presets.md). A friend's room's max level
            // is not known here, so no loadout the popup could offer counts:
            // the kits must play now.
            if(!LocalPlayerProfile.mayPlay())
            {
               return;
            }
            var full:Boolean = rooms[i].players >= rooms[i].size * 2;
            SrvContextMenu.addItem(items,{
               "name":LocalizationData.getName(full ? "st_srp_rooms_queue_room" : "st_srp_rooms_join_room"),
               "click_function":this.joinFriendRoom
            });
            return;
         }
      }

      private function joinFriendRoom() : void
      {
         if(RoomsWindow.me != null && this._friendRoom != null)
         {
            RoomsWindow.me.joinFriend(this._friendRoom);
         }
      }
   }
}

