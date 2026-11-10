package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsLook;
   import survarium.rooms.RoomsWindow;
   import survarium.watch.WatchWindow;
   
   public class TopMenu extends UIComponent
   {
      
      public var tabPage:ButtonBar;
      
      public const CHAR_INDEX:uint = 0;
      
      public const SHOP_INDEX:uint = 2;
      
      public const INV_INDEX:uint = 1;
      
      public const CLAN_INDEX:uint = 3;
      
      public const QUESTS_INDEX:uint = 7;
      
      public const RANK_INDEX:uint = 6;
      
      public const MAP_INDEX:uint = 4;

      // srp's Watch (survarium.watch), between Rooms and Ranking
      public const WATCH_INDEX:uint = 5;
      
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

      // The Watch tab: our eye in place of its art, and on it the count of the live matches that
      // can be watched now: red, yellow when a friend plays in one; what it counts said under the
      // tab while the arrow is on it.
      private var _watchIcon:Shape;

      private var _watchBadge:Sprite;

      private var _watchTip:Sprite;

      // the badge as last drawn: its text and colour
      private var _watchSaid:String = null;

      private var _watchFriend:Boolean = false;
      
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
            name:LocalizationData.getName("st_srp_watch_title"),
            icon:6
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
         this.watchTabChanged();
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
      // (Watch's too) then wait with them.
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
            // Watch is on and off with it
            this.tabPage.dataProvider[this.WATCH_INDEX] = {
               "name":LocalizationData.getName("st_srp_watch_title"),
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
            // measured shown (Ruffle has no bounds for a hidden clip)
            art.visible = true;
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

      // R (LobbyMenu.lobbyKey): the rooms' tab chosen, as a click on it, or left when it is chosen.
      public function toggleRoomsTab() : void
      {
         if(!this._roomsTabOn)
         {
            return;
         }
         this.selectActiveWindow(this.tabPage.selectedIndex == this.MAP_INDEX ? -1 : int(this.MAP_INDEX));
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
      
      // On each live list (WatchWindow): the Watch tab's eye, as the rooms' tab has its own icon,
      // and `live` (the live matches that can be watched now) on a badge at its top right, none
      // for 0: `more` adds a plus (the list is at its most), `friend` (one plays in them) makes
      // it yellow.
      public function paintWatchTab(live:int, more:Boolean = false, friend:Boolean = false) : void
      {
         try
         {
            var tab:* = this.getRenderer(this.WATCH_INDEX);
            var art:MovieClip = tab != null ? tab.icon as MovieClip : null;
            if(art == null)
            {
               return;
            }
            if(this._watchIcon == null || this._watchIcon.parent != tab)
            {
               this._watchIcon = new Shape();
               var g:* = this._watchIcon.graphics;
               // one tone, as the other tabs' art: an eye
               g.lineStyle(3,0x4E5457);
               g.moveTo(-14,0);
               g.curveTo(0,-14,14,0);
               g.curveTo(0,14,-14,0);
               g.lineStyle();
               g.beginFill(0x4E5457,1);
               g.drawCircle(0,0,4.5);
               g.endFill();
               tab.addChild(this._watchIcon);
               this._watchBadge = new Sprite();
               this._watchBadge.name = "srpWatchBadge";
               this._watchBadge.mouseEnabled = false;
               this._watchBadge.mouseChildren = false;
               var field:TextField = new TextField();
               var label:TextField = tab.textField as TextField;
               field.embedFonts = true;
               field.selectable = false;
               field.autoSize = TextFieldAutoSize.LEFT;
               field.defaultTextFormat = new TextFormat(label != null ? label.getTextFormat().font : null,12,0xFFFFFF,true);
               this._watchBadge.addChild(field);
               tab.addChild(this._watchBadge);
               this._watchSaid = null;
               tab.addEventListener(MouseEvent.ROLL_OVER,this.onWatchTabOver);
               tab.addEventListener(MouseEvent.ROLL_OUT,this.onWatchTabOut);
            }
            art.visible = true;
            var box:Rectangle = art.getBounds(tab);
            if(box.width > 0)
            {
               this._watchIcon.x = Math.round(box.x + box.width / 2);
               this._watchIcon.y = Math.round(box.y + box.height / 2);
            }
            art.visible = false;
            this._watchIcon.alpha = art.alpha;
            this._watchIcon.transform.colorTransform = art.transform.colorTransform;
            var said:String = live > 0 ? String(live) + (more ? "+" : "") : "";
            if(said != this._watchSaid || friend != this._watchFriend)
            {
               this._watchSaid = said;
               this._watchFriend = friend;
               this.drawWatchBadge();
               if(this._watchTip != null && this._watchTip.visible)
               {
                  this.showWatchTip(tab);
               }
            }
            this._watchBadge.x = int(this._watchIcon.x + 12);
            this._watchBadge.y = int(this._watchIcon.y - 18);
         }
         catch(e:Error)
         {
         }
      }

      private function drawWatchBadge() : void
      {
         var field:TextField = this._watchBadge.getChildAt(0) as TextField;
         this._watchBadge.visible = this._watchSaid != "";
         // dark digits on yellow, white on red
         field.textColor = this._watchFriend ? 0x1B1B1B : 0xFFFFFF;
         field.text = this._watchSaid;
         var w:int = Math.max(18,int(field.textWidth + 10));
         // On the text itself: a TextField has a 2 px gutter on each side, and a narrow digit
         // centred by the field's own box sits off the middle of the circle.
         field.x = Math.round((w - field.textWidth) / 2) - 2;
         field.y = Math.round((18 - field.textHeight) / 2) - 2;
         this._watchBadge.graphics.clear();
         this._watchBadge.graphics.beginFill(this._watchFriend ? RoomsLook.YELLOW : 0xC62B22,1);
         this._watchBadge.graphics.drawRoundRect(0,0,w,18,18,18);
         this._watchBadge.graphics.endFill();
      }

      private function onWatchTabOver(e:MouseEvent) : void
      {
         this.showWatchTip(e.currentTarget as DisplayObject);
      }

      private function onWatchTabOut(e:MouseEvent) : void
      {
         if(this._watchTip != null)
         {
            this._watchTip.visible = false;
         }
      }

      // What the badge counts, in a small box under the Watch tab; none without a badge.
      private function showWatchTip(tab:DisplayObject) : void
      {
         if(this._watchTip == null)
         {
            this._watchTip = new Sprite();
            this._watchTip.name = "srpWatchTip";
            this._watchTip.mouseEnabled = false;
            this._watchTip.mouseChildren = false;
            var words:TextField = new TextField();
            var badge:TextField = this._watchBadge.getChildAt(0) as TextField;
            words.embedFonts = true;
            words.selectable = false;
            words.autoSize = TextFieldAutoSize.LEFT;
            words.defaultTextFormat = new TextFormat(badge.defaultTextFormat.font,13,RoomsLook.LIGHT,true);
            words.x = 8;
            words.y = 4;
            this._watchTip.addChild(words);
            addChild(this._watchTip);
         }
         var field:TextField = this._watchTip.getChildAt(0) as TextField;
         this._watchTip.visible = this._watchSaid != null && this._watchSaid != "";
         if(!this._watchTip.visible)
         {
            return;
         }
         field.text = LocalizationData.getName(this._watchFriend ? "st_srp_watch_badge_friend" : "st_srp_watch_badge_live");
         var w:int = int(field.textWidth + 20);
         var h:int = int(field.textHeight + 12);
         this._watchTip.graphics.clear();
         this._watchTip.graphics.lineStyle(1,0x3A4248);
         this._watchTip.graphics.beginFill(0x0C0E10,0.95);
         this._watchTip.graphics.drawRect(0,0,w,h);
         this._watchTip.graphics.endFill();
         var box:Rectangle = tab.getBounds(this);
         this._watchTip.x = int(box.x + (box.width - w) / 2);
         this._watchTip.y = int(box.bottom + 4);
         setChildIndex(this._watchTip,numChildren - 1);
      }

      // The Watch window opened or closed: its tab follows, as the rooms' does (selectRoomsTab).
      public function selectWatchTab(on:Boolean) : void
      {
         this._roomsSyncing = true;
         try
         {
            var at:int = this.tabPage.selectedIndex;
            if(on && at != this.WATCH_INDEX)
            {
               this.selectActiveWindow(this.WATCH_INDEX);
            }
            else if(!on && at == this.WATCH_INDEX)
            {
               this.selectActiveWindow(-1);
            }
         }
         catch(e:Error)
         {
         }
         this._roomsSyncing = false;
      }

      // A tab chosen: Watch opens its window, another closes it.
      private function watchTabChanged() : void
      {
         try
         {
            WatchWindow.topMenu = this;
            if(WatchWindow.me != null && !this._roomsSyncing)
            {
               WatchWindow.me.tabChosen(this.tabPage.selectedIndex == this.WATCH_INDEX);
            }
         }
         catch(err:Error)
         {
         }
      }

      // The tabs made: the map's (its index, which the game leaves off) is
      // the rooms': Rooms, and the countdown while their window is hidden.
      private function nameRoomsTab() : void
      {
         try
         {
            RoomsWindow.topMenu = this;
            WatchWindow.topMenu = this;
            if(RoomsWindow.me != null)
            {
               RoomsWindow.me.retitle();
            }
            else
            {
               this.setRoomsTab(LocalizationData.getName("st_srp_rooms_servers"));
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

