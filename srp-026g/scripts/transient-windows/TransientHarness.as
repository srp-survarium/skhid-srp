package
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;

   // The lobby's transient windows (docs/notes/transient-windows.md) with the lobby movie's own
   // root, History list and bottom bar, top bar (its match settings under Play) and rooms window,
   // and a CLIK dropdown in the match settings made as their bot-match ones are (their own are
   // wired on a render this stage does not give); the shop is a stand-in opened as the top menu
   // opens it (MainScene.activeWindow), the Settings button a stand-in in the top bar.
   // - History opens (its button selected) over the windows; the shop, the rooms window, a press on
   //   Settings each close it; it opens again; with nothing else opened it stays;
   // - one at a time: the match settings close History, History closes them; a dropdown opened
   //   in the match settings keeps them open, a press in them closes only the dropdown;
   // - a press outside closes it and goes on to what it is on (its click arrives);
   // - Esc closes the top one first (the dropdown, then the match settings), then has nothing;
   // - the match settings opened over the rooms window are over it and take the press there;
   //   closed, the top bar is under the rooms window again;
   // - a resize closes them.
   // Prints "TRANSIENT ..." lines (trace) and ends with "TRANSIENT RESULT pass" or
   // "TRANSIENT RESULT fail: ...".
   public class TransientHarness extends Sprite
   {
      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:*;

      private var Transient:*;

      private var MainScene:*;

      private var history:*;

      private var button:*;

      private var status:*;

      private var settings:*;

      private var rooms:*;

      private var shop:MovieClip;

      private var settingsButton:Sprite;

      private var dropdown:*;

      private var failures:Array = [];

      private var frames:int = 0;

      public function TransientHarness()
      {
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         // the lobby's own root fails outside the game (no exe): its parts are made here
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
            if(frames > 0)
            {
               trace("TRANSIENT error " + e.error);
            }
         });
         loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
      }

      private function make(name:String) : *
      {
         return new (domain.getDefinition(name) as Class)();
      }

      private function loaded(e:Event) : void
      {
         var step:String = "the classes";
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            step = "the texts";
            domain.getDefinition("survarium.data.LocalizationData").setData(HarnessText.DATA.english);
            Transient = domain.getDefinition("survarium.controls.Transient");
            MainScene = domain.getDefinition("MainScene");
            lobby = loader.content;
            addChild(loader);
            step = "the bottom bar";
            var panel:* = make("SrvBottomPanel");
            panel.y = 680;
            lobby.bottom_panel = panel;
            lobby.addChild(panel);
            step = "the top bar";
            status = make("SrvTopMenuContainer");
            lobby.statusPanel = status;
            lobby.addChild(status);
            step = "the rooms window";
            var Win:* = domain.getDefinition("survarium.rooms.RoomsWindow");
            rooms = new Win(panel);
            Win.me = rooms;
            lobby.addChild(rooms);
            step = "History";
            history = make("SrvNotificationListSC");
            history.visible = false;
            lobby.notificationList = history;
            lobby.notificationListTemp = make("SrvNotificationList");
            lobby.addChild(history);
            lobby.addChild(lobby.notificationListTemp);
            history.x = 700;
            history.y = 300;
            step = "the shop";
            shop = new MovieClip();
            shop.graphics.beginFill(0x202020,1);
            shop.graphics.drawRect(0,0,600,500);
            shop.visible = false;
            shop.closeWindow = function():void
            {
               shop.visible = false;
            };
            lobby.addChild(shop);
            settingsButton = new Sprite();
            settingsButton.graphics.beginFill(0x808080,1);
            settingsButton.graphics.drawRect(0,0,30,30);
            settingsButton.x = 1200;
            status.addChild(settingsButton);
         }
         catch(err:Error)
         {
            finish("fail: " + step + ": " + err);
            return;
         }
         addEventListener(Event.ENTER_FRAME,this.onFrame);
      }

      // The CLIK parts lay themselves out over a few frames first.
      // History left open at frame 20 is checked again at frame 80: nothing else opened, it stays.
      private function onFrame(e:Event) : void
      {
         ++frames;
         try
         {
            if(frames == 20)
            {
               button = lobby.bottom_panel.btnShowLogList;
               settings = status.matchSettings;
               if(button == null || settings == null)
               {
                  removeEventListener(Event.ENTER_FRAME,this.onFrame);
                  finish("fail: no History button or match settings (" + button + ", " + settings + ")");
                  return;
               }
               checkHistory();
            }
            if(frames < 80)
               return;
            removeEventListener(Event.ENTER_FRAME,this.onFrame);
            expect("with nothing else opened History stays (60 frames)",historyOpen());
            pressHistory();
            expect("History's button closes it",historyClosed());
            checkOneAtATime();
            checkPressOutside();
            checkEsc();
            checkOverRooms();
            checkResize();
         }
         catch(err:Error)
         {
            failures.push("threw: " + err + " " + err.getStackTrace());
         }
         finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
      }

      private function expect(what:String, ok:Boolean) : void
      {
         trace("TRANSIENT " + what + ": " + (ok ? "yes" : "NO"));
         if(!ok)
            failures.push(what);
      }

      private function historyOpen() : Boolean
      {
         return history.visible && button.selected && Transient.isOpen(history);
      }

      private function historyClosed() : Boolean
      {
         return !history.visible && !button.selected && !Transient.isOpen(history);
      }

      private function settingsOpen() : Boolean
      {
         return settings.visible && Transient.isOpen(settings);
      }

      private function settingsClosed() : Boolean
      {
         return !settings.visible && !Transient.isOpen(settings);
      }

      private function above(a:DisplayObject, b:DisplayObject) : Boolean
      {
         return lobby.getChildIndex(a) > lobby.getChildIndex(b);
      }

      // A press (down, up, click) on `target`, as the player's mouse makes it: the stage's
      // capture listeners see it first.
      private function press(target:DisplayObject) : void
      {
         var at:Point = target.localToGlobal(new Point(2,2));
         var local:Point = target.globalToLocal(at);
         for each(var type:String in [MouseEvent.MOUSE_DOWN,MouseEvent.MOUSE_UP,MouseEvent.CLICK])
            target.dispatchEvent(new MouseEvent(type,true,false,local.x,local.y));
      }

      // The arrow beside the mode under Play, pressed (CLIK's button makes its click, which opens
      // or closes the settings).
      private function pressArrow() : void
      {
         press(status.dropDownButton);
      }

      // History's button pressed: its click is LobbyMenu.toggleReports (showHistory).
      private function pressHistory() : void
      {
         press(button);
         lobby.showHistory(!history.visible);
      }

      private function openShop() : void
      {
         shop.visible = true;
         MainScene.activeWindow = shop;
         lobby.addChild(shop);
      }

      private function closeShop() : void
      {
         MainScene.activeWindow = null;
         shop.visible = false;
      }

      private function checkHistory() : void
      {
         pressHistory();
         expect("History opens, its button selected",historyOpen());
         expect("History opened is over the windows open",above(history,rooms) && above(history,status) && above(history,shop));
         openShop();
         expect("the shop opening closes History",historyClosed());
         expect("the shop is over History",above(shop,history));
         closeShop();
         pressHistory();
         expect("History opens again over the shop's place",historyOpen() && above(history,shop));
         rooms.show(true);
         expect("the rooms window opening closes History",historyClosed());
         expect("the rooms window is over History",above(rooms,history));
         pressHistory();
         expect("History opens again over the rooms window",historyOpen() && above(history,rooms));
         press(settingsButton);
         expect("a press on Settings closes History",historyClosed());
         rooms.show(false);
         pressHistory();
         expect("History opens again",historyOpen());
         press(history);
         expect("a press inside History keeps it",historyOpen());
         press(button);
         expect("a press on History's button is its own (it stays until its click)",historyOpen());
      }

      // A dropdown in the match settings, made as their bot-match ones are
      // (MatchMakingSettings.pvbNewMenu), with three rows.
      private function pvbMenu() : *
      {
         if(dropdown != null)
            return dropdown;
         var DataProvider:Class = domain.getDefinition("scaleform.clik.data.DataProvider") as Class;
         dropdown = make("SrvDropDownMenuP");
         dropdown.dropdown = "OptionScrollingList";
         dropdown.itemRenderer = "OptionsListRenderer";
         dropdown.scrollBar = "";
         dropdown.menuDirection = "down";
         dropdown.menuRowCount = 3;
         dropdown.dataProvider = new DataProvider(["Small","Medium","Large"]);
         dropdown.x = 20;
         dropdown.y = 40;
         dropdown.width = 200;
         settings.addChild(dropdown);
         return dropdown;
      }

      // The first `kind` in `box`'s tree (a dropdown's list, on CLIK's popup canvas).
      private function find(box:DisplayObjectContainer, kind:Class) : DisplayObject
      {
         for(var i:int = 0; i < box.numChildren; i++)
         {
            var child:DisplayObject = box.getChildAt(i);
            if(child is kind)
               return child;
            var inner:DisplayObjectContainer = child as DisplayObjectContainer;
            var found:DisplayObject = inner != null ? find(inner,kind) : null;
            if(found != null)
               return found;
         }
         return null;
      }

      private function checkOneAtATime() : void
      {
         pressHistory();
         pressArrow();
         expect("the match settings open",settingsOpen());
         expect("the match settings opening closes History",historyClosed());
         pressHistory();
         expect("History opening closes the match settings",settingsClosed() && historyOpen());
         pressHistory();
         pressArrow();
         var menu:* = pvbMenu();
         if(menu == null)
         {
            expect("the match settings have their bot-match dropdowns",false);
            return;
         }
         menu.open();
         var list:DisplayObject = find(stage,domain.getDefinition("OptionScrollingList") as Class);
         trace("TRANSIENT the dropdown's list " + list + (list != null ? " on the stage: " + (list.stage != null) : ""));
         expect("a dropdown in the match settings opens its list",menu.selected && list != null && Transient.isOpen(list));
         if(list != null && list.stage != null)
         {
            press(list);
            expect("a press in the dropdown's list keeps it and the match settings",menu.selected && settingsOpen());
         }
         expect("the dropdown opening keeps the match settings",settingsOpen());
         press(settings);
         expect("a press in the match settings closes the dropdown only",!menu.selected && settingsOpen());
         menu.open();
         pressHistory();
         expect("History opening closes the dropdown and the match settings",!menu.selected && settingsClosed() && historyOpen());
         pressHistory();
      }

      private function checkPressOutside() : void
      {
         var clicks:int = 0;
         var target:Sprite = new Sprite();
         target.graphics.beginFill(0x00ff00,1);
         target.graphics.drawRect(0,0,40,40);
         target.addEventListener(MouseEvent.CLICK,function(e:MouseEvent):void
         {
            clicks++;
         });
         lobby.addChild(target);
         pressHistory();
         press(target);
         expect("a press outside History closes it",historyClosed());
         expect("the press outside goes on to what it is on",clicks == 1);
         pressArrow();
         press(target);
         expect("a press outside the match settings closes them",settingsClosed());
         expect("the press outside the match settings goes on too",clicks == 2);
         lobby.removeChild(target);
      }

      private function checkEsc() : void
      {
         lobby.has_active_window();
         expect("nothing open: Esc is the game's",!lobby.has_active_window());
         pressHistory();
         expect("History open: Esc is the lobby's",lobby.has_active_window());
         lobby.close_active_window();
         expect("Esc closes History",historyClosed());
         lobby.has_active_window();
         pressArrow();
         var menu:* = pvbMenu();
         if(menu != null)
         {
            menu.open();
            expect("a dropdown open: Esc is the lobby's",lobby.has_active_window());
            lobby.close_active_window();
            expect("Esc closes the dropdown first",!menu.selected && settingsOpen());
            lobby.has_active_window();
         }
         expect("the match settings open: Esc is the lobby's",lobby.has_active_window());
         lobby.close_active_window();
         expect("Esc closes the match settings",settingsClosed());
         lobby.has_active_window();
         expect("all closed: Esc is the game's again",!lobby.has_active_window());
      }

      // The match settings under Play opened over the rooms window (the owner's screenshot of
      // 2026-10-30: their list behind "Private room").
      private function checkOverRooms() : void
      {
         rooms.show(true);
         expect("the rooms window is over the top bar",above(rooms,status));
         pressArrow();
         expect("the match settings open over the rooms window",settingsOpen() && above(status,rooms));
         var box:Rectangle = settings.getBounds(stage);
         var room:Rectangle = rooms.getBounds(stage);
         var at:Point = new Point(box.x + Math.min(box.width,room.width) / 2,Math.max(box.y,room.y) + 10);
         trace("TRANSIENT match settings " + box + ", rooms window " + room + ", pressed at " + at);
         expect("the press point is on both the match settings and the rooms window",box.containsPoint(at) && room.containsPoint(at));
         var under:Array = stage.getObjectsUnderPoint(at);
         var topmost:DisplayObject = under.length > 0 ? under[under.length - 1] as DisplayObject : null;
         expect("the topmost thing there is the match settings' (" + topmost + ")",topmost != null && (topmost == settings || settings.contains(topmost)));
         pressArrow();
         expect("the arrow closes the match settings",settingsClosed());
         expect("closed, the top bar is under the rooms window again",above(rooms,status));
         rooms.show(false);
      }

      private function checkResize() : void
      {
         pressHistory();
         stage.dispatchEvent(new Event(Event.RESIZE));
         expect("a resize closes History",historyClosed());
      }

      private function finish(result:String) : void
      {
         trace("TRANSIENT RESULT " + result);
      }
   }
}
