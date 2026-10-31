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
   import survarium.controls.Modals;
   import survarium.controls.PlayerSearchComponent;
   import survarium.controls.SrvContextMenu;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IFriendsList;
   import survarium.constants.UiSound;
   
   public class SquadList extends UIComponent
   {
      
      // a squad's most members: two (crates/session/src/squads.rs; the
      // owner's of 2026-10-31). The panel has two places, and a squad of two
      // is full: neither the friends' nor the chat's menu offers "Invite to
      // squad" then (PlayerInfo.isSquadFull), so nothing invites a third.
      private static const MAX_SQUAD_MEMBERS:int = 2;
      
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
      
      public static function isInSquad(playerName:String) : Boolean
      {
         var i:int = 0;
         var member:Object;
         while(i < _squadList.length)
         {
            member = _squadList[i];
            if(member && member.name == playerName)
            {
               return true;
            }
            i++;
         }
         return false;
      }
      
      public function setFriendsList(friendsList:IFriendsList) : void
      {
         _friendsList = friendsList;
      }
      
      public function lock(locked:Boolean) : void
      {
         this.textFieldLabel.alpha = locked ? 0.6 : 1;
         this.list.enabled = !locked;
         if(locked && this.searchComponent.visible)
         {
            this.closeComponent();
         }
         this.htntArea.visible = this.lockIcon.visible = locked;
         if(locked)
         {
            this.htntArea.tipData = {name:LocalizationData.getName("squad_list_locked_hint")};
         }
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         trace("SL::handleInput::",event.isDefaultPrevented());
         if(event.isDefaultPrevented())
         {
            return;
         }
         var details:InputDetails = event.details;
         var controllerIndex:uint = details.controllerIndex;
         trace("details.code",details.code);
         switch(details.code)
         {
            // Esc: the search is a modal (Modals, onItemClick)
            case Keyboard.BACK:
               if(details.value == InputValue.KEY_UP)
               {
                  if(this.searchComponent.visible == true && !(stage.focus is TextField))
                  {
                     this.closeComponent(null);
                     event.preventDefault();
                  }
               }
            break;
         }
      }
      
      protected function onLocalization(value:String) : void
      {
         this.border.tooltipRendererName = "SrvTextHintRenderer";
         this.updateBordrerHint();
      }
      
      protected function closeComponent(event:Event = null) : void
      {
         this.searchComponent.visible = false;
         this.searchComponent.cleanUp();
         this.onLostEnter(null);
         MainScene.activeComponent = null;
         Modals.leave(this.searchComponent);
      }
      
      protected function searchInput(event:Event) : void
      {
      }
      
      protected function requestPlayersList(searchText:String) : void
      {
      }
      
      protected function keyDownHandler(event:KeyboardEvent) : void
      {
         if(event.keyCode == Keyboard.ENTER)
         {
         }
      }
      
      protected function addFriend(event:ListEvent = null) : void
      {
         trace("try_add_friend",event.target,event.currentTarget);
         if(!event || (SrvContextMenu.getInstance()) && (SrvContextMenu.getInstance().parent))
         {
            return;
         }
         var itemData:* = event.itemData;
         var player:Object;
         // A search left open from before the squad filled invites nobody.
         if(itemData && !PlayerInfo.isSquadFull)
         {
            player = itemData;
            trace("invite_to_squad::",player.name);
            ExternalInterface.call("invite_to_squad",player.name);
         }
         this.searchComponent.cleanUp();
         this.searchComponent.visible = false;
         this.onLostEnter(null);
      }
      
      protected function onNewIndex(event:ListEvent = null) : void
      {
      }
      
      protected function onLostEnter(event:FocusEvent) : void
      {
      }
      
      protected function onStartEnter(event:FocusEvent) : void
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
      
      protected function onFocusChange(event:FocusEvent) : void
      {
         _inFocus = event.type == FocusEvent.FOCUS_IN;
         addEventListener(Event.RENDER,this.afterFocusChange);
      }
      
      protected function afterFocusChange(event:Event) : void
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
      
      protected function onRenderReaction(event:Event) : void
      {
         _hidden = !this._hidden;
         this.updateBordrerHint();
         this.arrow.scaleX *= -1;
         var offset:int = -75 * this.arrow.scaleX;
         layoutData.offsetH += offset;
         this.closeComponent();
         TweenLite.to(this,0.2,{x:x + offset});
         TweenLite.to(this.border,0.2,{x:(this._hidden ? 93 - 7 : 93)});
         TweenLite.to(this.arrow,0.2,{x:(this._hidden ? 101 - 7 : 101)});
         ExternalInterface.call("sound_play",UiSound.SQUAD_PANEL_SLIDE);
      }
      
      private function updateSize() : void
      {
         var lastFilled:Boolean = this.list.dataProvider[this.list.dataProvider.length - 1] != null;
         trace("SL::updateSize::",this.background,this.list,this.border,"|",this.list.dataProvider.length);
         this.list.height = this.list.dataProvider.length * this.list.rowHeight;
         this.list.validateNow();
         this.background.height = this.list.dataProvider.length * this.list.rowHeight + 48;
         this.border.setActualSize(this.border.width,this.background.height - 16);
         this.arrow.y = (this.background.height - 16 - this.arrow.height) / 2 + 8;
         this.overlay.height = this.background.height - 7;
         var allReady:uint = 1;
         var memberCount:uint = 0;
         var i:int = 0;
         var member:Object;
         while(i < _squadList.length)
         {
            member = _squadList[i];
            if(member)
            {
               memberCount++;
            }
            if(member && !member.ready)
            {
               allReady = 0;
            }
            i++;
         }
         PlayerInfo.isInSquad = memberCount > 0;
         PlayerInfo.isSquadFull = memberCount >= MAX_SQUAD_MEMBERS;
         this.onNewIndex();
      }
      
      private function onItemClick(event:ListEvent) : void
      {
         trace("onItemClick::",this.searchComponent.visible,PlayerInfo.commander);
         if(this.searchComponent.visible == true)
         {
            this.closeComponent();
         }
         else if(!event.itemData && PlayerInfo.commander)
         {
            this.searchComponent.setFriendsList(this._friendsList.getFriendsList());
            this.searchComponent.visible = true;
            Modals.open(this.searchComponent,Modals.LOBBY,this.closeComponent,function():Boolean
            {
               return searchComponent.visible && searchComponent.stage != null;
            });
            trace("AtPos::",event.itemRenderer.y + 4 + 30);
            this.searchComponent.y = event.itemRenderer.y + 4 + 30;
            stage.focus = this.searchComponent.textInput;
         }
      }
      
      public function set_squad_player_type(slotIndex:uint, playerType:uint, weaponIcon:uint) : void
      {
         trace("set_squad_player_type",slotIndex,playerType,_squadList[slotIndex]);
         var member:Object = _squadList[slotIndex];
         if(member)
         {
            member.type = playerType;
            member.weapon = weaponIcon;
         }
         this._squadTypes[slotIndex] = playerType;
         this._squadWeapons[slotIndex] = weaponIcon;
         this.list.dataProvider = new DataProvider(_squadList);
         this.updateSize();
      }
      
      public function set_squad_player(slotIndex:uint, playerId:uint, playerName:String, isReady:Boolean, playerType:uint, weaponIcon:uint) : void
      {
         var savedType:int = (this._squadTypes[slotIndex]);
         var savedWeapon:int = (this._squadWeapons[slotIndex]);
         trace("set_squad_player",slotIndex,_squadList.length,savedType);
         _squadList[slotIndex] = {
            index:slotIndex,
            id:playerId,
            name:playerName,
            ready:isReady,
            type:savedType,
            weapon:savedWeapon
         };
         _squadList.length = MAX_SQUAD_MEMBERS;
         this.list.dataProvider = new DataProvider(_squadList);
         this.updateSize();
      }
      
      public function remove_squad_player(slotIndex:uint) : void
      {
         trace("remove_squad_player",slotIndex,_squadList.length);
         if(_squadList[slotIndex])
         {
            _squadList[slotIndex] = null;
         }
         if(slotIndex == 0)
         {
            _squadList.length = 1;
         }
         this.list.dataProvider = new DataProvider(_squadList);
         this.updateSize();
      }
   }
}

