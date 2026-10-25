package survarium.friends
{
   import com.greensock.TweenLite;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.KeyboardEvent;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.ui.Keyboard;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.ScrollBar;
   import scaleform.clik.controls.ScrollingListSmoothDynam;
   import scaleform.clik.controls.TextInput;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.ui.InputDetails;
   import survarium.constants.Events;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IFriendsList;
   import survarium.interfaces.ISubSystem;
   import survarium.constants.FriendStatus;
   
   public class FriendsMenu extends UIComponent implements IFriendsList, ISubSystem
   {
      
      protected static var _statusMap:Array = ["friend_state_lobby","friend_state_game","friend_state_offline","friend_state_away","friend_invite"];
      
      protected static var _friends:Array;
      
      protected static var _ignored:Array;
      
      protected static var _invites:Array;
      
      protected var _onFriends:Boolean = true;
      
      protected var _res:Array = new Array();
      
      protected var _online:uint = 0;
      
      public var labelTextField:TextField;
      
      public var playersList:ScrollingListSmoothDynam;
      
      public var friendsTab:ButtonBar;
      
      public var btn_friend:Button;
      
      public var btnFilter:Button;
      
      public var btnIgnore:Button;
      
      public var btn_close:Button;
      
      public var btnCancelSearch:Button;
      
      public var searchIcon:MovieClip;
      
      public var loadingIndicator:MovieClip;
      
      public var hintContainer:MovieClip;
      
      public var text_inpput:TextInput;
      
      protected var _separatorData:Object = {
         text:"online",
         isSeparator:true
      };
      
      protected var _inFocus:Boolean = false;
      
      private var _subsystemName:String;
      
      protected var _showOfflinePlayers:Boolean = true;
      
      protected var _showPlayerGroups:Boolean = true;
      
      protected var _useSearchAsFilter:Boolean = true;
      
      protected var _runtimeSearch:Boolean = true;
      
      protected var hintDelayed:Boolean = false;
      
      public function FriendsMenu()
      {
         super();
         this.btnCancelSearch.addEventListener(ButtonEvent.CLICK,this.cancelSearch);
         this.btnCancelSearch.enabled = false;
         this.btnCancelSearch.visible = false;
         var format:TextFormat = this.text_inpput.getTextFormat();
         this.text_inpput.textField.defaultTextFormat = format;
         this.hintContainer.mouseEnabled = false;
         this.hintContainer.mouseChildren = false;
         this.loadingIndicator.mouseEnabled = false;
         this.loadingIndicator.mouseChildren = false;
         this.hintContainer.visible = false;
         this.searchIcon.addEventListener(MouseEvent.CLICK,this.requestPlayersList);
         LocalizationData.addCallback(this.onLocalization,"enter_search_player_name");
         this.loadingIndicator.gotoAndStop(1);
         this.text_inpput.textField.addEventListener(FocusEvent.FOCUS_IN,this.onStartEnterFr);
         this.text_inpput.textField.addEventListener(FocusEvent.FOCUS_OUT,this.onLostEnterFr);
         this.text_inpput.textField.addEventListener(Event.CHANGE,this.searchInputFr);
      }
      
      public static function getOnlineFriendsList() : Array
      {
         return filterOnlinePlayers(_friends);
      }
      
      public static function getFriendsList() : Array
      {
         return _friends;
      }
      
      public static function getStatus(status:int) : String
      {
         if(_statusMap[status])
         {
            return LocalizationData.getName(_statusMap[status]).toUpperCase();
         }
         return "";
      }
      
      public static function isIgnored(playerName:String) : Boolean
      {
         var i:int = 0;
         while(i < _ignored.length)
         {
            if(playerName == _ignored[i].name)
            {
               return true;
            }
            i++;
         }
         return false;
      }
      
      public static function isFriend(playerName:String) : Boolean
      {
         var i:int = 0;
         while(i < _friends.length)
         {
            if(playerName == _friends[i].name)
            {
               return true;
            }
            i++;
         }
         return false;
      }
      
      public static function isFriendOnline(playerName:String) : Boolean
      {
         var i:int = 0;
         while(i < _friends.length)
         {
            if(playerName == _friends[i].name)
            {
               if(_friends[i].status != FriendStatus.OFFLINE)
               {
                  return true;
               }
               return false;
            }
            i++;
         }
         return false;
      }
      
      protected static function filterOnlinePlayers(players:Array) : Array
      {
         var online:Array = [];
         var i:uint;
         if(players && players.length)
         {
            i = 0;
            while(i < players.length)
            {
               if(players[i].status != FriendStatus.OFFLINE)
               {
                  online.push(players[i]);
               }
               i++;
            }
         }
         return online;
      }
      
      override public function set enabled(value:Boolean) : void
      {
         if(value)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_ENABLED));
         }
         else
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_DISABLED));
         }
         super.enabled = value;
      }
      
      override public function set visible(value:Boolean) : void
      {
         if(!value)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_DEACTIVATED));
         }
         super.visible = value;
      }
      
      public function get subsystem_name() : String
      {
         return this._subsystemName;
      }
      
      public function set subsystem_name(value:String) : void
      {
         _subsystemName = value;
      }
      
      public function get subsystem_enable() : Boolean
      {
         return enabled;
      }
      
      public function set subsystem_enable(value:Boolean) : void
      {
         enabled = value;
      }
      
      public function get subsystem_active() : Boolean
      {
         return visible && parent;
      }
      
      public function set subsystem_active(value:Boolean) : void
      {
         dispatchEvent(new Event(Events.SUBSYSTEM_ACTIVATED));
      }
      
      override public function set x(value:Number) : void
      {
         super.x = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(value:Number) : void
      {
         super.y = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      public function getOnlineFriendsList() : Array
      {
         return FriendsMenu.filterOnlinePlayers(_friends);
      }
      
      public function getFriendsList() : Array
      {
         return FriendsMenu.getFriendsList();
      }
      
      protected function cancelSearch(event:Event) : void
      {
         this.showSearch(true);
         this.text_inpput.text = "";
         this.text_inpput.validateNow();
         stage.focus = this;
         this.onSelectBtn();
      }
      
      public function setInvites(invites:Array) : void
      {
         _invites = invites;
         this.onSelectBtn(null);
      }
      
      public function getPlayersOnline() : uint
      {
         return this._online;
      }
      
      public function setPlayers(players:Array, isFriends:Boolean = true) : void
      {
         if(isFriends)
         {
            _friends = new Array();
         }
         else
         {
            _ignored = new Array();
         }
         _online = 0;
         var i:int = 0;
         while(i < players.length)
         {
            players[i].status_text = getStatus(players[i].status);
            if(isFriends && players[i].status != FriendStatus.OFFLINE)
            {
               ++this._online;
            }
            if(isFriends)
            {
               players[i].ignored = false;
               _friends.push(players[i]);
            }
            else
            {
               players[i].ignored = true;
               _ignored.push(players[i]);
            }
            i++;
         }
         this.onSelectBtn();
      }
      
      protected function repalceFoundPlayersByFriends(foundPlayers:Array) : void
      {
         if(!_friends || !foundPlayers)
         {
            return;
         }
         var friendsById:Object = new Object();
         var friendCount:uint = _friends.length;
         var i:uint = 0;
         while(i < friendCount)
         {
            friend = _friends[i];
            friendsById[friend.id] = friend;
            i++;
         }
         var foundCount:uint = foundPlayers.length;
         var j:uint = 0;
         var key:String;
         var friend:Object;
         var player:Object;
         while(j < foundCount)
         {
            player = foundPlayers[j];
            if(friendsById[player.id])
            {
               foundPlayers[j] = friendsById[player.id];
               break;
            }
            j++;
         }
         for(key in friendsById)
         {
            delete friendsById[key];
         }
         friendsById = null;
      }
      
      public function fillFoundPlayers(foundPlayers:Array) : void
      {
         this.loadingIndicator.gotoAndStop(1);
         this.repalceFoundPlayersByFriends(foundPlayers);
         this.playersList.dataProvider = new DataProvider(foundPlayers);
         this.playersList.selectedIndex = 0;
         invalidateData();
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         trace("FM::handleInput::",event.isDefaultPrevented());
         if(event.isDefaultPrevented())
         {
            return;
         }
         var details:InputDetails = event.details;
         var controllerIndex:uint = details.controllerIndex;
         trace("KEY_STATE::",details.value,details.navEquivalent);
         switch(details.code)
         {
            // Esc: LobbyMenu.close_active_window (Modals), the one Esc handler
            case Keyboard.ENTER:
               if(details.value == InputValue.KEY_DOWN)
               {
                  if(this.text_inpput.textField.text.length > 2)
                  {
                     this.hideHint();
                     this.requestPlayersList();
                  }
               }
               break;
         }
      }
      
      override protected function configUI() : void
      {
         this.btnFilter.toggle = true;
         this.btnFilter.selected = true;
         this.btnFilter.addEventListener(ButtonEvent.CLICK,this.toggleOnlineFilter);
         this.btnIgnore.toggle = true;
         this.btnIgnore.selected = false;
         this.btnIgnore.addEventListener(ButtonEvent.CLICK,this.toggleIgnoreFilter);
         this.playersList.margin = 0;
         this.playersList.rowStep = 2;
         this.playersList.rowHeight = 27;
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         var scrollBarClass:Class = getDefinitionByName("SrvScrollBarSkinned") as Class;
         var listScrollBar:ScrollBar = new scrollBarClass();
         listScrollBar.height = this.playersList.height;
         listScrollBar.y = this.playersList.y;
         listScrollBar.x = this.playersList.x + this.playersList.width - listScrollBar.width;
         addChild(listScrollBar);
         this.playersList.scrollBar = listScrollBar;
         this.findPlayer();
         this.btn_close.addEventListener(ButtonEvent.CLICK,this.closeMe);
         addEventListener(FocusEvent.FOCUS_IN,this.onFocusChange);
         addEventListener(FocusEvent.FOCUS_OUT,this.onFocusChange);
      }
      
      private function toggleOnlineFilter(event:ButtonEvent) : void
      {
         _showOfflinePlayers = this.btnFilter.selected;
         this.onSelectBtn();
      }
      
      private function toggleIgnoreFilter(event:ButtonEvent) : void
      {
         this.onSelectBtn();
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.DATA))
         {
            this.onNewIndex();
         }
      }
      
      protected function onNewIndex(event:ListEvent = null) : void
      {
      }
      
      protected function filterFriends(event:Event) : void
      {
         this.onSelectBtn();
      }
      
      protected function closeMe(event:Event) : void
      {
         visible = false;
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      protected function closeComponent(event:Event) : void
      {
         this.onLostEnter(null);
         MainScene.activeComponent = null;
      }
      
      protected function onFocusChange(event:FocusEvent) : void
      {
         _inFocus = event.type == FocusEvent.FOCUS_IN;
         addEventListener(Event.RENDER,this.afterFocusChange);
      }
      
      protected function afterFocusChange(event:Event) : void
      {
         removeEventListener(Event.RENDER,this.afterFocusChange);
      }
      
      protected function onLocalization(localizedText:String) : void
      {
         this.text_inpput.defaultText = LocalizationData.getName("players_list_filter_text");
         this.btnFilter.tooltipRendererName = "SrvTextHintRenderer";
         this.btnIgnore.tooltipRendererName = "SrvTextHintRenderer";
         this.btnFilter.data = {name:LocalizationData.getName("checkbox_show_offline_players")};
         this.btnIgnore.data = {name:LocalizationData.getName("ignored")};
         this.hintContainer.textField.text = LocalizationData.getName("friend_search_hint");
      }
      
      protected function onSelectBtn(event:Event = null) : void
      {
         trace("FL_ON_SELECT_BUTTON::");
         if(this.btnIgnore.selected == false)
         {
            this.gotoFriends();
         }
         else
         {
            this.gotoIgnored();
         }
         this.onNewIndex();
      }
      
      protected function keyDownHandler(event:KeyboardEvent) : void
      {
         if(event.keyCode == Keyboard.ENTER && this.text_inpput.textField.text.length > 2)
         {
            this.hideHint();
            this.requestPlayersList();
         }
      }
      
      protected function filterPlayers(players:Array, filterText:String) : Array
      {
         var matches:Array = [];
         var i:uint;
         var playerName:String;
         if(filterText == this.text_inpput.defaultText || filterText == "")
         {
            return players;
         }
         filterText = filterText.toLowerCase();
         if(players && players.length)
         {
            i = 0;
            while(i < players.length)
            {
               playerName = (players[i].name as String).toLowerCase();
               if(playerName.indexOf(filterText) != -1)
               {
                  matches.push(players[i]);
               }
               i++;
            }
         }
         return matches;
      }
      
      protected function hideHint() : void
      {
         TweenLite.killTweensOf(this.hintContainer);
         if(this.hintContainer.alpha != 0)
         {
            TweenLite.to(this.hintContainer,0.3,{alpha:0});
         }
         hintDelayed = false;
      }
      
      protected function showHint() : void
      {
         hintDelayed = true;
         TweenLite.to(this.hintContainer,0.3,{
            delay:1.5,
            alpha:1
         });
      }
      
      protected function searchInputFr(event:Event = null) : void
      {
         var players:Array;
         if(this._runtimeSearch)
         {
            this.hintContainer.visible = true;
            this.hintContainer.alpha = 0;
            if(this.hintContainer.alpha == 0 && !this.hintDelayed)
            {
               trace("startHintAlpha::",this.hintContainer.visible,this.hintDelayed);
               this.showHint();
            }
            else
            {
               this.hideHint();
               this.showHint();
            }
            if(this.text_inpput.textField.text.length > 0)
            {
               if(this._useSearchAsFilter)
               {
                  players = true ? _friends : _ignored;
                  if(!this._showOfflinePlayers)
                  {
                     players = filterOnlinePlayers(players);
                  }
                  this._res.length = 0;
                  if(true && _invites)
                  {
                     _res = this._res.concat(this.filterPlayers(_invites,this.text_inpput.textField.text));
                  }
                  _res = this._res.concat(this.filterPlayers(players,this.text_inpput.textField.text));
                  if(this._showPlayerGroups)
                  {
                     this.addSeparators(this._res);
                  }
                  this.playersList.dataProvider = new DataProvider(this._res);
               }
               else
               {
                  this.requestPlayersList();
               }
            }
            else
            {
               this.onSelectBtn();
            }
         }
      }
      
      protected function onLostEnterFr(event:FocusEvent) : void
      {
         if(this.text_inpput.textField.text == "")
         {
            this.text_inpput.textField.addEventListener(FocusEvent.FOCUS_IN,this.onStartEnterFr);
         }
         this.hideHint();
      }
      
      protected function onStartEnterFr(event:FocusEvent) : void
      {
         this.text_inpput.textField.removeEventListener(FocusEvent.FOCUS_IN,this.onStartEnterFr);
         this.text_inpput.text = "";
      }
      
      protected function searchInput(event:Event) : void
      {
      }
      
      protected function onLostEnter(event:FocusEvent) : void
      {
      }
      
      protected function onStartEnter(event:FocusEvent) : void
      {
      }
      
      protected function findButtonClick(event:ButtonEvent) : void
      {
      }
      
      protected function requestPlayersList(event:Event = null) : void
      {
         var searchText:String = this.text_inpput.textField.text;
         if(searchText.length < 3 || searchText == this.text_inpput.defaultText)
         {
            return;
         }
         dispatchEvent(new Event(Event.CONNECT,true));
         ExternalInterface.call("find_players",searchText);
         this.loadingIndicator.gotoAndPlay(1);
         this.showSearch(false);
      }
      
      protected function showSearch(show:Boolean) : void
      {
         this.searchIcon.visible = show;
         this.btnCancelSearch.enabled = !show;
         this.btnCancelSearch.visible = !show;
      }
      
      protected function findPlayer() : void
      {
      }
      
      protected function addSeparators(players:Array) : void
      {
         var lastStatus:int = -1;
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
                        status_text:players[i].status_text,
                        isSeparator:true,
                        status:FriendStatus.SEPARATOR
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
      
      protected function gotoFriends(event:ButtonEvent = null) : void
      {
         _onFriends = true;
         var rows:Array = new Array();
         if(true && _invites && _invites.length)
         {
            rows = _invites.concat();
         }
         var friends:Array = this._showOfflinePlayers ? _friends : filterOnlinePlayers(_friends);
         if(friends)
         {
            friends.sortOn(["status","name"]);
         }
         rows = rows.concat(this.filterPlayers(friends,this.text_inpput.textField.text));
         if(this._showPlayerGroups)
         {
            this.addSeparators(rows);
         }
         var scrollPosition:int = this.playersList.scrollBar ? (this.playersList.scrollBar.position) : 0;
         trace("SHOW_FRIENDS:",rows.length);
         this.playersList.dataProvider = new DataProvider(rows);
         this.playersList.validateNow();
         if(this.playersList.scrollBar)
         {
            this.playersList.scrollBar.position = scrollPosition;
         }
      }
      
      protected function gotoIgnored(event:ButtonEvent = null) : void
      {
         _onFriends = false;
         var rows:Array = new Array();
         rows = this.filterPlayers(_ignored,this.text_inpput.textField.text);
         this.playersList.dataProvider = new DataProvider(rows);
      }
   }
}

