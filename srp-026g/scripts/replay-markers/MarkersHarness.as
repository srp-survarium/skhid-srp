package
{
   import flash.display.Loader;
   import flash.display.DisplayObject;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;

   // The replay player's one-string lists from the exe (survarium.replay.ReplayText, the root's
   // set_markers_text and set_players_text): the markers bar and the players bar show the good
   // entries (in time order, a label holding ":"), the prev/next label buttons step through them,
   // an empty string empties the bar; the markers are ticks on the seek bar (MarkerTicks) at their
   // place in time, on the track and in the stage, a label over a hovered one (its time, kind and
   // text) in the stage, a press seeking 4 s before it (the slider's VALUE_CHANGE, which the movie
   // sends as update_position); set_replay_speed(percent) shows 0.25x .. 8x in the speed
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
            trace("MARKERS stage " + stage.stageWidth + "x" + stage.stageHeight);
            markers(root);
            ticks(root);
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
         root.set_markers_text("24000:1:0'24;39000:1:0'39;61000:2:Battery;125000:3:;212000:1:3'32;300000:2:Battery;365000:1:6'05;480000:1:Ada > Bob;552000:3:");
         longNames(root);
         root.set_replay_speed(200);
         addEventListener(Event.ENTER_FRAME,function later(f:Event):void
         {
            if(++frames < 20)
               return;
            removeEventListener(Event.ENTER_FRAME,later);
            tickAt(root,7).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
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
         expect("the left list of markers shown",!root.labelsBar.visible);
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

      private function tickAt(root:*, i:int) : Sprite
      {
         return root.markerTicks().getChildByName("srpMarkerTick" + i) as Sprite;
      }

      private function inStage(r:Rectangle) : Boolean
      {
         return r.x >= 0 && r.y >= 0 && r.right <= stage.stageWidth && r.bottom <= stage.stageHeight;
      }

      private function ticks(root:*) : void
      {
         var times:Array = [2000,30000,61000,300000,598000];
         root.set_match_time(600000);
         root.set_markers_text("300000:3:;61000:2:Battery;bad;2000:1:;598000:1:0'00 Ada > Bob;30000:1:0'30");
         var track:Rectangle = root.slider.track.getBounds(stage);
         var xs:Array = [];
         for(var i:int = 0; i < times.length; i++)
         {
            var tick:Sprite = tickAt(root,i);
            if(tick == null)
            {
               failures.push("no tick " + i);
               return;
            }
            var r:Rectangle = tick.getBounds(stage);
            xs.push(tick.localToGlobal(new Point(0,0)).x);
            expect("tick " + i + " leaves the stage " + r,inStage(r));
            expect("tick " + i + " is not on the bar's track " + r + " " + track,xs[i] >= track.x - 2 && xs[i] <= track.right + 2
               && r.y < track.bottom && r.bottom > track.y);
         }
         expect("tick " + times.length + " drawn",tickAt(root,times.length) == null);
         // each at its time: the same pixels a millisecond all along the bar
         var rate:Number = (xs[times.length - 1] - xs[0]) / (times[times.length - 1] - times[0]);
         for(i = 1; i < times.length - 1; i++)
            expect("tick " + i + " at " + int(xs[i]) + ", not " + int(xs[0] + rate * (times[i] - times[0])),
               Math.abs(xs[i] - (xs[0] + rate * (times[i] - times[0]))) <= 1.5);
         expect("the ticks span " + int(xs[times.length - 1] - xs[0]) + " px of the track's " + int(track.width),
            rate > 0 && (xs[times.length - 1] - xs[0]) > track.width * 0.8);
         // over a tick: its label, in the stage
         var words:Array = ["0'02  Kill","0'30  Kill","1'01  Objective  Battery","5'00  Death","9'58  Kill  0'00 Ada > Bob"];
         for(i = 0; i < times.length; i++)
         {
            tickAt(root,i).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
            var label:DisplayObject = root.markerTicks().label;
            var text:String = label == null ? null : Object(label).getChildAt(1).text;
            expect("tick " + i + "'s label is '" + text + "', not '" + words[i] + "'",text == words[i]);
            if(label != null)
               expect("tick " + i + "'s label leaves the stage " + label.getBounds(stage),inStage(label.getBounds(stage)));
            tickAt(root,i).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
            expect("tick " + i + "'s label stays out of it",root.markerTicks().label == null);
         }
         // a press: 4 s before it, as the bar's press (VALUE_CHANGE, sent as update_position)
         var Change:* = loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.events.SliderEvent");
         var seeks:Array = [];
         var heard:Function = function(e:*):void
         {
            seeks.push(e.value);
         };
         root.slider.addEventListener(Change.VALUE_CHANGE,heard);
         tickAt(root,2).dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN));
         tickAt(root,0).dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN));
         root.slider.removeEventListener(Change.VALUE_CHANGE,heard);
         expect("presses sought " + seeks.join(","),seeks.join(",") == "57000,0");
         root.set_match_time(1200000);
         expect("the tick of 5'00 did not move with the match's length",Math.abs(tickAt(root,3).localToGlobal(new Point(0,0)).x - xs[3]) > 10);
         trace("MARKERS ticks: on the bar at their times, labels over them in the stage, a press seeks 4 s before");
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
