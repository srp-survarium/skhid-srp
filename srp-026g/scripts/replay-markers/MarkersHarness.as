package
{
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;

   // The replay player's one-string lists from the exe (survarium.replay.ReplayText, the root's
   // set_markers_text and set_players_text): the markers bar and the players bar show the good
   // entries (in time order, a label holding ":"), the prev/next label buttons step through them,
   // an empty string empties the bar. Prints "MARKERS ..." lines and ends with
   // "MARKERS RESULT pass" or "MARKERS RESULT fail: ...".
   public class MarkersHarness extends Sprite
   {
      private var loader:Loader = new Loader();

      private var failures:Array = [];

      public function MarkersHarness()
      {
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
         });
         loader.load(new URLRequest("replay_player.swf"),new LoaderContext(false,new ApplicationDomain()));
      }

      private function expect(what:String, ok:Boolean) : void
      {
         if(!ok)
            failures.push(what);
      }

      private function loaded(e:Event) : void
      {
         addChild(loader);
         // the root's frame script runs once it is on the stage
         addEventListener(Event.ENTER_FRAME,this.check);
      }

      private function check(e:Event) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.check);
         try
         {
            markers(loader.content);
            players(loader.content);
         }
         catch(err:Error)
         {
            failures.push("the player: " + err.getStackTrace());
         }
         finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
      }

      private function labels(root:*) : Array
      {
         var out:Array = [];
         var data:* = root.labelsBar.dataProvider;
         for(var i:int = 0; data != null && i < data.length; i++)
            out.push(data[i].time + "=" + data[i].label);
         return out;
      }

      private function markers(root:*) : void
      {
         root.set_match_time(600000);
         root.set_markers_text("125000:1:2:05;bad;30000:1:;9x:1:no;61000:2:Battery;");
         var got:String = labels(root).join(",");
         expect("markers are " + got,got == "30000=,61000=Battery,125000=2:05");
         expect("the markers bar hidden",root.labelsBar.visible);
         root.labelsBar.selectedIndex = 0;
         root.labelsBar.validateNow();
         root.checkLabelButtons();
         expect("previous on the first marker",!root.btn_previous_label.enabled && root.btn_next_label.enabled);
         var Click:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
         root.btn_next_label.dispatchEvent(new Click(Object(Click).CLICK));
         expect("next went to marker " + root.labelsBar.selectedIndex + ", the slider to " + root.slider.value,
            root.labelsBar.selectedIndex == 1 && root.slider.value == 61000);
         root.btn_next_label.dispatchEvent(new Click(Object(Click).CLICK));
         root.btn_previous_label.dispatchEvent(new Click(Object(Click).CLICK));
         expect("next, previous went to marker " + root.labelsBar.selectedIndex,root.labelsBar.selectedIndex == 1 && root.slider.value == 61000);
         root.set_markers_text("");
         expect("an empty string left markers",labels(root).length == 0 && !root.labelsBar.visible);
         trace("MARKERS markers: good entries in time order, next and previous step through them, an empty string empties the bar");
      }

      private function players(root:*) : void
      {
         var side:* = loader.contentLoaderInfo.applicationDomain.getDefinition("survarium.constants.TeamSide");
         root.set_players_text("3:1:Bob;0:0:Ada;20:0:Far;4:2:Odd;5:0:;7:0:Eve:2");
         var data:* = root.playersBar.dataProvider;
         var got:Array = [];
         for(var i:int = 0; data != null && i < data.length; i++)
            got.push(data[i].id + ":" + data[i].team + ":" + data[i].name);
         var want:String = ["0:" + side.ALLY + ":Ada","7:" + side.ALLY + ":Eve:2","3:" + side.ENEMY + ":Bob"].sort().join(",");
         expect("players are " + got.join(","),got.sort().join(",") == want);
         root.set_players_text("");
         expect("an empty string left players",root.playersBar.dataProvider.length == 0 && !root.playersBar.visible);
         trace("MARKERS players: good entries, teams as ally and enemy");
      }

      private function finish(result:String) : void
      {
         trace("MARKERS RESULT " + result);
      }
   }
}
