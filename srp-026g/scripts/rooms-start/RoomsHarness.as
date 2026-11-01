package
{
   import flash.display.DisplayObject;
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.DisplayObjectContainer;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.text.TextField;
   import flash.utils.getQualifiedClassName;

   // A private room's Start and teams (docs/notes/rooms.md, the owner's of 2026-10-26) with the
   // lobby movie's own rooms window (survarium.rooms.RoomsWindow over the movie's bottom bar), told
   // room states as the server's answers (`S1;R`, lobby.md 2.16.3) through RoomsWindow.take:
   // - the owner of a private room waiting: Start, "Press Start when everyone is in.", a move
   //   button on every member (itself too: » on team 1, « on team 2), the two teams in two columns;
   // - a member of it: no Start, no move, "Waiting for the creator to start.";
   // - the owner of an open room: no Start, no move (its countdown starts it, its sides drawn);
   // - the owner of a private room playing its match: no Start, no move;
   // - a move told (the next state's `A;<sides>`): the member in the other column;
   // - a private room's info line has no "N v N" and no max level, an open room's has both;
   // - the list (`S1;L`): a private room's row has no max level, nor its facts beside the list;
   //   an open room's has both;
   // - the create window (New room): Team size and Max level shown; a PIN typed hides both, and
   //   clearing it shows them again.
   // - squads (docs/notes/squads.md, the owner's of 2026-10-31): a pair both in the room shows
   //   the match's squad icon (its frame by the pair's number) after each name, ours green and
   //   another's tan, in the room's view and the list's side panel; every name keeps its place
   //   with and without the icons; every language's texts fit with them (checkFit);
   // - a squad's soldier (in a squad, not its leader): the list's Join and New room greyed, its
   //   commander's not (it goes into no room on its own; the server refuses it too); the top bar's
   //   Play says "IN SQUAD" for the soldier, whole in every language, and PLAY for its leader.
   // Prints "ROOMS ..." lines (trace) and ends with "ROOMS RESULT pass" or "ROOMS RESULT fail: ...".
   public class RoomsHarness extends Sprite
   {
      private static const STATE:int = 2060000001;

      private static const WHO:Array = ["Alice","Bob","Carol","Dave","Erin"];

      private var loader:Loader = new Loader();

      private var Win:*;

      private var win:Sprite;

      private var failures:Array = [];

      public function RoomsHarness()
      {
         var movie:String = loaderInfo.parameters.movie || "lobby_menu.swf";
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         // the lobby's own root fails outside the game (no exe): only its classes are used
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
         });
         loader.load(new URLRequest(movie),new LoaderContext(false,new ApplicationDomain()));
      }

      private function loaded(e:Event) : void
      {
         // The native placement adapter also initializes more of the lobby's
         // root. Keep that fixture separate from the lightweight rooms tests.
         if(loaderInfo.parameters.players == "1")
         {
            checkPlayerMenus();
            finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
            return;
         }
         var step:String = "the classes";
         try
         {
            var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
            // The English texts, as the exe gives them (set_localization_data).
            step = "the texts";
            domain.getDefinition("survarium.data.LocalizationData").setData(HarnessText.DATA.english);
            Win = domain.getDefinition("survarium.rooms.RoomsWindow");
            // The bottom bar's symbol (its class survarium.controls.BottomPanel), as the
            // lobby has it: the window sits over it.
            step = "the bottom panel";
            var panel:* = new (domain.getDefinition("SrvBottomPanel") as Class)();
            panel.y = 700;
            step = "the rooms window";
            win = new Win(panel);
            Win.me = win;
            addChild(win);
         }
         catch(err:Error)
         {
            finish("fail: " + step + ": " + err);
            return;
         }
         // 5 members; the asker the first (the owner) or the second.
         var seen:Object = check("private, owner, waiting", 1001, true, true, 0, "01010", true, 5,
            "Press Start when everyone is in.", false);
         if(seen != null && seen.alice != 0)
         {
            failures.push("Alice not on team 1 before the move");
         }
         // The owner moved Alice (herself) to team 2: the server's next state.
         seen = check("private, owner, Alice moved", 1001, true, true, 0, "11010", true, 5, null, false);
         if(seen != null && seen.alice != 1)
         {
            failures.push("Alice not on team 2 after the move");
         }
         check("private, member, waiting", 1002, true, false, 0, "01010", false, 0,
            "Waiting for the creator to start.", false);
         check("open, owner, waiting", 1003, false, true, 0, null, false, 0, "Waiting for players", true);
         check("private, owner, in match", 1004, true, true, 2, "01010", false, 0, null, false);
         check("open, waiting for next match after player limit", 1005, false, false, 2, null, false, 0,
            "This match has reached its player limit. You'll join the next match.", true, 7);
         // Out of the room: the list. A private room of max level 0 (the server's for it) and an
         // open room of max level 5; the private one chosen (the first). Then an open room alone
         // under the same key, chosen too.
         tell(["S1;N;0;0;0"]);
         checkList("list, private chosen", ["S1;L;2;0","1001;2;0;10;3;0;0;0;1;Alice","1002;2;5;6;1;0;0;0;0;Bob"],
            "PIN · Alice", false, "Bob", true, false);
         checkList("list, open chosen", ["S1;L;1;0","1001;2;5;6;1;0;0;0;0;Bob"], null, false, "Bob", true, true);
         checkSideStable();
         checkRoomView();
         checkQueueButton();
         checkPlayOffer();
         checkCreate();
         checkWindowOrder();
         checkSquads();
         checkSoldier();
         checkInSquad();
         checkFit();
         finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
      }

      // Ruffle draws the movie's embedded Calibri, yet not exactly as Scaleform lays it out: every
      // text must fit with this much of its width to spare.
      private static const FIT_MARGIN:Number = 0.05;

      // Every language (the dictionaries' texts, as the exe gives them): the list, a room in each
      // of its phases and hints, and the create window, each text on its line where it is drawn,
      // none running into another on the same line and none cut at its field's end. Prints a
      // "ROOMS OVERLAP" line for each miss.
      private function checkFit() : void
      {
         var Loc:Object = loader.contentLoaderInfo.applicationDomain.getDefinition("survarium.data.LocalizationData");
         var misses:Array = [];
         for each(var language:String in HarnessText.LANGUAGES)
         {
            var say:Object = HarnessText.DATA[language];
            Loc.setData(say);
            // The pool the cards and rows name: long map names, the longest weather, every mode.
            tell(["S1;M;3;0","0;61;5;5;2;0","1;62;13;5;3;1","2;63;12;3;6;0"]);
            var states:Array = [
               ["private, owner", 1001, true, true, 0, "01010", 0, []],
               ["private, member", 1002, true, false, 0, "01010", 0, []],
               ["open, waiting", 1003, false, false, 0, null, 0, []],
               ["open, vote", 1003, false, false, 3, null, 0, []],
               ["open, countdown", 1003, false, false, 1, null, 0, []],
               ["open, in match", 1003, false, false, 2, null, 0, []],
               ["open, joining in", 1003, false, false, 2, null, 0, ["J;25"]],
               ["open, kit unfit", 1003, false, false, 0, null, 1, []],
               ["open, gear unfit", 1003, false, false, 0, null, 2, []],
               ["open, squads", 1003, false, false, 0, null, 0, ["S;01122"]],
               ["open, squads, in match", 1003, false, false, 2, null, 0, ["S;01122"]],
               ["private, owner, squads", 1001, true, true, 0, "01010", 0, ["S;11220"]]];
            for(var why:int = 1; why <= 7; why++)
               states.push(["open, in match, why " + why, 1003, false, false, 2, null, 0, ["J;-1;" + why]]);
            for each(var s:Array in states)
            {
               tellRoom(s[1], s[2], s[3], s[4], s[5], s[6], s[7]);
               misses = misses.concat(overlaps(language + ", " + s[0]));
            }
            // Out of the room: the list (a room in its match, any max level; one voting; a private one).
            tell(["S1;N;0;0;0"]);
            tell(["S1;L;3;0","1001;15;0;10;20;2;754;61;0;Bob","1002;15;15;10;3;3;12;62;0;Carol","1003;10;0;10;5;0;0;63;1;Erin"]);
            misses = misses.concat(overlaps(language + ", list"));
            var buttons:Array = [];
            walk(win,[],[],0,0,buttons);
            for each(var b:* in buttons)
               if(b.label == say.st_srp_rooms_new)
               {
                  var ButtonEvent:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
                  b.dispatchEvent(new ButtonEvent(ButtonEvent["CLICK"]));
                  misses = misses.concat(overlaps(language + ", create"));
               }
         }
         Loc.setData(HarnessText.DATA.english);
         for each(var miss:String in misses)
            trace("ROOMS OVERLAP " + miss);
         trace("ROOMS every language's texts fit their places: " + (misses.length == 0 ? "yes" : misses.length + " misses"));
         if(misses.length != 0)
            failures.push(misses.length + " texts overlap or are cut (ROOMS OVERLAP lines)");
      }

      // The window told room `key`'s state: its cards, five members (the asker the first when it
      // is the owner, else the second, its kit or gear unfit by `fit`), the teams and `extra` rows.
      private function tellRoom(key:int, pin:Boolean, owner:Boolean, phase:int, teams:String, fit:int, extra:Array) : void
      {
         var size:int = 10;
         var names:Array = [["S1","R",key,15,pin ? 0 : 15,size,30,phase,754,owner ? 1 : 0,5,5,key,pin ? 1 : 0,63,pin ? 1 : 2,0].join(";"),
            "C;0;61;2;1","C;1;62;1;0","C;2;63;0;0"];
         for(var i:int = 0; i < WHO.length; i++)
         {
            var me:Boolean = owner ? i == 0 : i == 1;
            names.push(["P",i,15,i == 0 ? 1 : 0,me ? 1 : 0,phase == 2 && !me ? 1 : 0,me ? fit : 0,0,WHO[i]].join(";"));
         }
         if(teams != null)
            names.push("A;" + teams);
         names.push("L;0");
         var rows:Array = [];
         for(var r:int = 0; r < names.concat(extra).length; r++)
            rows.push({"place":STATE + r, "nickname":names.concat(extra)[r]});
         try
         {
            Win.take(rows);
         }
         catch(err:Error)
         {
            failures.push(key + ": " + err.message);
         }
      }

      // The squad icons in the window (RoomsLook.squadIcon: the hud's squad_icon frames, the
      // classes SquadIcon1..5): each one's frame, where it is and its tint.
      private function squadTags(texts:Array) : Array
      {
         var tags:Array = [];
         findIcons(win, 0, 0, tags);
         return tags;
      }

      private static function findIcons(at:DisplayObject, x:Number, y:Number, tags:Array) : void
      {
         if(!at.visible)
            return;
         var icon:Bitmap = at as Bitmap;
         if(icon != null && icon.bitmapData != null)
         {
            var kind:String = getQualifiedClassName(icon.bitmapData);
            if(kind.indexOf("SquadIcon") == 0)
               tags.push({"text":kind.substr(9), "x":x + at.x, "y":y + at.y, "colour":icon.transform.colorTransform.color,
                  "w":icon.width});
            return;
         }
         var box:DisplayObjectContainer = at as DisplayObjectContainer;
         if(box == null)
            return;
         for(var i:int = 0; i < box.numChildren; i++)
            findIcons(box.getChildAt(i), x + at.x, y + at.y, tags);
      }

      // Where each of WHO's names is drawn (its field's place, and its text's right end).
      private static function namePlaces(texts:Array) : Object
      {
         var at:Object = {};
         for each(var t:Object in texts)
         {
            // The owner's name with its tag: "Alice  (owner)".
            var name:String = String(t.text).split("  (")[0];
            if(WHO.indexOf(name) >= 0)
               at[name] = {"x":t.x, "y":t.y, "right":t.x + 2 + t.field.textWidth};
         }
         return at;
      }

      // The tags `tags` against the names: `want` gives each name's tag ("" none) and whether it
      // is ours (green, 0xB3EA97) or another's (tan, 0x9D7E57); each right of its name, on its
      // line; and every name where `plain` had it.
      private function checkTags(what:String, texts:Array, plain:Object, want:Object) : void
      {
         var names:Object = namePlaces(texts);
         var tags:Array = squadTags(texts);
         var said:Array = [];
         for(var who:String in want)
         {
            var name:Object = names[who];
            if(name == null || plain[who] == null)
            {
               failures.push(what + ": " + who + " not shown");
               continue;
            }
            if(name.x != plain[who].x || name.y != plain[who].y)
               failures.push(what + ": " + who + " moved with the squad icons");
            var tag:Object = null;
            for each(var t:Object in tags)
               if(t.x >= name.right - 1 && t.x <= name.right + 30 && Math.abs(t.y - name.y) < 12)
                  tag = t;
            var expected:Array = want[who];
            said.push(who + " " + (tag != null ? tag.text + "/" + tag.colour.toString(16) : "-"));
            if(expected[0] == "")
            {
               if(tag != null)
                  failures.push(what + ": " + who + " has a squad icon");
               continue;
            }
            if(tag == null || tag.text != expected[0])
            {
               failures.push(what + ": " + who + "'s squad icon " + expected[0] + " missing");
               continue;
            }
            var green:Boolean = (tag.colour >> 8 & 0xFF) > (tag.colour >> 16 & 0xFF);
            if(green != expected[1])
               failures.push(what + ": " + who + "'s tag is not " + (expected[1] ? "ours (green)" : "another's (tan)"));
         }
         trace("ROOMS " + what + ": " + said.join(", "));
      }

      private function checkSquads() : void
      {
         tell(["S1;M;3;0","0;61;5;5;2;0","1;62;13;5;3;1","2;63;12;3;6;0"]);
         // The room's view: Bob (the asker) and Carol a pair, Dave and Erin another.
         var want:Object = {"Alice":["",false], "Bob":["1",true], "Carol":["1",true], "Dave":["2",false], "Erin":["2",false]};
         var texts:Array = [];
         tellRoom(1003, false, false, 0, null, 0, []);
         walk(win,[],texts,0,0,[]);
         var plain:Object = namePlaces(texts);
         texts = [];
         tellRoom(1003, false, false, 0, null, 0, ["S;01122"]);
         walk(win,[],texts,0,0,[]);
         checkTags("squads in the room", texts, plain, want);
         // A private room's two teams, its owner Alice: Alice and Bob a pair on team 1.
         texts = [];
         tellRoom(1001, true, true, 0, "00110", 0, []);
         walk(win,[],texts,0,0,[]);
         plain = namePlaces(texts);
         texts = [];
         tellRoom(1001, true, true, 0, "00110", 0, ["S;11220"]);
         walk(win,[],texts,0,0,[]);
         checkTags("squads in a private room", texts, plain,
            {"Alice":["1",true], "Bob":["1",true], "Carol":["2",false], "Dave":["2",false], "Erin":["",false]});
         // The list's side panel, its room chosen: the members' list (`S1;G`) with the marks.
         tell(["S1;N;0;0;0"]);
         tell(["S1;L;1;0","1003;2;15;8;5;0;0;0;0;Alice"]);
         var rows:Array = [];
         walk(win,[],[],0,0,[]);
         findRows(win, rows);
         for each(var row:Sprite in rows)
            if(row.name == "1003")
               row.dispatchEvent(new MouseEvent(MouseEvent.CLICK));
         var members:Function = function(marks:Array):Array
         {
            var names:Array = ["S1;G;1003;5"];
            for(var i:int = 0; i < WHO.length; i++)
            {
               names.push(["P",i,15,i == 0 ? 1 : 0,i == 1 ? 1 : 0,0,0,0,0,11 + i,marks[i]].join(";"));
               names.push(WHO[i]);
            }
            tell(names);
            var seen:Array = [];
            walk(win,[],seen,0,0,[]);
            return seen;
         };
         plain = namePlaces(members([0,0,0,0,0]));
         checkTags("squads in the list's side panel", members([0,1,1,2,2]), plain, want);
      }

      // A squad's soldier goes into no room on its own (squads.md): Join and New room greyed in
      // the list; its commander's are not.
      private function checkSoldier() : void
      {
         var Info:Object = loader.contentLoaderInfo.applicationDomain.getDefinition("survarium.controls.PlayerInfo");
         var lit:Function = function(soldier:Boolean):Array
         {
            Info.isInSquad = true;
            Info.commander = !soldier;
            tell(["S1;N;0;0;0"]);
            tell(["S1;L;1;0","1003;2;15;8;5;0;0;0;0;Alice"]);
            var buttons:Array = [];
            walk(win,[],[],0,0,buttons);
            var join:* = null;
            var make:* = null;
            for each(var b:* in buttons)
            {
               if(b.label == "Join")
                  join = b;
               if(b.label == HarnessText.DATA.english.st_srp_rooms_new)
                  make = b;
            }
            return [join != null && join.enabled, make != null && make.enabled, join != null && make != null];
         };
         var soldier:Array = lit(true);
         var leader:Array = lit(false);
         Info.isInSquad = false;
         Info.commander = true;
         tell(["S1;N;0;0;0"]);
         trace("ROOMS a soldier's Join " + (soldier[0] ? "lit" : "greyed") + ", New room " + (soldier[1] ? "lit" : "greyed")
            + "; its commander's Join " + (leader[0] ? "lit" : "greyed") + ", New room " + (leader[1] ? "lit" : "greyed"));
         if(!soldier[2] || !leader[2])
            failures.push("soldier: the list's Join or New room not found");
         else if(soldier[0] || soldier[1] || !leader[0] || !leader[1])
            failures.push("soldier: Join and New room must be greyed for a soldier only");
      }

      // The top bar's Play (the lobby's StatusPanel, its symbol SrvTopMenuContainer): a soldier's
      // greyed Play says "In squad" (in capitals, as the game's PLAY), whole in every language;
      // its leader's is the game's own PLAY.
      private function checkInSquad() : void
      {
         var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
         var Info:Object = domain.getDefinition("survarium.controls.PlayerInfo");
         var Loc:Object = domain.getDefinition("survarium.data.LocalizationData");
         var panel:*;
         try
         {
            panel = new (domain.getDefinition("SrvTopMenuContainer") as Class)();
         }
         catch(err:Error)
         {
            failures.push("in squad: the top bar: " + err);
            return;
         }
         var btn:* = panel.playBtn;
         btn.label = "PLAY";
         btn.validateNow();
         var said:Array = [];
         for each(var language:String in HarnessText.LANGUAGES)
         {
            var say:Object = HarnessText.DATA[language];
            Loc.setData(say);
            Info.isInSquad = true;
            Info.commander = false;
            panel.showRoomsPlay(false,false,false,false,false,"");
            btn.validateNow();
            var want:String = String(say.st_srp_play_in_squad).toUpperCase();
            var field:TextField = btn.textField as TextField;
            said.push(language + " " + btn.label + (field != null ? " " + Math.round(field.textWidth) + "/" + Math.round(field.width) : ""));
            if(btn.label != want)
               failures.push("in squad, " + language + ": Play says " + btn.label + ", not " + want);
            else if(field == null || field.textWidth * (1 + FIT_MARGIN) > field.width - 4)
               failures.push("in squad, " + language + ": \"" + btn.label + "\" does not fit Play");
            if(btn.enabled)
               failures.push("in squad, " + language + ": Play lit for a soldier");
            Info.commander = true;
            panel.showRoomsPlay(false,false,false,false,true,"");
            if(btn.label != "PLAY")
               failures.push("in squad, " + language + ": the leader's Play says " + btn.label);
         }
         Info.isInSquad = false;
         Loc.setData(HarnessText.DATA.english);
         trace("ROOMS a soldier's Play: " + said.join(", "));
      }

      // The list's rows (their sprites are named by their rooms' keys).
      private static function findRows(at:DisplayObject, rows:Array) : void
      {
         var box:DisplayObjectContainer = at as DisplayObjectContainer;
         if(box == null)
            return;
         if(box is Sprite && Sprite(box).buttonMode && /^\d+$/.test(box.name))
            rows.push(box);
         for(var i:int = 0; i < box.numChildren; i++)
            findRows(box.getChildAt(i), rows);
      }

      // What `what` shows: every shown text's drawn extent (its alignment in its field, the margin
      // added), a single-line one cut at its field's end, and any two on one line that meet.
      private function overlaps(what:String) : Array
      {
         var texts:Array = [];
         walk(win,[],texts,0,0,[]);
         var misses:Array = [];
         var spans:Array = [];
         for each(var t:Object in texts)
         {
            var f:TextField = t.field as TextField;
            if(f.text == "" || f.type == "input")
               continue;
            var w:Number = f.textWidth * (1 + FIT_MARGIN);
            // Cut by its field, or to it (RoomsLook.fitName: its dots; the harness's names are short).
            if(!f.wordWrap && (w > f.width - 4 || f.text.charAt(f.text.length - 1) == "…"))
               misses.push(what + ": \"" + f.text + "\" cut: " + Math.round(f.textWidth) + " in " + Math.round(f.width - 4));
            // What shows of it: a field clips its line (a cut one is a miss above).
            w = Math.min(w, f.width - 4);
            var align:String = f.getTextFormat().align;
            var left:Number = t.x + 2;
            if(align == "right")
               left = t.x + f.width - 2 - w;
            else if(align == "center")
               left = t.x + (f.width - w) / 2;
            spans.push({"text":f.text, "left":left, "right":left + w, "top":t.y + 2, "bottom":t.y + 2 + f.textHeight});
         }
         for(var i:int = 0; i < spans.length; i++)
            for(var j:int = i + 1; j < spans.length; j++)
            {
               var a:Object = spans[i];
               var c:Object = spans[j];
               var high:Number = Math.min(a.bottom, c.bottom) - Math.max(a.top, c.top);
               if(high > 0.3 * Math.min(a.bottom - a.top, c.bottom - c.top) && a.left < c.right + 2 && c.left < a.right + 2)
                  misses.push(what + ": \"" + a.text + "\" meets \"" + c.text + "\"");
            }
         return misses;
      }

      // The selected room's footer owns its queue action; there is no
      // duplicate cancellation button above the list.
      private function checkQueueButton():void
      {
         tell(["S1;N;0;0;0"]);
         tell(["S1;L;1;0", "1001;2;15;8;16;0;0;0;0;QueueDemo01"]);
         queueButton("before queue", "Queue", false);
         tell(["S1;N;0;1001;1"]);
         queueButton("queued", "Leave queue", true);
         // The selected room may show a free seat before admission runs.
         tell(["S1;L;1;0", "1001;2;15;8;15;0;0;0;0;QueueDemo01"]);
         queueButton("queued while a place frees", "Leave queue", true);
         tell(["S1;L;1;0", "1001;2;15;8;16;0;0;0;0;QueueDemo01"]);
         tell(["S1;N;0;0;0"]);
         queueButton("queue cancelled", "Queue", false);
      }

      // Play's preset offer (presets.md, the owner's of 2026-10-30): Play that would queue is
      // offered a room of the other bracket as a Join of it is (the same popup and text), its
      // first button "Queue" (Play as it is) beside "Switch and enter"; Play into the room it
      // takes keeps "Enter anyway". The first button closes the popup.
      private function checkPlayOffer():void
      {
         playOffer("Play would queue, gear over a lower room", "S1;O;1002;2;2;10;15;10;1", "Queue",
            "Your gear 15 is over the room's max level 10. Loadout 2 is level 10.");
         playOffer("Play would queue, a higher room", "S1;O;1003;1;2;14;5;15;1", "Queue",
            "Room max level 15, your gear 5. Loadout 2 is level 14.");
         playOffer("Play into its room", "S1;O;1002;1;2;8;5;10;0", "Enter anyway",
            "Room max level 10, your gear 5. Loadout 2 is level 8.");
         // A full room's Queue (the owner's of 2026-11-01): Join's offer, its first button "Queue"
         // (the queue as it is) for a higher loadout, "Exit" for gear over the max.
         playOffer("Queue of a full room, a higher loadout", "S1;O;1001;1;2;15;10;15;0", "Queue",
            "Room max level 15, your gear 10. Loadout 2 is level 15.", "1001;2;15;8;16;0;0;0;0;QueueDemo01");
         playOffer("Queue of a full room, gear over its max", "S1;O;1001;2;2;10;15;10;0", "Exit",
            "Your gear 15 is over the room's max level 10. Loadout 2 is level 10.", "1001;2;10;8;16;0;0;0;0;QueueDemo01");
      }

      // Play pressed, or (`full`, a full room's list row) that room's Queue in the list; then the
      // server's offer `row`.
      private function playOffer(what:String, row:String, first:String, why:String, full:String = null):void
      {
         tell(["S1;N;0;0;0"]);
         try
         {
            if(full == null)
               win["quickPlay"]();
            else
               pressQueue(full);
         }
         catch(err:Error)
         {
            failures.push(what + ": " + (full == null ? "Play" : "Queue") + ": " + err.message);
            return;
         }
         tell([row]);
         var buttons:Array = [];
         var texts:Array = [];
         walk(win,[],texts,0,0,buttons);
         var sw:* = null;
         var left:* = null;
         for each(var b:* in buttons)
            if(b.label == "Switch and enter")
               sw = b;
         for each(b in buttons)
            if(sw != null && b.label == first && b.y == sw.y && b.x < sw.x)
               left = b;
         var said:Boolean = texts.some(function(t:*, ... rest):Boolean
         {
            return t.text == why;
         });
         trace("ROOMS " + what + ": " + (left != null ? left.label : "no " + first) + " | "
            + (sw != null ? sw.label : "no Switch and enter") + (said ? "" : " (text missing)"));
         if(sw == null || left == null)
         {
            failures.push(what + ": expected " + first + " beside Switch and enter");
            return;
         }
         if(!said)
            failures.push(what + ": the popup does not say '" + why + "'");
         var ButtonEvent:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
         left.dispatchEvent(new ButtonEvent(ButtonEvent["CLICK"]));
         buttons = [];
         walk(win,[],[],0,0,buttons);
         for each(b in buttons)
            if(b.label == "Switch and enter")
               failures.push(what + ": the popup stays after " + first);
      }

      // The list of the one full room `full`, its Queue pressed.
      private function pressQueue(full:String):void
      {
         tell(["S1;L;1;0", full]);
         var buttons:Array = [];
         walk(win,[],[],0,0,buttons);
         for each(var b:* in buttons)
            if(b.label == "Queue")
            {
               var ButtonEvent:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
               b.dispatchEvent(new ButtonEvent(ButtonEvent["CLICK"]));
               return;
            }
         throw new Error("no Queue button");
      }

      private function queueButton(what:String, expected:String, enabled:Boolean):void
      {
         var buttons:Array = [];
         walk(win,[],[],0,0,buttons);
         var actions:Array = buttons.filter(function(b:*, ... rest):Boolean
         {
            return b.label == "Join" || b.label == "Queue" || b.label == "Leave queue";
         });
         if(actions.length != 1 || actions[0].label != expected)
            failures.push(what + ": expected one " + expected + " button");
         else if(enabled && !actions[0].enabled)
            failures.push(what + ": leaving the queue is disabled");
         else if(actions[0].y < 100)
            failures.push(what + ": queue action is above the room list");
         trace("ROOMS " + what + ": " + actions.length + " action(s), "
            + (actions.length ? actions[0].label : "missing"));
      }

      private function checkPlayerMenus():void
      {
         try
         {
            var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
            var Lobby:Object = domain.getDefinition("LobbyMenu");
            var Menu:Object = domain.getDefinition("survarium.controls.SrvContextMenu");
            var Player:Object = domain.getDefinition("survarium.controls.PlayerInfo");
            var Friends:Object = domain.getDefinition("survarium.friends.FriendsMenu");
            var friends:Object = new (domain.getDefinition("SrvFriendsMenuReskin") as Class)();
            Menu.registerStage(stage,this);
            Menu.setPatternt(new (domain.getDefinition("ContextPattern") as Class)());
            Player.nickname = "Alice";
            Player.commander = true;
            Player.isSquadFull = false;
            friends.setPlayers([]);
            friends.setPlayers([],false);
            // The menu's names, from the second entry on: the friend action, the two mute entries that
            // change Bob's state (the server's ignore list: every muted player, a <muted> one's id with
            // bit 31), then message and squad.
            var names:Function = function():String
            {
               var items:Object = Menu.getInstance().contextList.list.dataProvider;
               var out:Array = [];
               for(var i:int = 1; i < items.length; i++)
                  out.push(items[i].name);
               return out.join("|");
            };
            var self:RoomsHarness = this;
            var menuFor:Function = function(what:String, want:String):void
            {
               Lobby.showPlayerInfoMenu("Bob",17,0,self);
               var got:String = names();
               trace("ROOMS player menu, " + what + ": " + got);
               if(got != want)
                  failures.push("player menu, " + what + ": " + got + ", not " + want);
            };
            menuFor("a stranger not muted", "#add_to_friends|#st_srp_hud_mute_hide|#st_srp_hud_mute_mask|#send_message|#invite_to_squad");
            friends.setPlayers([{id:123,name:"Bob",status:0}]);
            friends.setPlayers([{id:456,name:"Bob"}],false);
            menuFor("a friend hidden", "#remove_from_friends|#st_srp_hud_mute_mask|#st_srp_hud_unmute|#send_message|#invite_to_squad");
            if(Friends.getFriend("Bob").id != 123 || Friends.getIgnoredPlayer("Bob").id != 456)
               failures.push("the pushed friend and ignore lists' ids");
            friends.setPlayers([{id:uint(456 | 0x80000000),name:"Bob"}],false);
            menuFor("a friend shown as <muted>", "#remove_from_friends|#st_srp_hud_mute_hide|#st_srp_hud_unmute|#send_message|#invite_to_squad");
            friends.setPlayers([]);
            friends.setPlayers([],false);
            menuFor("unmuted, no longer a friend", "#add_to_friends|#st_srp_hud_mute_hide|#st_srp_hud_mute_mask|#send_message|#invite_to_squad");
            Player.commander = false;
            Lobby.showPlayerInfoMenu("Bob",17,0,this);
            if(Menu.getInstance().contextList.list.dataProvider.length != 5)
               failures.push("non-commander can invite to squad");
            Player.commander = true;
            Player.isSquadFull = true;
            Lobby.showPlayerInfoMenu("Bob",17,0,this);
            if(Menu.getInstance().contextList.list.dataProvider.length != 5)
               failures.push("full squad can invite another player");
            Lobby.showPlayerInfoMenu("Alice",17,0,this);
            if(Menu.getInstance().contextList.list.dataProvider.length != 1)
               failures.push("own player menu has social actions");
            Menu.hide();
            Lobby.showPlayerInfoMenu("",17,0,this);
            if(Menu.visisble)
               failures.push("empty player name opened a menu");
            Player.isSquadFull = false;
            trace("ROOMS player menu: friend/remove, the two mute entries of each state, message, squad guards and self");
         }
         catch(err:Error)
         {
            failures.push("player menus: " + err.getStackTrace());
         }
      }

      // Exercise the lobby's actual recurring layer adjustment. Background
      // panels may be re-added, but only opening a window changes its priority.
      private function checkWindowOrder():void
      {
         try
         {
            var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
            var lobby:Object = loader.content;
            lobby.squadList = new (domain.getDefinition("survarium.friends.SquadList") as Class)();
            lobby.questList = new (domain.getDefinition("survarium.quest.QuestList") as Class)();
            lobby.chat = null;
            var room:Sprite = new Sprite();
            var history:Sprite = new Sprite();
            lobby.addChild(room);
            lobby.addChild(history);
            for(var i:int = 0; i < 3; i++)
            {
               lobby.addChild(lobby.squadList);
               lobby.addChild(lobby.questList);
               lobby.layerRoomsWindow(room);
               if(lobby.getChildIndex(history) <= lobby.getChildIndex(room))
                  failures.push("History opened last was covered by a room frame");
            }
            lobby.setChildIndex(room,lobby.numChildren - 1);
            lobby.layerRoomsWindow(room);
            if(lobby.getChildIndex(room) <= lobby.getChildIndex(history))
               failures.push("room reopened last did not stay above History");
            lobby.setChildIndex(history,lobby.numChildren - 1);
            lobby.layerRoomsWindow(room);
            if(lobby.getChildIndex(history) <= lobby.getChildIndex(room))
               failures.push("History reopened last did not stay above room");
            trace("ROOMS most recently opened window stays on top across side-panel updates");
            // Use the production modal-order owner with lightweight results;
            // the full victory screen requires the game's player dictionaries.
            var Layers:Object = domain.getDefinition("survarium.rooms.RoomLayers");
            var results:Sprite = new Sprite();
            var dimmer:Sprite = new Sprite();
            lobby.addChild(dimmer);
            lobby.addChild(results);
            lobby.setChildIndex(room,lobby.numChildren - 1);
            for(i = 0; i < 3; i++)
            {
               Layers.belowResults(room,results,dimmer);
               lobby.addChild(lobby.squadList);
               lobby.layerRoomsWindow(room);
               if(lobby.getChildIndex(room) >= lobby.getChildIndex(dimmer) || lobby.getChildIndex(room) >= lobby.getChildIndex(results))
                  failures.push("late room state covered match results or their dimmer");
            }
            results.visible = false;
            lobby.setChildIndex(room,lobby.numChildren - 1);
            Layers.belowResults(room,results,dimmer);
            if(lobby.getChildIndex(room) <= lobby.getChildIndex(results))
               failures.push("closed results still constrained room order");
            trace("ROOMS post-match results stay above late room state until closed");
         }
         catch(err:Error)
         {
            failures.push("window order: " + err);
         }
      }

      // The window told an answer, a row a field list.
      private function tell(names:Array) : Boolean
      {
         var rows:Array = [];
         for(var r:int = 0; r < names.length; r++)
         {
            rows.push({
               "place":STATE - 1 + r,
               "nickname":names[r]
            });
         }
         try
         {
            Win.take(rows);
         }
         catch(err:Error)
         {
            failures.push(names[0] + ": " + err.message);
            return false;
         }
         return true;
      }

      // The texts of the row of the list whose name is `name`: its cells, drawn in the row's own
      // sprite (the chosen room's name beside the list is not in one).
      private static function rowOf(texts:Array, name:String) : Array
      {
         var at:* = null;
         for each(var t:Object in texts)
         {
            if(t.text == name && t.parent is Sprite && Sprite(t.parent).buttonMode)
            {
               at = t.parent;
            }
         }
         var row:Array = [];
         for each(var u:Object in texts)
         {
            if(at != null && u.parent == at)
            {
               row.push(u.text);
            }
         }
         return row;
      }

      // The list told `names`: the private room's row (`pinName`) without a max level, the
      // open room's row (`openName`, max level 5) with one, and the chosen room's facts with a
      // "Max level" when `factsMax`.
      private function checkList(what:String, names:Array, pinName:String, pinMax:Boolean, openName:String,
         openMax:Boolean, factsMax:Boolean) : void
      {
         if(!tell(names))
         {
            return;
         }
         var texts:Array = [];
         walk(win,[],texts,0,0,[]);
         var all:Array = texts.map(function(t:Object, ... rest):String
         {
            return t.text;
         });
         var hasMax:Function = function(row:Array):Boolean
         {
            return row.indexOf("Any") >= 0 || row.indexOf("0") >= 0 || row.indexOf("5") >= 0;
         };
         var pinRow:Array = pinName != null ? rowOf(texts,pinName) : [];
         var openRow:Array = rowOf(texts,openName);
         trace("ROOMS " + what + ": private row [" + pinRow.join(", ") + "], open row [" + openRow.join(", ") + "], facts "
            + (all.indexOf("Max level") >= 0 ? "with" : "without") + " Max level");
         if(pinName != null && pinRow.length == 0)
         {
            failures.push(what + ": no row " + pinName);
         }
         if(pinName != null && hasMax(pinRow) != pinMax)
         {
            failures.push(what + ": private row's max level " + (pinMax ? "missing" : "shown"));
         }
         if(openRow.length == 0 || hasMax(openRow) != openMax)
         {
            failures.push(what + ": open row's max level " + (openMax ? "missing" : "shown"));
         }
         if((all.indexOf("Max level") >= 0) != factsMax)
         {
            failures.push(what + ": the facts' Max level " + (factsMax ? "missing" : "shown"));
         }
      }

      // The room's own view (the owner's of 2026-10-29): a vote card's mode tag as wide as its name,
      // also after the same card showed a longer one; no vote hint line, none in the banner either
      // (the owner's of 2026-10-30); in its match, the map and
      // mode it is played on (the `L` row) under the banner's hint, whole.
      private function checkRoomView() : void
      {
         var tagOf:Function = function(text:String):Object
         {
            var found:Array = [];
            walk(win,[],found,0,0,[]);
            for each(var t:Object in found)
               if(t.text == text)
               {
                  var cell:* = t.parent;
                  var tag:* = cell.getChildAt(cell.getChildIndex(t.field) - 1);
                  return {"tag": tag.width, "text": t.field.textWidth};
               }
            return null;
         };
         // Card 1 (map 62): Blackout first, then the plain protective device.
         tell(["S1;M;2;0","0;61;5;5;3;0","1;62;13;5;6;0"]);
         tellRoom(1003, false, false, 3, null, 0, []);
         var long:Object = tagOf("PROTECTIVE DEVICE: BLACKOUT");
         tell(["S1;M;2;0","0;61;5;5;3;0","1;62;13;5;2;0"]);
         tellRoom(1003, false, false, 3, null, 0, []);
         var short:Object = tagOf("PROTECTIVE DEVICE");
         trace("ROOMS vote card tag: long " + (long != null ? long.tag + " for " + long.text : "none")
            + ", then short " + (short != null ? short.tag + " for " + short.text : "none"));
         if(long == null || short == null)
            failures.push("vote card tags not found");
         else if(short.tag > short.text + 20 || long.tag > long.text + 20)
            failures.push("a vote card's mode tag is wider than its name (" + short.tag + " for " + short.text + ")");
         var texts:Array = [];
         walk(win,[],texts,0,0,[]);
         for each(var t:Object in texts)
         {
            if(String(t.text).indexOf("Most votes wins") >= 0)
               failures.push("the vote hint line is still shown");
            if(String(t.text).indexOf("click a card") >= 0)
               failures.push("the banner's vote hint is still shown");
         }
         // In its match on map 62: the map and mode line, not cut.
         tellRoom(1003, false, false, 2, null, 0, ["L;62"]);
         var line:String = null;
         texts = [];
         walk(win,[],texts,0,0,[]);
         for each(var u:Object in texts)
            if(String(u.text).indexOf(" · Protective device") > 0)
               line = u.text;
         trace("ROOMS in its match, the banner's map line: " + line);
         if(line == null || line.indexOf("…") >= 0)
            failures.push("in its match: no whole map and mode line in the banner (" + line + ")");
      }

      // The chosen room beside the list keeps one layout through its vote: with no card ahead
      // (map 0) and with one (map 61), its name, "Map chosen by vote" or the map's line, and its
      // facts at the same heights (the owner's of 2026-10-29: the card flipped between two).
      private function checkSideStable() : void
      {
         tell(["S1;M;1;0","0;61;5;5;2;0"]);
         var heights:Function = function(map:int):Object
         {
            tell(["S1;L;1;0","1001;8;10;8;9;3;32;" + map + ";0;Kosztinpeter"]);
            var texts:Array = [];
            walk(win,[],texts,0,0,[]);
            var at:Object = {};
            for each(var t:Object in texts)
               if(t.text == "Kosztinpeter" || t.text == "State" || t.text == "Players" || t.text == "Map chosen by vote")
                  at[t.text] = Math.max(at[t.text] != null ? at[t.text] : -1, t.y);
            return at;
         };
         var none:Object = heights(0);
         var leading:Object = heights(61);
         trace("ROOMS side card, no map: name " + none["Kosztinpeter"] + ", players " + none["Players"] + ", state " + none["State"]
            + ", by vote " + none["Map chosen by vote"] + "; a map ahead: name " + leading["Kosztinpeter"] + ", players "
            + leading["Players"] + ", state " + leading["State"]);
         if(none["Map chosen by vote"] == null)
            failures.push("side card without a map: no \"Map chosen by vote\"");
         for each(var key:String in ["Kosztinpeter", "Players", "State"])
            if(none[key] == null || none[key] != leading[key])
               failures.push("side card: " + key + " moves with the map (" + none[key] + " / " + leading[key] + ")");
      }

      // Only private rooms can be made: no public sliders, and Create
      // needs exactly four PIN digits without disturbing input focus.
      private function checkCreate() : void
      {
         tell(["S1;M;1;0", "0;61;1;0;1;0"]);
         var buttons:Array = [];
         walk(win,[],[],0,0,buttons);
         var press:* = null;
         for each(var b:* in buttons)
         {
            if(b.label == "Private room")
            {
               press = b;
            }
         }
         if(press == null)
         {
            failures.push("create: no New room button");
            return;
         }
         try
         {
            var ButtonEvent:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
            press.dispatchEvent(new ButtonEvent(ButtonEvent["CLICK"]));
         }
         catch(err:Error)
         {
            failures.push("create: New room: " + err.message);
            return;
         }
         createShows("create, empty PIN",false);
         typePin("1");
         createShows("create, short PIN",false);
         typePin("1234");
         createShows("create, PIN typed",true);
         typePin("");
         createShows("create, PIN cleared",false);
         Object(win).show(false);
         if(stage.focus != null && win.contains(stage.focus))
            failures.push("hidden room still owns text focus");
         Object(win).show(true);
         trace("ROOMS PIN focus released before removal, restored after redraw, released on hide");
      }

      // The create window's PIN field (its one input) given `pin`, as typed.
      private function typePin(pin:String) : void
      {
         var texts:Array = [];
         walk(win,[],texts,0,0,[]);
         for each(var t:Object in texts)
         {
            if(t.field.type == "input")
            {
               var old:TextField = t.field as TextField;
               old.text = pin;
               stage.focus = old;
               old.setSelection(pin.length,pin.length);
               old.addEventListener(Event.REMOVED_FROM_STAGE,function(e:Event):void
               {
                  if(stage.focus == old)
                     failures.push("PIN input removed while still owning focus");
               });
               try
               {
                  t.field.dispatchEvent(new Event(Event.CHANGE));
                  var active:TextField = stage.focus as TextField;
                  if(active == null || active.stage == null || active.text != pin || active.selectionEndIndex != pin.length)
                     failures.push("PIN redraw did not transfer text/selection to the attached input");
               }
               catch(err:Error)
               {
                  failures.push("PIN redraw: " + err);
               }
               return;
            }
         }
         failures.push("create: no PIN field");
      }

      private function createShows(what:String, enabled:Boolean) : void
      {
         var texts:Array = [];
         var buttons:Array = [];
         walk(win,[],texts,0,0,buttons);
         var all:Array = texts.map(function(t:Object, ... rest):String
         {
            return t.text;
         });
         var sliders:Array = ["TEAM SIZE","PLAYERS TO START","MAX LEVEL","TIME TO START"].filter(function(label:String, ... rest):Boolean
         {
            return all.indexOf(label) >= 0;
         });
         trace("ROOMS " + what + ": sliders [" + sliders.join(", ") + "]");
         if(sliders.length != 0)
         {
            failures.push(what + ": sliders [" + sliders.join(", ") + "]");
         }
         var found:Boolean = false;
         for each(var b:* in buttons)
            if(b.label == "Create")
            {
               found = true;
               if(b.enabled != enabled)
                  failures.push(what + ": Create enabled=" + b.enabled);
            }
         if(!found)
            failures.push(what + ": no Create button");
      }

      // The window told a state of room `key`; its buttons and texts read back: what it says of
      // Alice's column ({alice: 0 left, 1 right, -1 not found}), or null when it could not.
      private function check(what:String, key:int, pin:Boolean, owner:Boolean, phase:int, teams:String,
         start:Boolean, moves:int, hint:String, sizes:Boolean, joinWhy:int = 0) : Object
      {
         var size:int = pin ? 10 : 6;
         var head:String = ["S1","R",key,0,0,size,30,phase,0,owner ? 1 : 0,5,5,key,pin ? 1 : 0,0,pin ? 1 : 2,0].join(";");
         var names:Array = [head,"C;0;61;0;0","C;1;63;0;0","C;2;65;0;0"];
         for(var i:int = 0; i < WHO.length; i++)
         {
            // P;<i>;<gear>;<owner>;<me>;<in match>;<fit>;<away>;<name>
            var me:Boolean = owner ? i == 0 : i == 1;
            names.push(["P",i,0,i == 0 ? 1 : 0,me ? 1 : 0,phase == 2 && !(me && joinWhy != 0) ? 1 : 0,0,0,WHO[i]].join(";"));
         }
         if(teams != null)
         {
            names.push("A;" + teams);
         }
         names.push("L;0");
         if(joinWhy != 0) names.push("J;-1;" + joinWhy);
         var rows:Array = [];
         for(var r:int = 0; r < names.length; r++)
         {
            rows.push({
               "place":STATE + r,
               "nickname":names[r]
            });
         }
         try
         {
            Win.take(rows);
         }
         catch(err:Error)
         {
            failures.push(what + ": " + err.message);
            return null;
         }
         var labels:Array = [];
         var texts:Array = [];
         walk(win,labels,texts,0);
         var info:String = "";
         var aliceX:Number = NaN;
         var team2X:Number = NaN;
         var texts1:Array = [];
         for each(var t:Object in texts)
         {
            texts1.push(t.text);
            if(t.text.indexOf("Pool") >= 0 && t.text.indexOf("code") >= 0)
            {
               info = t.text;
            }
            if(t.text.indexOf("Alice") == 0)
            {
               aliceX = t.x;
            }
            if(t.text.indexOf("TEAM 2") == 0)
            {
               team2X = t.x;
            }
         }
         var count:Function = function(label:String):int
         {
            // A game button's label, or a row's small button's text.
            return labels.concat(texts1).filter(function(each:String, ... rest):Boolean
            {
               return each == label;
            }).length;
         };
         var moved:int = count("»") + count("«");
         trace("ROOMS " + what + ": buttons [" + labels.join(", ") + "], moves " + (count("»") + count("«")) + ", info \"" + info + "\", Alice at " + int(aliceX)
            + ", team 2 at " + int(team2X));
         if((labels.indexOf("Start") >= 0) != start)
         {
            failures.push(what + ": Start " + (start ? "missing" : "shown"));
         }
         if(moved != moves)
         {
            failures.push(what + ": " + moved + " move buttons, not " + moves);
         }
         if(teams != null && moves > 0 && (count("»") != countOf(teams,"0") || count("«") != countOf(teams,"1")))
         {
            failures.push(what + ": » and « not as the teams " + teams);
         }
         if(hint != null && texts1.indexOf(hint) < 0)
         {
            failures.push(what + ": no \"" + hint + "\"");
         }
         if(info == "")
         {
            failures.push(what + ": no info line");
         }
         if((info.indexOf(" v ") >= 0) != sizes)
         {
            failures.push(what + ": N v N " + (sizes ? "missing" : "shown"));
         }
         // A private room has no max level either (the owner's of 2026-10-26).
         if((info.indexOf("Max level") >= 0) != !pin)
         {
            failures.push(what + ": max level " + (pin ? "shown" : "missing"));
         }
         if(teams != null && isNaN(team2X))
         {
            failures.push(what + ": no team columns");
         }
         return {"alice":isNaN(aliceX) || isNaN(team2X) ? -1 : (aliceX + 1 >= team2X - 34 ? 1 : 0)};
      }

      private static function countOf(text:String, digit:String) : int
      {
         return text.split(digit).length - 1;
      }

      // The labels of the buttons (CLIK: a `label`; the buttons themselves into `buttons`) and
      // the texts (with their x and y in the window, their field and its parent) under `at`,
      // shown ones only; `x` and `y` those of its parent in the window (unscaled: the window is
      // scaled to a stage the game sizes).
      private function walk(at:DisplayObject, labels:Array, texts:Array, x:Number, y:Number = 0, buttons:Array = null) : void
      {
         if(!at.visible)
         {
            return;
         }
         if(at is TextField)
         {
            texts.push({
               "text":TextField(at).text,
               "x":x + at.x,
               "y":y + at.y,
               "field":at,
               "parent":at.parent
            });
            return;
         }
         try
         {
            var label:* = Object(at)["label"];
            if(label is String && label != "")
            {
               labels.push(label);
               if(buttons != null)
               {
                  buttons.push(at);
               }
               return;
            }
         }
         catch(err:Error)
         {
         }
         var box:DisplayObjectContainer = at as DisplayObjectContainer;
         if(box == null)
         {
            return;
         }
         for(var i:int = 0; i < box.numChildren; i++)
         {
            walk(box.getChildAt(i),labels,texts,x + at.x,y + at.y,buttons);
         }
      }

      private function finish(result:String) : void
      {
         trace("ROOMS RESULT " + result);
      }
   }
}
