package
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.text.TextField;
   import flash.text.TextFieldType;

   // A private room's Start and teams (docs/notes/rooms.md, the owner's of 2026-10-26) with the
   // lobby movie's own rooms window (survarium.rooms.RoomsWindow over the movie's bottom bar), told
   // room states as the server's answers (`S1;R`, lobby.md 2.16.3) through RoomsWindow.take:
   // - the owner of a private room waiting: Start, "Press Start when everyone is in.", a move
   //   button on every member (itself too: » on team 1, « on team 2), the two teams in two columns;
   // - a member of it: no Start, no move, "Waiting for the creator to start.";
   // - the owner of an open room: no Start, no move (its countdown starts it, its sides drawn);
   // - the owner of a private room playing its match: no Start, no move;
   // - a move told (the next state's `A;<sides>`): the member in the other column;
   // - a private room's info line has no "N v N", an open room's has;
   // - the room's bots a team (the state's field 17, the owner's of 2026-10-26): "Bots per team N" on
   //   the info line when it has some, nothing of them when it has none;
   // - New room: a "Bots per team" slider (None to 3 v 3), in an open room's form beside Players to
   //   start (one row), in a private room's (a PIN typed) under Max level;
   // - with bots chosen (the form's start, `S1;F`), the pool's maps and modes the server says the bots
   //   don't play (the pool's `bots` 0) greyed out and said so ("Bots can't play it"), none of them
   //   greyed with no bots (the owner's of 2026-10-26).
   // Prints "ROOMS ..." lines (trace) and ends with "ROOMS RESULT pass" or "ROOMS RESULT fail: ...".
   public class RoomsHarness extends Sprite
   {
      private static const STATE:int = 2060000001;

      private static const POOL:int = 2060000010;

      private static const FORM:int = 2060000009;

      private static const WHO:Array = ["Alice","Bob","Carol","Dave","Erin"];

      private var loader:Loader = new Loader();

      private var Win:*;

      private var win:Sprite;

      private var failures:Array = [];

      private var domain:ApplicationDomain;

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
         var step:String = "the classes";
         try
         {
            domain = loader.contentLoaderInfo.applicationDomain;
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
         check("open, owner, 2 bots a team", 1005, false, true, 0, null, false, 0, null, true, 2);
         check("private, owner, 3 bots a team", 1006, true, true, 0, "01010", true, 5, null, false, 3);
         createForm();
         greyedForms();
         finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
      }

      // The window told a state of room `key`; its buttons and texts read back: what it says of
      // Alice's column ({alice: 0 left, 1 right, -1 not found}), or null when it could not.
      private function check(what:String, key:int, pin:Boolean, owner:Boolean, phase:int, teams:String,
         start:Boolean, moves:int, hint:String, sizes:Boolean, bots:int = 0) : Object
      {
         var size:int = pin ? 10 : 6;
         var head:String = ["S1","R",key,0,0,size,30,phase,0,owner ? 1 : 0,5,5,key,pin ? 1 : 0,0,pin ? 1 : 2,0,bots].join(";");
         var names:Array = [head,"C;0;61;0;0","C;1;63;0;0","C;2;65;0;0"];
         for(var i:int = 0; i < WHO.length; i++)
         {
            // P;<i>;<gear>;<owner>;<me>;<in match>;<fit>;<away>;<name>
            var me:Boolean = owner ? i == 0 : i == 1;
            names.push(["P",i,0,i == 0 ? 1 : 0,me ? 1 : 0,phase == 2 ? 1 : 0,0,0,WHO[i]].join(";"));
         }
         if(teams != null)
         {
            names.push("A;" + teams);
         }
         names.push("L;0");
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
            if(t.text.indexOf("Max level") >= 0 && t.text.indexOf("code") >= 0)
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
         if((info.indexOf(" v ") >= 0) != sizes)
         {
            failures.push(what + ": N v N " + (sizes ? "missing" : "shown"));
         }
         if((info.indexOf("Bots per team " + bots) >= 0) != (bots > 0) || bots == 0 && info.indexOf("Bots") >= 0)
         {
            failures.push(what + ": bots " + (bots > 0 ? "missing" : "shown") + " in \"" + info + "\"");
         }
         if(teams != null && isNaN(team2X))
         {
            failures.push(what + ": no team columns");
         }
         return {"alice":isNaN(aliceX) || isNaN(team2X) ? -1 : (aliceX + 1 >= team2X - 34 ? 1 : 0)};
      }

      // New room from the list (no room): the form's sliders, open and private (a PIN typed).
      private function createForm() : void
      {
         try
         {
            Win.take([{
               "place":STATE,
               "nickname":"S1;N;0;0;0"
            }]);
            var labels:Array = [];
            var texts:Array = [];
            var buttons:Array = [];
            walk(win,labels,texts,0,0,buttons);
            var made:* = null;
            for each(var b:* in buttons)
            {
               if(b.label == "New room")
               {
                  made = b;
               }
            }
            if(made == null)
            {
               failures.push("create: no New room in [" + labels.join(", ") + "]");
               return;
            }
            var press:Class = domain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
            made.dispatchEvent(new press(press.CLICK));
            formSliders("create, open",true);
            texts = [];
            walk(win,[],texts,0,0,[]);
            var pin:TextField = null;
            for each(var t:Object in texts)
            {
               if(t.field.type == TextFieldType.INPUT)
               {
                  pin = t.field;
               }
            }
            if(pin == null)
            {
               failures.push("create: no PIN field");
               return;
            }
            pin.text = "1234";
            pin.dispatchEvent(new Event(Event.CHANGE));
            formSliders("create, private",false);
         }
         catch(err:Error)
         {
            failures.push("create: " + err.message);
         }
      }

      // The shown button labelled `label`, or null.
      private function button(label:String) : *
      {
         var buttons:Array = [];
         walk(win,[],[],0,0,buttons);
         for each(var b:* in buttons)
         {
            if(b.label == label)
            {
               return b;
            }
         }
         return null;
      }

      private function press(label:String) : Boolean
      {
         var b:* = button(label);
         if(b == null)
         {
            return false;
         }
         var event:Class = domain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
         b.dispatchEvent(new event(event.CLICK));
         return true;
      }

      // A pool of three entries: a map the bots play (level 1, Protective device), the same map in
      // Research (no mode of theirs) and a map without their walk data (level 2). The create window
      // opened from a form of 3 bots a team, then of none.
      private function greyedForms() : void
      {
         try
         {
            press("Back");
            Win.take([{
               "place":STATE,
               "nickname":"S1;N;0;0;0"
            }]);
            var pool:Array = ["S1;M;3;0","0;61;1;1;2;0;1","1;71;1;1;5;0;0","2;63;2;1;2;0;0"];
            var rows:Array = [];
            for(var r:int = 0; r < pool.length; r++)
            {
               rows.push({
                  "place":POOL + r,
                  "nickname":pool[r]
               });
            }
            Win.take(rows);
            for each(var bots:int in [3,0])
            {
               Win.take([{
                  "place":FORM,
                  "nickname":"S1;F;1;8;1;0;30;;" + bots
               },{
                  "place":FORM + 1,
                  "nickname":"E;0;7"
               }]);
               if(!press("New room"))
               {
                  failures.push("greyed: no New room");
                  return;
               }
               var texts:Array = [];
               walk(win,[],texts,0,0,[]);
               var said:Array = [];
               for each(var t:Object in texts)
               {
                  if(t.text.indexOf("an't play it") >= 0 || t.text.indexOf("AN'T PLAY IT") >= 0)
                  {
                     said.push(t.text);
                  }
               }
               trace("ROOMS greyed, " + bots + " bots: [" + said.join(" | ") + "]");
               if(bots > 0)
               {
                  var research:Boolean = false;
                  var level2:Boolean = false;
                  var tile:Boolean = false;
                  for each(var s:String in said)
                  {
                     research = research || s.indexOf("Research") == 0;
                     level2 = level2 || s.indexOf("Research") < 0 && s.indexOf("·") > 0;
                     tile = tile || s == "BOTS CAN'T PLAY IT";
                  }
                  if(!research || !level2 || !tile)
                  {
                     failures.push("greyed with bots: Research " + research + ", the map without data " + level2
                        + ", its tile " + tile);
                  }
               }
               else if(said.length > 0)
               {
                  failures.push("greyed with no bots: " + said.join(" | "));
               }
               press("Back");
            }
         }
         catch(err:Error)
         {
            failures.push("greyed: " + err.message);
         }
      }

      // The form's slider labels: Bots per team with its range (None to 3 v 3); in an open room's
      // form on the row of Players to start, right of it; in a private room's alone (no team size,
      // no players to start).
      private function formSliders(what:String, open:Boolean) : void
      {
         var texts:Array = [];
         walk(win,[],texts,0,0,[]);
         var at:Function = function(text:String):Object
         {
            for each(var t:Object in texts)
            {
               if(t.text == text)
               {
                  return t;
               }
            }
            return null;
         };
         var bots:Object = at("BOTS PER TEAM");
         var players:Object = at("PLAYERS TO START");
         var size:Object = at("TEAM SIZE");
         var max:Object = at("MAX LEVEL");
         trace("ROOMS " + what + ": bots " + (bots == null ? "none" : int(bots.x) + "," + int(bots.y)) + ", players to start "
            + (players == null ? "none" : int(players.x) + "," + int(players.y)) + ", None " + (at("None") != null)
            + ", 3 v 3 " + (at("3 v 3") != null));
         if(bots == null || at("None") == null || at("3 v 3") == null)
         {
            failures.push(what + ": no Bots per team slider from None to 3 v 3");
            return;
         }
         if(open && (players == null || size == null || int(players.y) != int(bots.y) || bots.x <= players.x))
         {
            failures.push(what + ": Bots per team not beside Players to start");
         }
         if(!open && (players != null || size != null || max == null || bots.y <= max.y))
         {
            failures.push(what + ": a private room's form not Max level then Bots per team");
         }
      }

      private static function countOf(text:String, digit:String) : int
      {
         return text.split(digit).length - 1;
      }

      // The labels of the buttons (CLIK: a `label`; the buttons too, if asked) and the texts (with
      // their x and y on the stage, and the field) under `at`, shown ones only; `x` and `y` those of
      // its parent in the window (unscaled: the window is scaled to a stage the game sizes).
      private function walk(at:DisplayObject, labels:Array, texts:Array, x:Number, y:Number = 0,
         buttons:Array = null) : void
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
               "field":at
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
