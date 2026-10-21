package survarium.friends
{
   import com.greensock.TweenLite;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.KeyboardEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.ui.Keyboard;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ScrollingList;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.ui.InputDetails;
   import survarium.controls.PlayerInfo;
   import survarium.controls.PlayerSearchComponent;
   import survarium.controls.SrvContextMenu;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IFriendsList;
   
   public class SquadList extends UIComponent
   {
      
      private static var _squadList:Array = [];
      
      public var list:ScrollingList;
      
      public var textFieldLabel:TextField;
      
      public var searchComponent:PlayerSearchComponent;
      
      public var background:MovieClip;
      
      public var overlay:MovieClip;
      
      public var border:Button;
      
      public var arrow:MovieClip;
      
      public var lockIcon:MovieClip;
      
      public var htntArea:ToolTip;
      
      private var _inFocus:Boolean = false;
      
      private var _squadTypes:Array = [];
      
      private var _squadWeapons:Array = [];
      
      private var _friendsList:IFriendsList;
      
      private var _hidden:Boolean = false;
      
      public function SquadList()
      {
         super();
      }
      
      public static function isInSquad(param1:String) : Boolean
      {
         var _loc2_:int = 0;
         var _loc3_:Object;
         while(_loc2_ < _squadList.length)
         {
            _loc3_ = _squadList[_loc2_];
            if(_loc3_ && _loc3_.name == param1)
            {
               return true;
            }
            _loc2_++;
         }
         return false;
      }
      
      public function setFriendsList(param1:IFriendsList) : void
      {
         _friendsList = param1;
      }
      
      public function lock(param1:Boolean) : void
      {
         this.textFieldLabel.alpha = param1 ? 0.6 : 1;
         this.list.enabled = !param1;
         if(param1 && this.searchComponent.visible)
         {
            this.closeComponent();
         }
         this.htntArea.visible = this.lockIcon.visible = param1;
         if(param1)
         {
            this.htntArea.tipData = {name:LocalizationData.getName("squad_list_locked_hint")};
         }
      }
      
      override public function handleInput(param1:InputEvent) : void
      {
         trace("SL::handleInput::",param1.isDefaultPrevented());
         if(param1.isDefaultPrevented())
         {
            return;
         }
         var _loc2_:InputDetails = param1.details;
         var _loc3_:uint = _loc2_.controllerIndex;
         trace("details.code",_loc2_.code);
         switch(_loc2_.code)
         {
            case Keyboard.ESCAPE:
               trace("closeSearchComponent::",_loc2_.value,InputValue.KEY_UP);
               if(_loc2_.value == InputValue.KEY_UP)
               {
                  if(this.searchComponent.visible == true && !(stage.focus is TextField))
                  {
                     this.closeComponent(null);
                     param1.preventDefault();
                  }
               }
               break;
            case Keyboard.BACK:
               if(_loc2_.value == InputValue.KEY_UP)
               {
                  if(this.searchComponent.visible == true && !(stage.focus is TextField))
                  {
                     this.closeComponent(null);
                     param1.preventDefault();
                  }
               }
            break;
         }
      }
      
      protected function onLocalization(param1:String) : void
      {
         this.border.tooltipRendererName = "SrvTextHintRenderer";
         this.updateBordrerHint();
      }
      
      protected function closeComponent(param1:Event = null) : void
      {
         this.searchComponent.visible = false;
         this.searchComponent.cleanUp();
         this.onLostEnter(null);
         MainScene.activeComponent = null;
      }
      
      protected function searchInput(param1:Event) : void
      {
      }
      
      protected function requestPlayersList(param1:String) : void
      {
      }
      
      protected function keyDownHandler(param1:KeyboardEvent) : void
      {
         if(param1.keyCode == 13)
         {
         }
      }
      
      protected function addFriend(param1:ListEvent = null) : void
      {
         trace("try_add_friend",param1.target,param1.currentTarget);
         if(!param1 || (SrvContextMenu.getInstance()) && (SrvContextMenu.getInstance().parent))
         {
            return;
         }
         var _loc2_:* = param1.itemData;
         var _loc3_:Object;
         if(_loc2_)
         {
            _loc3_ = _loc2_;
            trace("invite_to_squad::",_loc3_.name);
            ExternalInterface.call("invite_to_squad",_loc3_.name);
         }
         this.searchComponent.cleanUp();
         this.searchComponent.visible = false;
         this.onLostEnter(null);
      }
      
      protected function onNewIndex(param1:ListEvent = null) : void
      {
      }
      
      protected function onLostEnter(param1:FocusEvent) : void
      {
      }
      
      protected function onStartEnter(param1:FocusEvent) : void
      {
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         trace("SquadListConfig",this.searchComponent);
         this.overlay.mouseEnabled = false;
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         this.arrow.mouseEnabled = false;
         this.arrow.mouseChildren = false;
         this.arrow.visible = false;
         this.border.visible = false;
         this.lockIcon.visible = this.htntArea.visible = false;
         this.htntArea.tooltipRendererName = "SrvProfileTipRenderer";
         this.arrow.scaleX *= -1;
         this.searchComponent.playersList.addEventListener(ListEvent.ITEM_CLICK,this.addFriend);
         this.searchComponent.playersList.addEventListener(ListEvent.INDEX_CHANGE,this.onNewIndex);
         this.searchComponent.btnClose.addEventListener(ButtonEvent.CLICK,this.closeComponent);
         _squadList.length = 1;
         this.list.dataProvider = new DataProvider(_squadList);
         this.textFieldLabel.defaultTextFormat = this.textFieldLabel.getTextFormat();
         LocalizationData.addTextField(this.textFieldLabel,"squad");
         this.list.addEventListener(ListEvent.ITEM_CLICK,this.onItemClick);
         this.searchComponent.visible = false;
         LocalizationData.addCallback(this.onLocalization,"enter_search_player_name");
         this.updateSize();
         this.border.addEventListener(ButtonEvent.CLICK,this.onRenderReaction);
         this.border.preventAutosizing = true;
         addEventListener(FocusEvent.FOCUS_IN,this.onFocusChange);
         addEventListener(FocusEvent.FOCUS_OUT,this.onFocusChange);
      }
      
      protected function onFocusChange(param1:FocusEvent) : void
      {
         _inFocus = param1.type == FocusEvent.FOCUS_IN;
         addEventListener(Event.RENDER,this.afterFocusChange);
      }
      
      protected function afterFocusChange(param1:Event) : void
      {
         removeEventListener(Event.RENDER,this.afterFocusChange);
         if(this._inFocus && this.searchComponent.visible)
         {
            MainScene.activeComponent = this.searchComponent;
         }
         else
         {
            MainScene.activeComponent = null;
         }
      }
      
      private function updateBordrerHint() : void
      {
         this.border.data = {name:LocalizationData.getName(this._hidden ? "show_squad_list" : "hide_squad_list")};
      }
      
      protected function onRenderReaction(param1:Event) : void
      {
         _hidden = !this._hidden;
         this.updateBordrerHint();
         this.arrow.scaleX *= -1;
         var _loc2_:int = -75 * this.arrow.scaleX;
         layoutData.offsetH += _loc2_;
         this.closeComponent();
         TweenLite.to(this,0.2,{x:x + _loc2_});
         TweenLite.to(this.border,0.2,{x:(this._hidden ? 93 - 7 : 93)});
         TweenLite.to(this.arrow,0.2,{x:(this._hidden ? 101 - 7 : 101)});
         ExternalInterface.call("sound_play",16);
      }
      
      private function updateSize() : void
      {
         var _loc1_:Boolean = this.list.dataProvider[this.list.dataProvider.length - 1] != null;
         trace("SL::updateSize::",this.background,this.list,this.border,"|",this.list.dataProvider.length);
         this.list.height = this.list.dataProvider.length * this.list.rowHeight;
         this.list.validateNow();
         this.background.height = this.list.dataProvider.length * this.list.rowHeight + 48;
         this.border.setActualSize(this.border.width,this.background.height - 16);
         this.arrow.y = (this.background.height - 16 - this.arrow.height) / 2 + 8;
         this.overlay.height = this.background.height - 7;
         var _loc2_:uint = 1;
         var _loc3_:uint = 0;
         var _loc4_:int = 0;
         var _loc5_:Object;
         while(_loc4_ < _squadList.length)
         {
            _loc5_ = _squadList[_loc4_];
            if(_loc5_)
            {
               _loc3_++;
            }
            if(_loc5_ && !_loc5_.ready)
            {
               _loc2_ = 0;
            }
            _loc4_++;
         }
         PlayerInfo.isInSquad = _loc3_ > 0;
         PlayerInfo.isSquadFull = _loc3_ == 3;
         this.onNewIndex();
      }
      
      private function onItemClick(param1:ListEvent) : void
      {
         trace("onItemClick::",this.searchComponent.visible,PlayerInfo.commander);
         if(this.searchComponent.visible == true)
         {
            this.closeComponent();
         }
         else if(!param1.itemData && PlayerInfo.commander)
         {
            this.searchComponent.setFriendsList(this._friendsList.getFriendsList());
            this.searchComponent.visible = true;
            trace("AtPos::",param1.itemRenderer.y + 4 + 30);
            this.searchComponent.y = param1.itemRenderer.y + 4 + 30;
            stage.focus = this.searchComponent.textInput;
         }
      }
      
      public function set_squad_player_type(param1:uint, param2:uint, param3:uint) : void
      {
         trace("set_squad_player_type",param1,param2,_squadList[param1]);
         var _loc4_:Object = _squadList[param1];
         if(_loc4_)
         {
            _loc4_.type = param2;
            _loc4_.weapon = param3;
         }
         this._squadTypes[param1] = param2;
         this._squadWeapons[param1] = param3;
         this.list.dataProvider = new DataProvider(_squadList);
         this.updateSize();
      }
      
      public function set_squad_player(param1:uint, param2:uint, param3:String, param4:Boolean, param5:uint, param6:uint) : void
      {
         var _loc7_:int = (this._squadTypes[param1]);
         var _loc8_:int = (this._squadWeapons[param1]);
         trace("set_squad_player",param1,_squadList.length,_loc7_);
         _squadList[param1] = {
            index:param1,
            id:param2,
            name:param3,
            ready:param4,
            type:_loc7_,
            weapon:_loc8_
         };
         _squadList.length = 3;
         this.list.dataProvider = new DataProvider(_squadList);
         this.updateSize();
      }
      
      public function remove_squad_player(param1:uint) : void
      {
         trace("remove_squad_player",param1,_squadList.length);
         if(_squadList[param1])
         {
            _squadList[param1] = null;
         }
         if(param1 == 0)
         {
            _squadList.length = 1;
         }
         this.list.dataProvider = new DataProvider(_squadList);
         this.updateSize();
      }
   }
}

