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

   // The Watch window as the lobby draws it, for a screenshot (shots.sh): the lobby movie's own top
   // bar, bottom bar, History and WatchWindow (WatchLobby) on a stage of the game's size, told
   // WatchFake's rows. Parameters: movie, language, view (lobby, mine, friends, search, live, reasons,
   // copied, results, history). Traces "SHOT ready" once drawn.
   public class WatchShots extends Sprite
   {
      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:WatchLobby;

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

      private function click(box:DisplayObjectContainer, name:String) : void
      {
         var Click:* = domain.getDefinition("scaleform.clik.events.ButtonEvent");
         var b:DisplayObject = WatchLobby.find(box,name);
         if(b != null)
            b.dispatchEvent(new Click(Click.CLICK));
      }

      private function press(box:DisplayObjectContainer, name:String) : void
      {
         var b:DisplayObject = WatchLobby.find(box,name);
         if(b != null)
            b.dispatchEvent(new MouseEvent(MouseEvent.CLICK));
      }

      private function loaded(e:Event) : void
      {
         var view:String = loaderInfo.parameters.view || "lobby";
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            lobby = new WatchLobby(domain,this,WatchLobby.texts(loaderInfo.parameters.language || "english"));
            var W:* = lobby.W;
            lobby.take(WatchFake.LIVE);
            lobby.take(WatchFake.played("PocketTest1"));
            lobby.history.visible = view == "history";
            // (Ruffle stops drawing the top menu's own icons once the History's rows are made)
            lobby.top.visible = view != "history" && view != "results";
            if(view == "history" || view == "results")
            {
               lobby.historyRow(WatchFake.MATCH,"Rudnya","87:64",0);
               lobby.historyRow(0x22,"School","1:3",0);
               lobby.historyRow(0x21,"Chemical Plant","100:64",1);
            }
            if(view == "results")
            {
               var screen:* = lobby.make("SrvVictoryScreenReskin");
               screen.x = int((1920 - 1199) / 2);
               screen.y = int((1080 - 730) / 2);
               lobby.holder.addChild(screen);
               try
               {
                  screen.setData({match_id:WatchFake.OTHER,score_components:[],statistics_rating:{},statistics_exp:{},statistics_money:{},
                     statistics_reputation:{faction_id:1},team1_score:3,team2_score:1,allow_drop_card:false,dropped_item:null,players:[]});
               }
               catch(err:Error)
               {
               }
            }
            else if(view != "lobby" && view != "history")
            {
               // the top menu's Watch tab, as a player opens it
               lobby.top.selectActiveWindow(lobby.top.WATCH_INDEX);
               if(view == "mine" || view == "copied")
               {
                  press(W.me,"srpWatchTab_replays");
                  lobby.take(WatchFake.played("PocketTest1"));
                  if(view == "copied")
                     press(WatchLobby.find(W.me,"srpWatchCard1") as DisplayObjectContainer,"srpWatchCopy");
               }
               else if(view == "friends")
               {
                  press(W.me,"srpWatchTab_replays");
                  press(W.me,"srpWatchChip_friends");
                  lobby.take(WatchFake.FRIENDS);
               }
               else if(view == "reasons")
               {
                  press(W.me,"srpWatchTab_replays");
                  press(W.me,"srpWatchChip_everyone");
                  lobby.take(WatchFake.RECENT);
               }
               else if(view == "search")
               {
                  W.openPlayer("ada");
                  W.found([{"id":5,"name":"Ada"},{"id":6,"name":"Adam_K"}]);
                  lobby.take(WatchFake.played("Ada"));
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
            if(++frames == 40)
            {
               removeEventListener(Event.ENTER_FRAME,wait);
               trace("SHOT ready " + stage.stageWidth + "x" + stage.stageHeight + " " + view);
            }
         });
      }
   }
}
