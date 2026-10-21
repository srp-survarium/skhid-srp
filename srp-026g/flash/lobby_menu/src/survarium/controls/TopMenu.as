package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.events.Event;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsLook;
   import survarium.rooms.RoomsText;
   import survarium.rooms.RoomsWindow;
   
   public class TopMenu extends UIComponent
   {
      
      public var tabPage:ButtonBar;
      
      public const CHAR_INDEX:uint = 0;
      
      public const SHOP_INDEX:uint = 2;
      
      public const INV_INDEX:uint = 1;
      
      public const CLAN_INDEX:uint = 3;
      
      public const QUESTS_INDEX:uint = 6;
      
      public const RANK_INDEX:uint = 5;
      
      public const MAP_INDEX:uint = 4;
      
      protected var sceneContainer:MovieClip;
      
      protected var currentUI:UIWindow;
      
      protected var locked:Boolean = false;
      
      protected var _byUser:Boolean = true;
      
      protected var _questButton:Button;

      // The map's tab is the rooms' (survarium.rooms): on as the Clan tab
      // is, chosen while the rooms window is open (not then a choice of the
      // player's), our own icon on it and its label yellow in a room.
      private var _roomsTabOn:Boolean = true;

      private var _roomsSyncing:Boolean;

      private var _roomsIcon:Shape;

      private var _roomsLabel:TextField;

      private var _roomsLabelColour:uint;
      
      public function TopMenu()
      {
         super();
         LocalizationData.addCallback(this.fill_windows,"character");
      }
      
      public function get inventoryTab() : MovieClip
      {
         return this.getRenderer(this.INV_INDEX);
      }
      
      public function get questsTab() : MovieClip
      {
         return this.getRenderer(this.QUESTS_INDEX);
      }
      
      public function get skillsTab() : MovieClip
      {
         return this.getRenderer(this.CHAR_INDEX);
      }
      
      public function set questButton(value:Button) : void
      {
         _questButton = value;
         this._questButton.toggle = true;
         this._questButton.addEventListener(ButtonEvent.CLICK,this.showQuests);
      }
      
      public function get questButton() : Button
      {
         return this._questButton;
      }
      
      private function showQuests(event:ButtonEvent) : void
      {
         trace("TOP::showQuests");
         if(this.getActiveWindow() == this.QUESTS_INDEX)
         {
            this.selectActiveWindow(-1);
         }
         else
         {
            this.selectActiveWindow(this.QUESTS_INDEX);
         }
      }
      
      private function setByUser(value:Boolean) : void
      {
         trace("setByUser",value);
         _byUser = value;
         stackTrace("_affectByUser");
      }
      
      public function selectActiveWindow(windowIndex:int, byUser:Boolean = true) : void
      {
         trace("selectActiveWindow::",windowIndex,byUser);
         if(windowIndex != -1 && this.tabPage.selectedIndex != windowIndex)
         {
            this.setByUser(byUser);
         }
         this.tabPage.selectedIndex = windowIndex;
      }
      
      public function getActiveWindow() : int
      {
         return this.tabPage.selectedIndex;
      }
      
      public function inventoryLocked() : Boolean
      {
         return this.locked;
      }
      
      public function setActiveFaction(faction:int) : void
      {
      }
      
      public function getRenderer(tabIndex:int) : MovieClip
      {
         return this.tabPage.getButtonAt(tabIndex);
      }
      
      public function lockIndex(tabIndex:int, isLocked:Boolean) : void
      {
         var tabData:Object = this.tabPage.dataProvider[tabIndex];
         if(tabData)
         {
            tabData.enabled = !isLocked;
         }
         if(this.QUESTS_INDEX == tabIndex)
         {
            this._questButton.enabled = tabData.enabled;
         }
         trace("lockIndex",tabIndex);
         this.tabPage.dataProvider.invalidate();
      }
      
      public function lockInventory(isLocked:Boolean) : void
      {
         if(this.locked == isLocked)
         {
            return;
         }
         locked = isLocked;
         this.tabPage.dataProvider[this.CHAR_INDEX] = {
            name:LocalizationData.getName("character"),
            icon:1,
            enabled:!isLocked
         };
         this.tabPage.dataProvider[this.SHOP_INDEX] = {
            name:LocalizationData.getName("shop"),
            icon:3,
            enabled:!isLocked
         };
         this.tabPage.dataProvider[this.INV_INDEX] = {
            name:LocalizationData.getName("inventrory"),
            icon:2,
            enabled:!isLocked
         };
         if((this.tabPage.selectedIndex == this.SHOP_INDEX || this.tabPage.selectedIndex == this.INV_INDEX || this.tabPage.selectedIndex == this.CHAR_INDEX) && isLocked == true)
         {
            this.tabPage.selectedIndex = -1;
         }
         this.tabPage.dataProvider.invalidate();
      }
      
      protected function fill_windows(section:String) : void
      {
         this.tabPage.dataProvider = new DataProvider([{
            name:LocalizationData.getName("character"),
            icon:1
         },{
            name:LocalizationData.getName("inventrory"),
            icon:2
         },{
            name:LocalizationData.getName("shop"),
            icon:3
         },{
            name:LocalizationData.getName("clan"),
            icon:4
         },{
            name:LocalizationData.getName("map"),
            icon:6,
            enabled:false
         },{
            name:LocalizationData.getName("ranking"),
            icon:5
         },{
            name:LocalizationData.getName("missions"),
            icon:7,
            enabled:true
         }]);
         this.tabPage.addEventListener(IndexEvent.INDEX_CHANGE,this.changeActiveMenu);
         this.nameRoomsTab();
      }
      
      protected function changeActiveMenu(event:IndexEvent) : void
      {
         if(MainScene.activePop && MainScene.activePop.closePopup)
         {
            MainScene.activePop.closePopup();
         }
         if(this.currentUI && this.currentUI.visible == true)
         {
            this.currentUI.removeEventListener(Event.CLOSE,this.onClose);
            this.currentUI.closeWindow();
            MainScene.activeWindow = null;
         }
         this._questButton.selected = this.tabPage.selectedIndex == this.QUESTS_INDEX;
         if(this.tabPage.selectedIndex == this.CHAR_INDEX)
         {
            currentUI = this.sceneContainer["charWindow"];
         }
         else if(this.tabPage.selectedIndex == this.INV_INDEX)
         {
            currentUI = this.sceneContainer["inventory_list"];
            this.sceneContainer["inventory_list"].toggleShopMode(false);
         }
         else if(this.tabPage.selectedIndex == this.SHOP_INDEX)
         {
            currentUI = this.sceneContainer["inventory_list"];
            this.sceneContainer["inventory_list"].toggleShopMode(true);
         }
         else if(this.tabPage.selectedIndex == this.QUESTS_INDEX)
         {
            currentUI = this.sceneContainer["questWindow"];
            trace("QUESTS_INDEX",this.currentUI);
            this.sceneContainer["questWindow"].onActivate();
         }
         else if(this.tabPage.selectedIndex == this.RANK_INDEX)
         {
            currentUI = this.sceneContainer["ranking"];
            this.sceneContainer["ranking"].onActivate();
         }
         else if(this.tabPage.selectedIndex == this.CLAN_INDEX)
         {
            currentUI = this.sceneContainer["clanWindow"];
            this.sceneContainer["clanWindow"].onActivate();
         }
         else
         {
            currentUI = null;
            MainScene.activeWindow = null;
         }
         if(this.currentUI)
         {
            trace("changeActiveMenu",this._byUser);
            MainScene.activeWindow = this.currentUI;
            if(this.sceneContainer["victory_screen"] && this.sceneContainer["victory_screen"].parent)
            {
               this.sceneContainer.addChildAt(this.currentUI,Math.max(this.sceneContainer.getChildIndex(this.sceneContainer["victory_screen"]) - 1,0));
            }
            else
            {
               this.sceneContainer.addChild(this.currentUI);
            }
            this.currentUI.activate(true,this._byUser);
            this.currentUI.addEventListener(Event.CLOSE,this.onClose);
         }
         else
         {
            MainScene.activeWindow = null;
         }
         _byUser = true;
         this.roomsTabChanged();
      }
      
      protected function onClose(event:Event) : void
      {
         this.currentUI.removeEventListener(Event.CLOSE,this.onClose);
         this.tabPage.selectedIndex = -1;
      }
      
      override protected function configUI() : void
      {
         sceneContainer = parent.parent as MovieClip;
         this.sceneContainer["shop_list"].visible = false;
         this.sceneContainer["charWindow"].visible = false;
         this.sceneContainer["inventory_list"].visible = false;
         this.sceneContainer["ranking"].visible = false;
      }
      
      // The rooms' tab: its label (the rooms window's), on as the Clan tab
      // is: the lobby locks its tabs for a moment while it loads, and ours
      // then waits with them.
      public function setRoomsTab(text:String) : void
      {
         try
         {
            var clan:Object = this.tabPage.dataProvider[this.CLAN_INDEX];
            this._roomsTabOn = clan == null || clan.enabled !== false;
            this.tabPage.dataProvider[this.MAP_INDEX] = {
               "name":text,
               "icon":6,
               "enabled":this._roomsTabOn
            };
            this.tabPage.dataProvider.invalidate();
         }
         catch(e:Error)
         {
         }
      }

      // Each frame: the Clan tab's lock came or went, ours follows.
      public function followClanTab(text:String) : void
      {
         try
         {
            var clan:Object = this.tabPage.dataProvider[this.CLAN_INDEX];
            if((clan == null || clan.enabled !== false) != this._roomsTabOn)
            {
               this.setRoomsTab(text);
            }
         }
         catch(e:Error)
         {
         }
      }

      // Each frame: our own icon on the rooms' tab, as the map's art (frame 6)
      // is drawn darker than the others: a stack of three servers in the
      // game icon's place, as bright as the tab makes its icon (its alpha
      // and colour transform, copied). Yellow, and the label too, while
      // `marked` (in a room or a room's queue; the owner's of 2026-10-20).
      public function paintRoomsTab(marked:Boolean) : void
      {
         try
         {
            var tab:* = this.getRenderer(this.MAP_INDEX);
            var art:MovieClip = tab.icon as MovieClip;
            if(tab == null || art == null)
            {
               return;
            }
            if(this._roomsIcon == null || this._roomsIcon.parent != tab)
            {
               this._roomsIcon = new Shape();
               var g:* = this._roomsIcon.graphics;
               // One tone, as the skull of the Clan tab: three servers,
               // each a small square and a bar to its right.
               g.beginFill(0x4E5457,1);
               for(var i:int = 0; i < 3; i++)
               {
                  g.drawRect(-13,-12 + i * 9,6,6);
                  g.drawRect(-4,-12 + i * 9,17,6);
               }
               g.endFill();
               tab.addChild(this._roomsIcon);
            }
            var box:Rectangle = art.getBounds(tab);
            if(box.width > 0)
            {
               this._roomsIcon.x = Math.round(box.x + box.width / 2);
               this._roomsIcon.y = Math.round(box.y + box.height / 2);
            }
            art.visible = false;
            this._roomsIcon.alpha = art.alpha;
            this._roomsIcon.transform.colorTransform = marked ? RoomsLook.YELLOW_TINT : art.transform.colorTransform;
            this.colourRoomsLabel(tab.textField as TextField,marked);
         }
         catch(e:Error)
         {
         }
      }

      // The tab's label: yellow while marked, its own colour otherwise (the
      // tab sets it again on each state: ours each frame).
      private function colourRoomsLabel(field:TextField, marked:Boolean) : void
      {
         if(field == null)
         {
            return;
         }
         if(field != this._roomsLabel)
         {
            this._roomsLabel = field;
            this._roomsLabelColour = field.textColor;
         }
         if(marked)
         {
            if(field.textColor != RoomsLook.YELLOW)
            {
               field.textColor = RoomsLook.YELLOW;
            }
         }
         else if(field.textColor == RoomsLook.YELLOW)
         {
            field.textColor = this._roomsLabelColour;
         }
      }

      // The rooms window opened or closed: our tab follows, chosen while it
      // is open, without that choice opening or closing it again.
      public function selectRoomsTab(on:Boolean) : void
      {
         this._roomsSyncing = true;
         try
         {
            var at:int = this.tabPage.selectedIndex;
            if(on && at != this.MAP_INDEX)
            {
               this.selectActiveWindow(this.MAP_INDEX);
            }
            else if(!on && at == this.MAP_INDEX)
            {
               this.selectActiveWindow(-1);
            }
         }
         catch(e:Error)
         {
         }
         this._roomsSyncing = false;
      }
      
      // The tabs made: the map's (its index, which the game leaves off) is
      // the rooms': Rooms, and the countdown while their window is hidden.
      private function nameRoomsTab() : void
      {
         try
         {
            RoomsWindow.topMenu = this;
            if(RoomsWindow.me != null)
            {
               RoomsWindow.me.retitle();
            }
            else
            {
               this.setRoomsTab(RoomsText.sayFirst("servers"));
            }
         }
         catch(err:Error)
         {
         }
      }

      // A tab chosen: the rooms' opens their window; another closes it (as
      // the game's own windows do).
      private function roomsTabChanged() : void
      {
         try
         {
            RoomsWindow.topMenu = this;
            if(RoomsWindow.me != null && !this._roomsSyncing)
            {
               RoomsWindow.me.tabChosen(this.tabPage.selectedIndex == this.MAP_INDEX);
            }
         }
         catch(err:Error)
         {
         }
      }
   }
}

