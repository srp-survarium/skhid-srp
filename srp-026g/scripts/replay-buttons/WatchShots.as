package
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.display.StageAlign;
   import flash.display.StageScaleMode;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;

   // The Watch window as the lobby draws it, for a screenshot (shots.sh): the lobby movie's own
   // bottom bar and WatchWindow on a stage of the game's size, told WatchFake's rows. Parameters:
   // movie, view (live, roster, recent, recent-answered, mine, player, empty, history). Traces
   // "SHOT ready" once drawn.
   public class WatchShots extends Sprite
   {
      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      public function WatchShots()
      {
         stage.align = StageAlign.TOP_LEFT;
         stage.scaleMode = StageScaleMode.NO_SCALE;
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
         });
         loader.load(new URLRequest(loaderInfo.parameters.movie || "lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
      }

      private static function find(box:DisplayObjectContainer, name:String) : DisplayObject
      {
         for(var i:int = 0; i < box.numChildren; i++)
         {
            var child:DisplayObject = box.getChildAt(i);
            if(child.name == name)
               return child;
            if(child is DisplayObjectContainer)
            {
               var deeper:DisplayObject = find(child as DisplayObjectContainer,name);
               if(deeper != null)
                  return deeper;
            }
         }
         return null;
      }

      private function take(names:Array) : void
      {
         domain.getDefinition("survarium.watch.WatchWindow").take(WatchFake.rows(names));
      }

      // `button` pressed, then the server's answer about `hex`.
      private function asked(button:DisplayObject, status:String, hex:String) : void
      {
         var Click:* = domain.getDefinition("scaleform.clik.events.ButtonEvent");
         if(button != null)
            button.dispatchEvent(new Click(Click.CLICK));
         answer(status,hex);
         answer("failed",hex);
      }

      private function answer(status:String, hex:String) : void
      {
         domain.getDefinition("survarium.controls.NotificationListScrollContent").replayTake(
            [{"nickname":"R1;" + status + ";" + hex + ";0","place":0,"ranking":0,"ranking_diff":0}]);
      }

      private function loaded(e:Event) : void
      {
         var view:String = loaderInfo.parameters.view || "live";
         var shown:DisplayObject = null;
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            domain.getDefinition("survarium.data.LocalizationData").setData(HarnessText.DATA.english);
            var Scene:* = domain.getDefinition("MainScene");
            Scene.stage = this;
            Scene.layout = new (domain.getDefinition("scaleform.clik.layout.Layout") as Class)();
            // the History, the prompt's home (and the History shot)
            var list:* = new (domain.getDefinition("SrvNotificationListSC") as Class)();
            var types:* = domain.getDefinition("survarium.constants.NotificationType");
            list.registerRenderer(types.MATCH,domain.getDefinition("NotificationMatchRenderer"));
            addChild(list);
            list.x = stage.stageWidth - 420;
            list.y = 120;
            list.addNotification({"notificationType":types.MATCH,"label":"","match_id":WatchFake.MATCH,"map_name":"Vostok Radar Station",
               "time":"19:05","result":"100:98","match_result":1,"reward":null});
            list.addNotification({"notificationType":types.MATCH,"label":"","match_id":WatchFake.OTHER,"map_name":"Rudnya",
               "time":"18:31","result":"3:1","match_result":0,"reward":null});
            var panel:* = new (domain.getDefinition("SrvBottomPanel") as Class)();
            panel.y = stage.stageHeight - panel.height + 13;
            addChild(panel);
            var W:* = domain.getDefinition("survarium.watch.WatchWindow");
            if(W.me == null)
               W.me = new W(panel);
            addChild(W.me);
            W.me.layout();
            if(view == "history")
            {
               var watches:Array = [];
               for(var i:int = 0; i < list.numChildren * 4; i++)
               {
                  var w:DisplayObject = find(list,"srpReplayWatch");
                  if(w == null || watches.indexOf(w) >= 0)
                     break;
                  watches.push(w);
                  w.name = "seen";
               }
               asked(watches.length > 0 ? watches[0] : null,"watching","1ce45cbe");
               asked(watches.length > 1 ? watches[1] : null,"gone","69f8c4bc");
               shown = list;
            }
            else
            {
               list.visible = false;
               shown = W.me;
               if(view == "live" || view == "roster" || view == "empty")
               {
                  W.open("live");
                  take(view == "empty" ? ["L0;0"] : WatchFake.LIVE);
                  if(view == "roster")
                  {
                     W.spectate("Petro");
                     take(WatchFake.ROSTER);
                  }
               }
               else if(view == "recent" || view == "recent-answered")
               {
                  W.open("recent");
                  take(WatchFake.RECENT);
                  if(view == "recent-answered")
                  {
                     asked(find(find(W.me,"srpWatchCard1") as DisplayObjectContainer,"srpWatchWatch"),"preparing","69f8c4bc");
                  }
               }
               else if(view == "mine")
               {
                  W.open("mine");
                  take(WatchFake.played("PocketTest1"));
               }
               else if(view == "player")
               {
                  W.open("player","Ada");
                  take(WatchFake.played("Ada"));
               }
            }
         }
         catch(err:Error)
         {
            trace("SHOT fail: " + err.getStackTrace());
            return;
         }
         var frames:int = 0;
         addEventListener(Event.ENTER_FRAME,function wait(f:Event):void
         {
            if(++frames == 20)
            {
               removeEventListener(Event.ENTER_FRAME,wait);
               trace("SHOT ready " + stage.stageWidth + "x" + stage.stageHeight + " " + view + " scale " + shown.scaleX);
            }
         });
      }
   }
}
