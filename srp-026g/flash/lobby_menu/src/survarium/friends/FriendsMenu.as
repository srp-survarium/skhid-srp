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
         var _loc1_:TextFormat = this.text_inpput.getTextFormat();
         this.text_inpput.textField.defaultTextFormat = _loc1_;
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
      
      public static function getStatus(param1:int) : String
      {
         if(_statusMap[param1])
         {
            return LocalizationData.getName(_statusMap[param1]).toUpperCase();
         }
         return "";
      }
      
      public static function isIgnored(param1:String) : Boolean
      {
         var _loc2_:int = 0;
         while(_loc2_ < _ignored.length)
         {
            if(param1 == _ignored[_loc2_].name)
            {
               return true;
            }
            _loc2_++;
         }
         return false;
      }
      
      public static function isFriend(param1:String) : Boolean
      {
         var _loc2_:int = 0;
         while(_loc2_ < _friends.length)
         {
            if(param1 == _friends[_loc2_].name)
            {
               return true;
            }
            _loc2_++;
         }
         return false;
      }
      
      public static function isFriendOnline(param1:String) : Boolean
      {
         var _loc2_:int = 0;
         while(_loc2_ < _friends.length)
         {
            if(param1 == _friends[_loc2_].name)
            {
               if(_friends[_loc2_].status != 2)
               {
                  return true;
               }
               return false;
            }
            _loc2_++;
         }
         return false;
      }
      
      protected static function filterOnlinePlayers(param1:Array) : Array
      {
         var _loc2_:Array = [];
         var _loc3_:uint;
         if(param1 && param1.length)
         {
            _loc3_ = 0;
            while(_loc3_ < param1.length)
            {
               if(param1[_loc3_].status != 2)
               {
                  _loc2_.push(param1[_loc3_]);
               }
               _loc3_++;
            }
         }
         return _loc2_;
      }
      
      override public function set enabled(param1:Boolean) : void
      {
         if(param1)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_ENABLED));
         }
         else
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_DISABLED));
         }
         super.enabled = param1;
      }
      
      override public function set visible(param1:Boolean) : void
      {
         if(!param1)
         {
            dispatchEvent(new Event(Events.SUBSYSTEM_DEACTIVATED));
         }
         super.visible = param1;
      }
      
      public function get subsystem_name() : String
      {
         return this._subsystemName;
      }
      
      public function set subsystem_name(param1:String) : void
      {
         _subsystemName = param1;
      }
      
      public function get subsystem_enable() : Boolean
      {
         return enabled;
      }
      
      public function set subsystem_enable(param1:Boolean) : void
      {
         enabled = param1;
      }
      
      public function get subsystem_active() : Boolean
      {
         return visible && parent;
      }
      
      public function set subsystem_active(param1:Boolean) : void
      {
         dispatchEvent(new Event(Events.SUBSYSTEM_ACTIVATED));
      }
      
      override public function set x(param1:Number) : void
      {
         super.x = int(param1 * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(param1:Number) : void
      {
         super.y = int(param1 * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      public function getOnlineFriendsList() : Array
      {
         return FriendsMenu.filterOnlinePlayers(_friends);
      }
      
      public function getFriendsList() : Array
      {
         return FriendsMenu.getFriendsList();
      }
      
      protected function cancelSearch(param1:Event) : void
      {
         this.showSearch(true);
         this.text_inpput.text = "";
         this.text_inpput.validateNow();
         stage.focus = this;
         this.onSelectBtn();
      }
      
      public function setInvites(param1:Array) : void
      {
         _invites = param1;
         this.onSelectBtn(null);
      }
      
      public function getPlayersOnline() : uint
      {
         return this._online;
      }
      
      public function setPlayers(param1:Array, param2:Boolean = true) : void
      {
         if(param2)
         {
            _friends = new Array();
         }
         else
         {
            _ignored = new Array();
         }
         _online = 0;
         var _loc3_:int = 0;
         while(_loc3_ < param1.length)
         {
            param1[_loc3_].status_text = getStatus(param1[_loc3_].status);
            if(param2 && param1[_loc3_].status != 2)
            {
               ++this._online;
            }
            if(param2)
            {
               param1[_loc3_].ignored = false;
               _friends.push(param1[_loc3_]);
            }
            else
            {
               param1[_loc3_].ignored = true;
               _ignored.push(param1[_loc3_]);
            }
            _loc3_++;
         }
         this.onSelectBtn();
      }
      
      protected function repalceFoundPlayersByFriends(param1:Array) : void
      {
         if(!_friends || !param1)
         {
            return;
         }
         var _loc2_:Object = new Object();
         var _loc3_:uint = _friends.length;
         var _loc4_:uint = 0;
         while(_loc4_ < _loc3_)
         {
            _loc8_ = _friends[_loc4_];
            _loc2_[_loc8_.id] = _loc8_;
            _loc4_++;
         }
         var _loc5_:uint = param1.length;
         var _loc6_:uint = 0;
         var _loc7_:String;
         var _loc8_:Object;
         var _loc9_:Object;
         while(_loc6_ < _loc5_)
         {
            _loc9_ = param1[_loc6_];
            if(_loc2_[_loc9_.id])
            {
               param1[_loc6_] = _loc2_[_loc9_.id];
               break;
            }
            _loc6_++;
         }
         for(_loc7_ in _loc2_)
         {
            delete _loc2_[_loc7_];
         }
         _loc2_ = null;
      }
      
      public function fillFoundPlayers(param1:Array) : void
      {
         this.loadingIndicator.gotoAndStop(1);
         this.repalceFoundPlayersByFriends(param1);
         this.playersList.dataProvider = new DataProvider(param1);
         this.playersList.selectedIndex = 0;
         invalidateData();
      }
      
      override public function handleInput(param1:InputEvent) : void
      {
         trace("FM::handleInput::",param1.isDefaultPrevented());
         if(param1.isDefaultPrevented())
         {
            return;
         }
         var _loc2_:InputDetails = param1.details;
         var _loc3_:uint = _loc2_.controllerIndex;
         trace("KEY_STATE::",_loc2_.value,_loc2_.navEquivalent);
         switch(_loc2_.code)
         {
            case Keyboard.ESCAPE:
               if(_loc2_.value == InputValue.KEY_DOWN || _loc2_.value == InputValue.KEY_UP)
               {
                  if(stage.focus == this.text_inpput.textField || stage.focus == this.text_inpput.textField)
                  {
                     stage.focus = this;
                     param1.preventDefault();
                  }
               }
               break;
            case Keyboard.ENTER:
               if(_loc2_.value == InputValue.KEY_DOWN)
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
         var _loc1_:Class = getDefinitionByName("SrvScrollBarSkinned") as Class;
         var _loc2_:ScrollBar = new _loc1_();
         _loc2_.height = this.playersList.height;
         _loc2_.y = this.playersList.y;
         _loc2_.x = this.playersList.x + this.playersList.width - _loc2_.width;
         addChild(_loc2_);
         this.playersList.scrollBar = _loc2_;
         this.findPlayer();
         this.btn_close.addEventListener(ButtonEvent.CLICK,this.closeMe);
         addEventListener(FocusEvent.FOCUS_IN,this.onFocusChange);
         addEventListener(FocusEvent.FOCUS_OUT,this.onFocusChange);
      }
      
      private function toggleOnlineFilter(param1:ButtonEvent) : void
      {
         _showOfflinePlayers = this.btnFilter.selected;
         this.onSelectBtn();
      }
      
      private function toggleIgnoreFilter(param1:ButtonEvent) : void
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
      
      protected function onNewIndex(param1:ListEvent = null) : void
      {
      }
      
      protected function filterFriends(param1:Event) : void
      {
         this.onSelectBtn();
      }
      
      protected function closeMe(param1:Event) : void
      {
         visible = false;
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      protected function closeComponent(param1:Event) : void
      {
         this.onLostEnter(null);
         MainScene.activeComponent = null;
      }
      
      protected function onFocusChange(param1:FocusEvent) : void
      {
         _inFocus = param1.type == FocusEvent.FOCUS_IN;
         addEventListener(Event.RENDER,this.afterFocusChange);
      }
      
      protected function afterFocusChange(param1:Event) : void
      {
         removeEventListener(Event.RENDER,this.afterFocusChange);
      }
      
      protected function onLocalization(param1:String) : void
      {
         this.text_inpput.defaultText = LocalizationData.getName("players_list_filter_text");
         this.btnFilter.tooltipRendererName = "SrvTextHintRenderer";
         this.btnIgnore.tooltipRendererName = "SrvTextHintRenderer";
         this.btnFilter.data = {name:LocalizationData.getName("checkbox_show_offline_players")};
         this.btnIgnore.data = {name:LocalizationData.getName("ignored")};
         this.hintContainer.textField.text = LocalizationData.getName("friend_search_hint");
      }
      
      protected function onSelectBtn(param1:Event = null) : void
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
      
      protected function keyDownHandler(param1:KeyboardEvent) : void
      {
         if(param1.keyCode == 13 && this.text_inpput.textField.text.length > 2)
         {
            this.hideHint();
            this.requestPlayersList();
         }
      }
      
      protected function filterPlayers(param1:Array, param2:String) : Array
      {
         var _loc3_:Array = [];
         var _loc4_:uint;
         var _loc5_:String;
         if(param2 == this.text_inpput.defaultText || param2 == "")
         {
            return param1;
         }
         param2 = param2.toLowerCase();
         if(param1 && param1.length)
         {
            _loc4_ = 0;
            while(_loc4_ < param1.length)
            {
               _loc5_ = (param1[_loc4_].name as String).toLowerCase();
               if(_loc5_.indexOf(param2) != -1)
               {
                  _loc3_.push(param1[_loc4_]);
               }
               _loc4_++;
            }
         }
         return _loc3_;
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
      
      protected function searchInputFr(param1:Event = null) : void
      {
         var _loc2_:Array;
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
                  _loc2_ = true ? _friends : _ignored;
                  if(!this._showOfflinePlayers)
                  {
                     _loc2_ = filterOnlinePlayers(_loc2_);
                  }
                  this._res.length = 0;
                  if(true && _invites)
                  {
                     _res = this._res.concat(this.filterPlayers(_invites,this.text_inpput.textField.text));
                  }
                  _res = this._res.concat(this.filterPlayers(_loc2_,this.text_inpput.textField.text));
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
      
      protected function onLostEnterFr(param1:FocusEvent) : void
      {
         if(this.text_inpput.textField.text == "")
         {
            this.text_inpput.textField.addEventListener(FocusEvent.FOCUS_IN,this.onStartEnterFr);
         }
         this.hideHint();
      }
      
      protected function onStartEnterFr(param1:FocusEvent) : void
      {
         this.text_inpput.textField.removeEventListener(FocusEvent.FOCUS_IN,this.onStartEnterFr);
         this.text_inpput.text = "";
      }
      
      protected function searchInput(param1:Event) : void
      {
      }
      
      protected function onLostEnter(param1:FocusEvent) : void
      {
      }
      
      protected function onStartEnter(param1:FocusEvent) : void
      {
      }
      
      protected function findButtonClick(param1:ButtonEvent) : void
      {
      }
      
      protected function requestPlayersList(param1:Event = null) : void
      {
         var _loc2_:String = this.text_inpput.textField.text;
         if(_loc2_.length < 3 || _loc2_ == this.text_inpput.defaultText)
         {
            return;
         }
         dispatchEvent(new Event(Event.CONNECT,true));
         ExternalInterface.call("find_players",_loc2_);
         this.loadingIndicator.gotoAndPlay(1);
         this.showSearch(false);
      }
      
      protected function showSearch(param1:Boolean) : void
      {
         this.searchIcon.visible = param1;
         this.btnCancelSearch.enabled = !param1;
         this.btnCancelSearch.visible = !param1;
      }
      
      protected function findPlayer() : void
      {
      }
      
      protected function addSeparators(param1:Array) : void
      {
         var _loc2_:int = -1;
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
                        status_text:param1[_loc3_].status_text,
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
      
      protected function gotoFriends(param1:ButtonEvent = null) : void
      {
         _onFriends = true;
         var _loc2_:Array = new Array();
         if(true && _invites && _invites.length)
         {
            _loc2_ = _invites.concat();
         }
         var _loc3_:Array = this._showOfflinePlayers ? _friends : filterOnlinePlayers(_friends);
         if(_loc3_)
         {
            _loc3_.sortOn(["status","name"]);
         }
         _loc2_ = _loc2_.concat(this.filterPlayers(_loc3_,this.text_inpput.textField.text));
         if(this._showPlayerGroups)
         {
            this.addSeparators(_loc2_);
         }
         var _loc4_:int = this.playersList.scrollBar ? (this.playersList.scrollBar.position) : 0;
         trace("SHOW_FRIENDS:",_loc2_.length);
         this.playersList.dataProvider = new DataProvider(_loc2_);
         this.playersList.validateNow();
         if(this.playersList.scrollBar)
         {
            this.playersList.scrollBar.position = _loc4_;
         }
      }
      
      protected function gotoIgnored(param1:ButtonEvent = null) : void
      {
         _onFriends = false;
         var _loc2_:Array = new Array();
         _loc2_ = this.filterPlayers(_ignored,this.text_inpput.textField.text);
         this.playersList.dataProvider = new DataProvider(_loc2_);
      }
   }
}

