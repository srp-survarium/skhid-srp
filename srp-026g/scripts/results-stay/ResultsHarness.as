package
{
   import flash.display.DisplayObject;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;

   // The match results stay up while the rooms window changes under them (docs/notes/rooms.md,
   // "After a match"), with the lobby movie's own LobbyMenu.show_victory_screen and onVsRemoved, its
   // results screen (SrvVictoryScreenReskin), top menu (SrvTopMenu) and rooms window
   // (survarium.rooms.RoomsWindow), told the server's answers (`S1;R`, `S1;N`, lobby.md 2.16.3)
   // through RoomsWindow.take. The lobby's root does not run its own setup outside the game, so
   // the harness gives it the parts show_victory_screen uses. Each case opens the results, then:
   // - let in from a room's queue (a room new to the window; live 2026-10-30 20:58:59, one who left
   //   the room's match and queued for it again by Play was let in as that match ended and got its
   //   results): the results stay, the top menu's tab as it was; closing them shows the room's
   //   window, its tab chosen;
   // - the room goes on after its match (every member stays in it, however it came in; the
   //   owner's of 2026-10-31), its next vote told: the results stay over the room's window;
   //   closing them shows the room;
   // - out of the room while they are up (a kick, a maintenance): the results stay, the window
   //   under them on the list (no room it is not in); closing them shows the list;
   // - the same, the results opened twice (the match's end and the screen's message, results.md
   //   hazard 11) with the answer between them: the results stay;
   // - the same, the answer before the results open, the room's window open all the while: the
   //   results stay.
   // Prints "RESULTS ..." lines (trace) and ends with "RESULTS RESULT pass" or
   // "RESULTS RESULT fail: ...".
   public class ResultsHarness extends Sprite
   {
      private static const STATE:int = 2060000001;

      private var loader:Loader = new Loader();

      private var domain:ApplicationDomain;

      private var lobby:Object;

      private var Win:*;

      private var win:*;

      private var top:*;

      private var results:*;

      private var failures:Array = [];

      public function ResultsHarness()
      {
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         // the lobby's own root fails outside the game (no exe): its parts are given below
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
         });
         loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
      }

      private function make(name:String) : *
      {
         return new (domain.getDefinition(name) as Class)();
      }

      private function loaded(e:Event) : void
      {
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
            domain.getDefinition("survarium.data.LocalizationData").setData(HarnessText.DATA.english);
            // srp's tables, as exe patch 94 gives them (root.srp_data).
            domain.getDefinition("survarium.data.SrpData").take(HarnessText.SRP_DATA.rooms);
            lobby = loader.content;
            addChild(loader);
            domain.getDefinition("MainScene").layout = make("scaleform.clik.layout.Layout");
            lobby.notificationListTemp = make("SrvNotificationList");
            Win = domain.getDefinition("survarium.rooms.RoomsWindow");
         }
         catch(err:Error)
         {
            finish("fail: the lobby: " + err);
            return;
         }
         queueAdmit();
         roomGoesOn();
         outUnderResults();
         outBetweenTwoOpenings();
         outBeforeTheResults();
         finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
      }

      // A lobby as after a match: a top menu with no window chosen, a results screen, and a rooms
      // window over the bottom bar, hidden (the match's loading closed it).
      private function fresh() : void
      {
         if(results != null && results.parent != null)
            results.parent.removeChild(results);
         if(win != null && win.parent != null)
            win.parent.removeChild(win);
         if(top != null && top.parent != null)
            top.parent.removeChild(top);
         top = make("SrvTopMenu");
         lobby.addChild(top);
         lobby.topMenu = top;
         Win.topMenu = top;
         results = make("SrvVictoryScreenReskin");
         lobby.victory_screen = results;
         var panel:* = make("SrvBottomPanel");
         win = new Win(panel);
         Win.me = win;
         lobby.addChild(win);
         // the lobby's first word on Play: the window knows its lobby (and its results)
         win.playEnabled(lobby,true);
         win.show(false);
      }

      // The results screen opened as the exe opens it (show_match_statistic).
      private function openResults() : void
      {
         try
         {
            lobby.show_victory_screen();
         }
         catch(err:Error)
         {
            // its last line scrolls the chat, which the harness has not
         }
      }

      private function resultsUp() : Boolean
      {
         return results.visible && results.stage != null;
      }

      // The player closes the results (their own close). Its first line tells the exe, which Ruffle
      // has not (ExternalInterface throws): the rest of it, the removal, is done here.
      private function closeResults() : void
      {
         try
         {
            results.closeWindow();
         }
         catch(err:Error)
         {
         }
         if(results.parent != null)
            results.parent.removeChild(results);
      }

      private function tab() : int
      {
         return top.tabPage.selectedIndex;
      }

      private function queued(key:int) : void
      {
         tell(["S1;N;0;" + key + ";1"]);
      }

      private function outOfRoom() : void
      {
         tell(["S1;N;0;0;0"]);
      }

      // Room `key`'s state, the asker a member of it (not its owner) with three others, waiting.
      private function inRoom(key:int) : void
      {
         var names:Array = [["S1","R",key,15,15,10,30,0,754,0,4,5,key,0,63,2,0].join(";"),"C;0;61;0;0"];
         var who:Array = ["Alice","Bob","Carol","Dave"];
         for(var i:int = 0; i < who.length; i++)
            names.push(["P",i,15,i == 0 ? 1 : 0,i == 1 ? 1 : 0,0,0,0,who[i]].join(";"));
         names.push("L;0");
         tell(names);
      }

      private function tell(names:Array) : void
      {
         var rows:Array = [];
         for(var r:int = 0; r < names.length; r++)
            rows.push({"place":STATE + r, "nickname":names[r]});
         try
         {
            Win.take(rows);
         }
         catch(err:Error)
         {
            failures.push(names[0] + ": " + err);
         }
      }

      private function expect(what:String, ok:Boolean) : void
      {
         if(!ok)
            failures.push(what);
      }

      private function queueAdmit() : void
      {
         fresh();
         queued(3146);
         openResults();
         expect("queue admit: the results did not open",resultsUp());
         var before:int = tab();
         inRoom(3146);
         expect("queue admit: the results closed",resultsUp());
         expect("queue admit: the top tab changed under the results (" + before + " -> " + tab() + ")",tab() == before);
         closeResults();
         expect("queue admit: the room's window not shown after the results",win.shown);
         expect("queue admit: the Rooms tab not chosen after the results (" + tab() + ")",tab() == top.MAP_INDEX);
         trace("RESULTS let in from a queue under the results: they stay; closing them shows the room");
      }

      private function roomGoesOn() : void
      {
         fresh();
         inRoom(2215);
         openResults();
         expect("room goes on: the room's window not under the results",win.shown);
         var before:int = tab();
         inRoom(2215);
         expect("room goes on: the results closed",resultsUp());
         expect("room goes on: the top tab changed under the results",tab() == before);
         closeResults();
         expect("room goes on: the room's window not shown after the results",win.shown && win.inRoom);
         expect("room goes on: the Rooms tab not chosen after the results (" + tab() + ")",tab() == top.MAP_INDEX);
         trace("RESULTS the room goes on after its match: the results stay over it; closing them shows it");
      }

      private function outUnderResults() : void
      {
         fresh();
         inRoom(2215);
         openResults();
         var before:int = tab();
         outOfRoom();
         expect("out under the results: the results closed",resultsUp());
         expect("out under the results: the top tab changed under the results",tab() == before);
         expect("out under the results: the window still has the room",!win.inRoom);
         closeResults();
         expect("out under the results: the window not shown after the results",win.shown);
         expect("out under the results: the Rooms tab not chosen after the results (" + tab() + ")",tab() == top.MAP_INDEX);
         trace("RESULTS out of the room under the results: they stay; the window under them is on the list");
      }

      private function outBetweenTwoOpenings() : void
      {
         fresh();
         inRoom(2215);
         openResults();
         outOfRoom();
         openResults();
         expect("out, opened twice: the results closed",resultsUp());
         trace("RESULTS out of the room between two openings of the results: they stay");
      }

      // The answer before the results open, the room's window open all through the match (the
      // match's loading closed something else): the results open over the list and stay.
      private function outBeforeTheResults() : void
      {
         fresh();
         inRoom(2215);
         win.show(true);
         outOfRoom();
         openResults();
         expect("out before the results: the results did not open",resultsUp());
         var before:int = tab();
         outOfRoom();
         expect("out before the results: the results closed",resultsUp());
         expect("out before the results: the top tab changed under the results",tab() == before);
         trace("RESULTS out of the room before the results open: they stay");
      }

      private function finish(result:String) : void
      {
         trace("RESULTS RESULT " + result);
      }
   }
}
