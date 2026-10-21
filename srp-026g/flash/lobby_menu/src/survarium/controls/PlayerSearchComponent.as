package survarium.controls
{
   import com.greensock.TweenLite;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.ui.Keyboard;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ScrollingListSmooth;
   import scaleform.clik.controls.TextInput;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.FocusHandlerEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   import survarium.data.LocalizationData;
   import survarium.constants.FriendStatus;
   
   public class PlayerSearchComponent extends UIComponent
   {
      
      public var textInput:TextInput;
      
      public var btnClose:Button;
      
      public var btnCancelSearch:Button;
      
      public var btnFilter:Button;
      
      public var playersList:ScrollingListSmooth;
      
      public var hintContainer:MovieClip;
      
      public var loadingIndicator:MovieClip;
      
      public var searchIcon:MovieClip;
      
      private var _friendsList:Array;
      
      private var hintDelayed:Boolean = false;
      
      private var _dropDown:Boolean = true;
      
      public function PlayerSearchComponent()
      {
         super();
      }
      
      public function setDropDown(value:Boolean) : void
      {
         _dropDown = value;
      }
      
      public function fillFoundPlayers(players:Array) : void
      {
         this.playersList.dataProvider = new DataProvider(players);
         this.playersList.selectedIndex = 0;
         this.loadingIndicator.gotoAndStop(1);
         this.btnCancelSearch.visible = true;
         this.unpdatePlayerListHeight();
      }
      
      public function setFriendsList(friends:Array) : void
      {
         _friendsList = friends;
         if(!this.btnCancelSearch.visible)
         {
            this.unpdatePlayerList();
         }
      }
      
      public function cleanUp(clearText:Boolean = true) : void
      {
         if(clearText)
         {
            this.textInput.text = "";
         }
         this.playersList.dataProvider = new DataProvider(null);
         this.playersList.visible = false;
         this.showSearch(true);
         stackTrace("cleanUp::");
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(this.btnFilter)
         {
            this.btnFilter.toggle = true;
            this.btnFilter.addEventListener(ButtonEvent.CLICK,this.toggleOnlineFilter);
         }
         this.playersList.visible = false;
         this.playersList.rowHeight = 27;
         this.textInput.textField.addEventListener(FocusEvent.FOCUS_IN,this.onStartEnter);
         this.textInput.textField.addEventListener(FocusEvent.FOCUS_OUT,this.onLostEnter);
         this.textInput.addEventListener(FocusHandlerEvent.FOCUS_IN,this.onInputFocus);
         this.textInput.addEventListener(FocusHandlerEvent.FOCUS_OUT,this.onInputFocus);
         this.textInput.textField.addEventListener(Event.CHANGE,this.searchInput);
         this.btnCancelSearch.addEventListener(ButtonEvent.CLICK,this.cancelSearch);
         this.loadingIndicator.gotoAndStop(1);
         this.searchIcon.addEventListener(MouseEvent.CLICK,this.onSearchClick);
         LocalizationData.addCallback(this.onLocalization,"players_list_filter_text");
         this.btnCancelSearch.visible = false;
         this.hintContainer.mouseEnabled = false;
         this.hintContainer.mouseChildren = false;
         this.loadingIndicator.mouseEnabled = false;
         this.loadingIndicator.mouseChildren = false;
         this.hintContainer.visible = false;
         this.showSearch(true);
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         this.playersList.focusTarget = this.textInput;
      }
      
      private function toggleOnlineFilter(event:ButtonEvent) : void
      {
         this.unpdatePlayerList();
      }
      
      private function onSearchClick(event:MouseEvent) : void
      {
         stage.focus = this;
         this.requestPlayersList();
      }
      
      private function onInputFocus(event:FocusHandlerEvent) : void
      {
         this.searchIcon.alpha = event.type == FocusHandlerEvent.FOCUS_IN ? 0.8 : 0.3;
      }
      
      private function cancelSearch(event:ButtonEvent) : void
      {
         this.textInput.text = "";
         this.unpdatePlayerList();
         this.showSearch(true);
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
            case Keyboard.ESCAPE:
               if(details.value == InputValue.KEY_DOWN || details.value == InputValue.KEY_UP)
               {
                  if(stage.focus == this.textInput.textField || stage.focus == this.textInput.textField)
                  {
                     stage.focus = this;
                     event.preventDefault();
                  }
               }
               break;
            case Keyboard.ENTER:
               if(details.value == InputValue.KEY_DOWN)
               {
                  if(this.textInput.textField.text.length > 2)
                  {
                     this.hideHint();
                     this.requestPlayersList();
                  }
               }
            break;
         }
      }
      
      protected function onLocalization(key:String) : void
      {
         this.textInput.defaultText = LocalizationData.getName("players_list_filter_text");
         this.hintContainer.textField.text = LocalizationData.getName("friend_search_hint");
      }
      
      protected function onLostEnter(event:FocusEvent) : void
      {
         trace("onLostEnter");
         if(this.textInput.textField.text == "")
         {
            this.textInput.textField.addEventListener(FocusEvent.FOCUS_IN,this.onStartEnter);
         }
         this.hideHint();
      }
      
      private function logListData(event:Event) : void
      {
         trace("logListData",this.playersList.visible);
      }
      
      protected function onStartEnter(event:FocusEvent) : void
      {
         trace("onStartEnter");
         this.textInput.textField.removeEventListener(FocusEvent.FOCUS_IN,this.onStartEnter);
         this.textInput.text = "";
      }
      
      protected function searchInput(event:Event = null) : void
      {
         var friends:Array;
         var found:Array;
         this.hintContainer.visible = true;
         this.hintContainer.alpha = 0;
         if(this.hintContainer.alpha == 0 && !this.hintDelayed)
         {
            this.showHint();
         }
         else
         {
            this.hideHint();
            this.showHint();
         }
         this.showSearch(true);
         if(this.textInput.textField.text.length > 0)
         {
            friends = this._friendsList;
            found = [];
            found = found.concat(this.filterPlayers(this.filterOnlinePlayers(friends),this.textInput.textField.text));
            this.playersList.dataProvider = new DataProvider(found);
            this.unpdatePlayerListHeight();
         }
         else
         {
            this.unpdatePlayerList();
         }
      }
      
      private function unpdatePlayerListHeight() : void
      {
         stackTrace("unpdatePlayerListHeight");
         trace("unpdatePlayerListHeight",this.playersList.dataProvider);
         this.playersList.height = this.playersList.dataProvider && this.playersList.dataProvider.length > 0 ? Math.min(10,this.playersList.dataProvider.length) * this.playersList.rowHeight + 8 : 1;
         this.playersList.visible = this.playersList.height > 1;
         this.playersList.y = this._dropDown ? this.textInput.height + 10 : -this.playersList.height;
      }
      
      private function unpdatePlayerList() : void
      {
         this.playersList.dataProvider = new DataProvider(this.filterOnlinePlayers(this._friendsList));
         this.unpdatePlayerListHeight();
      }
      
      protected function hideHint() : void
      {
         TweenLite.killTweensOf(this.hintContainer);
         if(this.hintContainer.alpha != 0)
         {
            TweenLite.to(this.hintContainer,0.3,{alpha:0});
         }
         hintDelayed = false;
         stage.removeEventListener(MouseEvent.MOUSE_MOVE,this.onStageMouseMove);
      }
      
      protected function requestPlayersList(event:Event = null) : void
      {
         var query:String = this.textInput.textField.text;
         if(query.length < 3 || query == this.textInput.defaultText)
         {
            return;
         }
         dispatchEvent(new Event(Event.CONNECT,true));
         ExternalInterface.call("find_players",query);
         this.loadingIndicator.gotoAndPlay(1);
         this.showSearch(false);
         this.btnCancelSearch.visible = false;
      }
      
      protected function showSearch(show:Boolean) : void
      {
         this.searchIcon.visible = show;
         this.btnCancelSearch.enabled = !show;
      }
      
      protected function showHint() : void
      {
         hintDelayed = true;
         TweenLite.to(this.hintContainer,0.3,{
            delay:1.5,
            alpha:1
         });
         stage.addEventListener(MouseEvent.MOUSE_MOVE,this.onStageMouseMove);
      }
      
      private function onStageMouseMove(event:MouseEvent) : void
      {
         this.hideHint();
      }
      
      protected function filterOnlinePlayers(players:Array) : Array
      {
         var onlineOnly:Boolean = false;
         if(this.btnFilter)
         {
            onlineOnly = !this.btnFilter.selected;
         }
         if(!onlineOnly)
         {
            return players;
         }
         var result:Array = [];
         var i:uint;
         var status:int;
         if(players && players.length)
         {
            i = 0;
            while(i < players.length)
            {
               status = (players[i].status);
               if(status != FriendStatus.OFFLINE)
               {
                  result.push(players[i]);
               }
               i++;
            }
         }
         return result;
      }
      
      protected function filterPlayers(players:Array, query:String) : Array
      {
         var result:Array = [];
         var i:uint;
         var playerName:String;
         if(query == this.textInput.defaultText || query == "")
         {
            return players;
         }
         query = query.toLowerCase();
         if(players && players.length)
         {
            i = 0;
            while(i < players.length)
            {
               playerName = (players[i].name as String).toLowerCase();
               if(playerName.indexOf(query) != -1)
               {
                  result.push(players[i]);
               }
               i++;
            }
         }
         return result;
      }
   }
}

