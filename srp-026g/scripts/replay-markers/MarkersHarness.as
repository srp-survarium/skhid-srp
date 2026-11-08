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
   // an empty string empties the bar; set_replay_speed(percent) shows 0.25x .. 8x in the speed
   // stepper (25 to 800, the nearest end outside), kept over the bottom panel's frames; a player's
   // name of 16 characters (Latin, the 10 languages' letters, wide ones) fits its row whole.
   // Parameter shot=1: the player set for a screenshot (shot.sh), "SHOT ready" once drawn.
   // Prints "MARKERS ..." lines and ends with "MARKERS RESULT pass" or "MARKERS RESULT fail: ...".
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

      // 16 characters each, in the 10 languages' letters
      private static const NAMES:Array = ["ReplayTesterLong","Maksym_Shevchuk1","Сергій_Коваленко","Łukasz_Żółkiewsk",
         "Ўладзімір_Шчэрба","Árvíztűrő_Tükör1","Jiří_Řehořovský1","Đorđe_Šćekić_Ćir","Äijälä_Öhrström1","Jürgen_Größmann1"];

      private var frames:int = 0;

      private function check(e:Event) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.check);
         var root:* = loader.content;
         if(loaderInfo.parameters.shot == "1")
         {
            shot(root);
            return;
         }
         try
         {
            markers(root);
            players(root);
            speed(root);
            root.show_buttons(false);
            root.show_buttons(true);
            longNames(root);
         }
         catch(err:Error)
         {
            failures.push("the player: " + err.getStackTrace());
         }
         // the bottom panel's frame scripts, then the renderers drawn
         addEventListener(Event.ENTER_FRAME,function later(f:Event):void
         {
            if(++frames < 5)
               return;
            removeEventListener(Event.ENTER_FRAME,later);
            try
            {
               expect("the speed after the bottom panel's frames reads '" + root.stepper.textField.text + "'",root.stepper.textField.text == "2x");
               expect("the bottom panel's frames set the speed to " + root.speedPercent + " %",root.speedPercent == 200);
               namesFit(root);
            }
            catch(err:Error)
            {
               failures.push("the player: " + err.getStackTrace());
            }
            finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
         });
      }

      private function speed(root:*) : void
      {
         var cases:Array = [[25,"0.25x"],[50,"0.5x"],[100,"1x"],[400,"4x"],[800,"8x"],[10,"0.25x"],[2000,"8x"],[200,"2x"]];
         for each(var c:Array in cases)
         {
            root.set_replay_speed(c[0]);
            var field:* = root.stepper.textField;
            expect("set_replay_speed(" + c[0] + ") reads '" + field.text + "', not '" + c[1] + "'",field.text == c[1]);
            expect("'" + c[1] + "' leaves the stepper's field",field.textWidth <= field.width);
         }
         expect("the speed is " + root.speedPercent + " %",root.speedPercent == 200);
         trace("MARKERS speed: set_replay_speed shows 0.25x .. 8x, the ends outside");
      }

      private function longNames(root:*) : void
      {
         var text:Array = [];
         for(var i:int = 0; i < NAMES.length; i++)
            text.push(i + ":" + i % 2 + ":" + NAMES[i]);
         root.set_players_text(text.join(";"));
      }

      private function namesFit(root:*) : void
      {
         var seen:int = 0;
         for(var i:int = 0; i < NAMES.length; i++)
         {
            var b:* = root.playersBar.getButtonAt(i);
            if(b == null)
               continue;
            seen++;
            var field:* = b.textField;
            var size:Number = Number(field.getTextFormat().size);
            expect("the name '" + field.text + "' is not whole",NAMES.indexOf(field.text) >= 0);
            expect("'" + field.text + "' needs " + int(field.textWidth) + " px, has " + int(field.width - 4) + ", scrolls " + field.maxScrollH + " (size " + size + ")",
               field.textWidth <= field.width - 4 && field.maxScrollH == 0);
            expect("'" + field.text + "' shrunk to " + size,size >= 8);
            expect("'" + field.text + "' overlaps the view buttons",field.x + field.textWidth <= b.btnFirstView.x);
         }
         expect("the players bar shows " + seen + " of " + NAMES.length,seen == NAMES.length);
         var last:* = root.playersBar.getButtonAt(NAMES.length - 1);
         var below:Number = last == null ? 0 : last.getBounds(root).bottom;
         expect("free view covers the last player (" + root.freeViewButton.y + " < " + below + ")",root.freeViewButton.y >= below - 1);
         trace("MARKERS names: 16 characters fit their row whole");
      }

      private function shot(root:*) : void
      {
         root.set_match_time(600000);
         root.set_markers_text("30000:1:;61000:2:Battery;125000:1:2:05");
         longNames(root);
         root.set_replay_speed(200);
         addEventListener(Event.ENTER_FRAME,function later(f:Event):void
         {
            if(++frames < 20)
               return;
            removeEventListener(Event.ENTER_FRAME,later);
            trace("SHOT ready");
         });
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
