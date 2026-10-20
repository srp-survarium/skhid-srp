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
      
      public function setDropDown(param1:Boolean) : void
      {
         _dropDown = param1;
      }
      
      public function fillFoundPlayers(param1:Array) : void
      {
         this.playersList.dataProvider = new DataProvider(param1);
         this.playersList.selectedIndex = 0;
         this.loadingIndicator.gotoAndStop(1);
         this.btnCancelSearch.visible = true;
         this.unpdatePlayerListHeight();
      }
      
      public function setFriendsList(param1:Array) : void
      {
         _friendsList = param1;
         if(!this.btnCancelSearch.visible)
         {
            this.unpdatePlayerList();
         }
      }
      
      public function cleanUp(param1:Boolean = true) : void
      {
         if(param1)
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
      
      private function toggleOnlineFilter(param1:ButtonEvent) : void
      {
         this.unpdatePlayerList();
      }
      
      private function onSearchClick(param1:MouseEvent) : void
      {
         stage.focus = this;
         this.requestPlayersList();
      }
      
      private function onInputFocus(param1:FocusHandlerEvent) : void
      {
         this.searchIcon.alpha = param1.type == FocusHandlerEvent.FOCUS_IN ? 0.8 : 0.3;
      }
      
      private function cancelSearch(param1:ButtonEvent) : void
      {
         this.textInput.text = "";
         this.unpdatePlayerList();
         this.showSearch(true);
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
                  if(stage.focus == this.textInput.textField || stage.focus == this.textInput.textField)
                  {
                     stage.focus = this;
                     param1.preventDefault();
                  }
               }
               break;
            case Keyboard.ENTER:
               if(_loc2_.value == InputValue.KEY_DOWN)
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
      
      protected function onLocalization(param1:String) : void
      {
         this.textInput.defaultText = LocalizationData.getName("players_list_filter_text");
         this.hintContainer.textField.text = LocalizationData.getName("friend_search_hint");
      }
      
      protected function onLostEnter(param1:FocusEvent) : void
      {
         trace("onLostEnter");
         if(this.textInput.textField.text == "")
         {
            this.textInput.textField.addEventListener(FocusEvent.FOCUS_IN,this.onStartEnter);
         }
         this.hideHint();
      }
      
      private function logListData(param1:Event) : void
      {
         trace("logListData",this.playersList.visible);
      }
      
      protected function onStartEnter(param1:FocusEvent) : void
      {
         trace("onStartEnter");
         this.textInput.textField.removeEventListener(FocusEvent.FOCUS_IN,this.onStartEnter);
         this.textInput.text = "";
      }
      
      protected function searchInput(param1:Event = null) : void
      {
         var _loc2_:Array;
         var _loc3_:Array;
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
            _loc2_ = this._friendsList;
            _loc3_ = [];
            _loc3_ = _loc3_.concat(this.filterPlayers(this.filterOnlinePlayers(_loc2_),this.textInput.textField.text));
            this.playersList.dataProvider = new DataProvider(_loc3_);
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
      
      protected function requestPlayersList(param1:Event = null) : void
      {
         var _loc2_:String = this.textInput.textField.text;
         if(_loc2_.length < 3 || _loc2_ == this.textInput.defaultText)
         {
            return;
         }
         dispatchEvent(new Event(Event.CONNECT,true));
         ExternalInterface.call("find_players",_loc2_);
         this.loadingIndicator.gotoAndPlay(1);
         this.showSearch(false);
         this.btnCancelSearch.visible = false;
      }
      
      protected function showSearch(param1:Boolean) : void
      {
         this.searchIcon.visible = param1;
         this.btnCancelSearch.enabled = !param1;
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
      
      private function onStageMouseMove(param1:MouseEvent) : void
      {
         this.hideHint();
      }
      
      protected function filterOnlinePlayers(param1:Array) : Array
      {
         var _loc2_:Boolean = false;
         if(this.btnFilter)
         {
            _loc2_ = !this.btnFilter.selected;
         }
         if(!_loc2_)
         {
            return param1;
         }
         var _loc3_:Array = [];
         var _loc4_:uint;
         var _loc5_:int;
         if(param1 && param1.length)
         {
            _loc4_ = 0;
            while(_loc4_ < param1.length)
            {
               _loc5_ = (param1[_loc4_].status);
               if(_loc5_ != 2)
               {
                  _loc3_.push(param1[_loc4_]);
               }
               _loc4_++;
            }
         }
         return _loc3_;
      }
      
      protected function filterPlayers(param1:Array, param2:String) : Array
      {
         var _loc3_:Array = [];
         var _loc4_:uint;
         var _loc5_:String;
         if(param2 == this.textInput.defaultText || param2 == "")
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
   }
}

