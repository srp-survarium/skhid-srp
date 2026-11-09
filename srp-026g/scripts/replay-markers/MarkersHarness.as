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
   import flash.utils.getTimer;

   // The replay player's one-string lists from the exe (survarium.replay.ReplayText, the root's
   // set_markers_text and set_players_text): the markers bar and the players bar show the good
   // entries (in time order, a label holding ":"), the prev/next label buttons step through them,
   // an empty string empties the bar; the markers are ticks on the seek bar (MarkerTicks) at their
   // place in time, on the track and in the stage, a label over a hovered one (its time, kind and
   // text) in the stage, a press seeking 4 s before it (the slider's VALUE_CHANGE, which the movie
   // sends as update_position); set_replay_speed(percent) shows 0.25x .. 8x in the speed
   // stepper (25 to 800, the nearest end outside), kept over the bottom panel's frames; a player's
   // name of 16 characters (Latin, the 10 languages' letters, wide ones) fits its row whole; a
   // viewer's own controls (root.start_viewer, survarium.replay.ViewerControls): Back at the top left
   // and the Moments switch right of the bottom panel, both in the stage and calling the exe when
   // pressed, the switch's choice from set_moments (Mine offered only to a viewer who played) fading
   // the ticks it does not light, and the key hint (survarium.replay.KeyHint: key caps and actions on
   // a panel) centred above the bar, fading in for its first 10 s and while the cursor is
   // (set_cursor_shown), in the stage in every language (two rows at most, nothing clipped).
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
            viewer(root);
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
               viewerPlaced(root);
            }
            catch(err:Error)
            {
               failures.push("the player: " + err.getStackTrace());
            }
            hintTimes(root);
         });
      }

      // start_viewer's texts in the 10 languages (the dictionaries' st_srp_viewer_*): Back, Moments,
      // All, Mine, Followed, Cursor, Pause, Filter, Hide, the Ctrl and Space caps.
      private static const TEXTS:Array = [
         ["Back","Moments","All","Mine","Followed","Cursor","Pause","Filter","Hide","Ctrl","Space"],
         ["Назад","Моменти","Усі","Мої","Відстежуваний","Курсор","Пауза","Фільтр","Сховати","Ctrl","Space"],
         ["Назад","Моменты","Все","Мои","Отслеживаемый","Курсор","Пауза","Фильтр","Скрыть","Ctrl","Space"],
         ["Назад","Моманты","Усе","Мае","Адсочваны","Курсор","Паўза","Фільтр","Схаваць","Ctrl","Space"],
         ["Zurück","Momente","Alle","Meine","Verfolgt","Cursor","Pause","Filter","Ausblenden","Strg","Leertaste"],
         ["Wstecz","Momenty","Wszystkie","Moje","Śledzony","Kursor","Pauza","Filtr","Ukryj","Ctrl","Spacja"],
         ["Nazad","Momenti","Svi","Moji","Praćeni","Kursor","Pauza","Filter","Sakrij","Ctrl","Space"],
         ["Zpět","Momenty","Vše","Moje","Sledovaný","Kurzor","Pauza","Filtr","Skrýt","Ctrl","Space"],
         ["Vissza","Pillanatok","Mind","Saját","Követett","Kurzor","Szünet","Szűrő","Elrejtés","Ctrl","Space"],
         ["Takaisin","Hetket","Kaikki","Omat","Seurattu","Kursori","Tauko","Suodatin","Piilota","Ctrl","Space"]];

      private static function texts(i:int, first:String = null) : Array
      {
         var out:Array = TEXTS[i].concat();
         if(first != null)
            out[0] = first;
         return out;
      }

      private static const MARKER_NAMES:String = "0:Me;1:Bob;2:Ada;3:Eve;4:Battery";

      private static const LIT:Number = 1;

      private static const FADED:Number = 0.3;

      // The ticks' alphas, in their order.
      private function alphas(root:*) : String
      {
         var out:Array = [];
         for(var i:int = 0; tickAt(root,i) != null; i++)
            out.push(Math.round(tickAt(root,i).alpha * 10) / 10);
         return out.join(",");
      }

      // What a press on `target` calls the exe with (ViewerControls.lastCall: Ruffle has no exe).
      private function calls(v:*, target:Sprite) : String
      {
         v.lastCall = "";
         target.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN));
         return v.lastCall;
      }

      private function viewer(root:*) : void
      {
         root.set_match_time(600000);
         // a kill of the viewer's, the followed player's, a death of the followed one that the viewer
         // made, an objective of no one's
         root.set_marker_names(MARKER_NAMES);
         root.set_markers_text("30000:1:1:0:1;60000:1:2:2:3;90000:3:3:0:3;120000:2:0:4:;bad:1:1::;7000:1:x::");
         expect("before start_viewer, no viewer's controls",root.viewer == null);
         expect("the markers with whose are " + labels(root).join(","),labels(root).join(",") == "30000=Me > Bob,60000=Ada > Eve,90000=Me > Eve,120000=Battery");
         root.start_viewer.apply(null,texts(0));
         root.start_viewer.apply(null,texts(0,"Twice"));
         var v:* = root.viewer;
         expect("start_viewer made no controls",v != null);
         if(v == null)
            return;
         expect("a second start_viewer made the controls again",Object(v.back.getChildAt(1)).text == "<  Back");
         root.set_moments(1,1);
         expect("Mine fades " + alphas(root),alphas(root) == [LIT,FADED,LIT,FADED].join(","));
         expect("Mine is not shown as the choice",v.choice == 1 && v.choices[1].visible && v.choices[1].getChildAt(0).alpha == 1 && v.choices[0].getChildAt(0).alpha < 1);
         root.set_moments(2,1);
         expect("Followed fades " + alphas(root),alphas(root) == [FADED,LIT,LIT,FADED].join(","));
         root.set_marker_names(MARKER_NAMES);
         root.set_markers_text("30000:1:1:0:1;60000:1:2:2:3;90000:3:3:0:3;120000:2:0:4:");
         expect("new markers under Followed fade " + alphas(root),alphas(root) == [FADED,LIT,LIT,FADED].join(","));
         root.set_moments(0,0);
         expect("All fades " + alphas(root),alphas(root) == [LIT,LIT,LIT,LIT].join(","));
         expect("Mine offered to a viewer who did not play",!v.choices[1].visible);
         expect("Followed not next to All with Mine hidden",Math.abs(v.choices[2].x - (v.choices[0].x + v.choices[0].width + 3)) < 1);
         expect("a press on Back calls '" + calls(v,v.back) + "'",calls(v,v.back) == "back_clicked");
         expect("a press on Followed calls '" + calls(v,v.choices[2]) + "'",calls(v,v.choices[2]) == "moments_clicked 2");
         expect("a press on All calls '" + calls(v,v.choices[0]) + "'",calls(v,v.choices[0]) == "moments_clicked 0");
         expect("the hint takes the mouse",!v.hint.mouseEnabled);
         root.set_moments(1,1);
         trace("MARKERS viewer: start_viewer once, set_moments fades the ticks it does not light, Back and the switch call the exe");
      }

      private function viewerPlaced(root:*) : void
      {
         var v:* = root.viewer;
         if(v == null)
            return;
         var back:Rectangle = v.back.getBounds(stage);
         var hint:Rectangle = v.hint.getBounds(stage);
         var moments:Rectangle = v.moments.getBounds(stage);
         var panel:Rectangle = root.bottomPanel.getBounds(stage);
         expect("Back is not at the top left, right of the minimap " + back,inStage(back) && back.x > stage.stageWidth * 0.15 && back.x < stage.stageWidth * 0.25 && back.y < 40);
         expect("the hint leaves the stage " + hint,inStage(hint));
         expect("the hint is not above the bar " + hint + " " + panel,hint.bottom < panel.y - 20 && hint.bottom > panel.y - 80);
         expect("the hint is not centred " + hint,Math.abs(hint.x + hint.width / 2 - stage.stageWidth / 2) < 3);
         expect("the hint has " + v.hint.groups.length + " keys",v.hint.groups.length == 6);
         expect("the switch leaves the stage " + moments,inStage(moments));
         expect("the switch is on the bottom panel " + moments + " " + panel,moments.x >= panel.right && moments.y < panel.bottom && moments.bottom > panel.y);
         expect("the hint is not shown at the start",v.hintShown);
         // every language's hint in this stage: two rows at most, each action whole, in the stage
         var Hint:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("survarium.replay.KeyHint") as Class;
         var font:String = root.freeViewButton.textField.getTextFormat().font;
         var room:Number = stage.stageWidth / root.scaleX - 20;
         for(var i:int = 0; i < TEXTS.length; i++)
         {
            var t:Array = TEXTS[i];
            var h:* = new Hint(font,[t[5],t[6],t[1],t[7],t[8],t[0],t[9],t[10]]);
            root.addChild(h);
            h.layout(room);
            var clipped:Array = [];
            for each(var g:* in h.groups)
            {
               var action:* = g.getChildByName("action");
               if(action.textWidth > action.width || action.maxScrollH > 0)
                  clipped.push(action.text);
            }
            expect(t[0] + "'s hint has " + h.rows + " rows, " + int(h.width) + " of " + int(room) + " units, clipped " + clipped.join(","),
               h.rows <= 2 && h.width <= room && clipped.length == 0);
            trace("MARKERS hint " + t[0] + ": " + h.rows + " row(s), " + int(h.width) + " of " + int(room));
            h.layout(360);
            expect(t[0] + "'s hint in 360 units has " + h.rows + " rows, " + int(h.width) + " wide",h.rows >= 2 && h.width <= 360);
            root.removeChild(h);
         }
         trace("MARKERS viewer: Back at the top left, the hint above the bar, the switch by the bottom panel, in the stage");
      }

      // The hint: hidden 10 s after start_viewer, shown again with the cursor.
      private function hintTimes(root:*) : void
      {
         var v:* = root.viewer;
         if(v == null)
         {
            finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
            return;
         }
         // at ms after the first: the check
         var steps:Array = [
            [500,function():void { expect("the hint faded in to " + v.hint.alpha,v.hint.visible && v.hint.alpha == 1); }],
            [10600,function():void
            {
               expect("the hint still shown after 10 s",!v.hint.visible && v.hint.alpha == 0);
               root.set_cursor_shown(true);
               expect("the hint not to be shown with the cursor",v.hintShown);
               expect("the hint cut in, not faded (" + v.hint.alpha + ")",v.hint.alpha == 0);
            }],
            [11100,function():void
            {
               expect("the hint not shown with the cursor",v.hint.visible && v.hint.alpha == 1);
               root.set_cursor_shown(false);
            }],
            [11600,function():void { expect("the hint shown with the cursor hidden",!v.hint.visible); }]];
         var from:int = getTimer();
         addEventListener(Event.ENTER_FRAME,function waited(f:Event):void
         {
            while(steps.length > 0 && getTimer() - from >= steps[0][0])
               steps.shift()[1]();
            if(steps.length > 0)
               return;
            removeEventListener(Event.ENTER_FRAME,waited);
            trace("MARKERS viewer: the hint fades in its first 10 s and with the cursor, out after");
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
         root.set_marker_names("0:0'24;1:0'39;2:Battery;3:3'32;4:6'05;5:Ada;6:Bob");
         root.set_markers_text("24000:1:0:0:;39000:1:1:1:;61000:2:0:2:;125000:3:3::;212000:1:1:3:;300000:2:0:2:;365000:1:0:4:;480000:1:2:5:6;552000:3:2::");
         root.start_viewer.apply(null,texts(0));
         root.set_moments(1,1);
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
         root.set_marker_names("7:2:05;9:Battery");
         root.set_markers_text("125000:1:0:7:;bad;30000:1:0::;9x:1:0::;61000:2:0:9:;5000:1:0:x:;40000:1:9;");
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
         root.set_marker_names("3:Battery;4:0'00 Ada > Bob;5:0'30");
         root.set_markers_text("300000:3:0::;61000:2:0:3:;bad;2000:1:0::;598000:1:0:4:;30000:1:0:5:");
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
