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

   // A private room's Start and teams (docs/notes/rooms.md, the owner's of 2026-10-26) with the
   // lobby movie's own rooms window (survarium.rooms.RoomsWindow over the movie's bottom bar), told
   // room states as the server's answers (`S1;R`, lobby.md 2.16.3) through RoomsWindow.take:
   // - the owner of a private room waiting: Start, "Press Start when everyone is in.", a move
   //   button on every member (itself too: » on team 1, « on team 2), the two teams in two columns;
   // - a member of it: no Start, no move, "Waiting for the creator to start.";
   // - the owner of an open room: no Start, no move (its countdown starts it, its sides drawn);
   // - the owner of a private room playing its match: no Start, no move;
   // - a move told (the next state's `A;<sides>`): the member in the other column;
   // - a private room's info line has no "N v N", an open room's has.
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
         var step:String = "the classes";
         try
         {
            var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
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
         finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
      }

      // The window told a state of room `key`; its buttons and texts read back: what it says of
      // Alice's column ({alice: 0 left, 1 right, -1 not found}), or null when it could not.
      private function check(what:String, key:int, pin:Boolean, owner:Boolean, phase:int, teams:String,
         start:Boolean, moves:int, hint:String, sizes:Boolean) : Object
      {
         var size:int = pin ? 10 : 6;
         var head:String = ["S1","R",key,0,0,size,30,phase,0,owner ? 1 : 0,5,5,key,pin ? 1 : 0,0,pin ? 1 : 2,0].join(";");
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

      // The labels of the buttons (CLIK: a `label`) and the texts (with their x on the stage)
      // under `at`, shown ones only; `x` that of its parent in the window (unscaled: the window is
   // scaled to a stage the game sizes).
      private function walk(at:DisplayObject, labels:Array, texts:Array, x:Number) : void
      {
         if(!at.visible)
         {
            return;
         }
         if(at is TextField)
         {
            texts.push({
               "text":TextField(at).text,
               "x":x + at.x
            });
            return;
         }
         try
         {
            var label:* = Object(at)["label"];
            if(label is String && label != "")
            {
               labels.push(label);
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
            walk(box.getChildAt(i),labels,texts,x + at.x);
         }
      }

      private function finish(result:String) : void
      {
         trace("ROOMS RESULT " + result);
      }
   }
}
