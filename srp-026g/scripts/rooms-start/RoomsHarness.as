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

   // A private room's Start (docs/notes/rooms.md, the owner's of 2026-10-26) with the lobby movie's
   // own rooms window (survarium.rooms.RoomsWindow under a BottomPanel of the movie), told room
   // states as the server's answers (`S1;R`, lobby.md 2.16.3) through RoomsWindow.take:
   // - the owner of a private room waiting: a Start button, "Press Start when everyone is in.";
   // - a member of it: no Start, "Waiting for the creator to start.";
   // - the owner of an open room: no Start (its countdown starts it);
   // - the owner of a private room playing its match: no Start;
   // - a private room's info line has no "N v N", an open room's has.
   // Prints "ROOMS ..." lines (trace) and ends with "ROOMS RESULT pass" or "ROOMS RESULT fail: ...".
   public class RoomsHarness extends Sprite
   {
      private static const STATE:int = 2060000001;

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
         // 5 members; the asker the first (`owner`) or the second.
         check("private, owner, waiting", 1001, true, true, 0, true, "Press Start when everyone is in.", false);
         check("private, member, waiting", 1002, true, false, 0, false, "Waiting for the creator to start.", false);
         check("open, owner, waiting", 1003, false, true, 0, false, "Waiting for players", true);
         check("private, owner, in match", 1004, true, true, 2, false, null, false);
         finish(failures.length == 0 ? "pass" : "fail: " + failures.join("; "));
      }

      // The window told a state of room `key`; its buttons and texts read back.
      private function check(what:String, key:int, pin:Boolean, owner:Boolean, phase:int, start:Boolean, hint:String, sizes:Boolean) : void
      {
         var size:int = pin ? 10 : 6;
         var head:String = ["S1","R",key,0,0,size,30,phase,0,owner ? 1 : 0,5,5,key,pin ? 1 : 0,0,pin ? 1 : 2,0].join(";");
         var names:Array = [head,"C;0;61;0;0","C;1;63;0;0","C;2;65;0;0"];
         var who:Array = ["Alice","Bob","Carol","Dave","Erin"];
         for(var i:int = 0; i < who.length; i++)
         {
            // P;<i>;<gear>;<owner>;<me>;<in match>;<fit>;<away>;<name>
            var me:Boolean = owner ? i == 0 : i == 1;
            names.push(["P",i,0,i == 0 ? 1 : 0,me ? 1 : 0,phase == 2 ? 1 : 0,0,0,who[i]].join(";"));
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
            return;
         }
         var labels:Array = [];
         var texts:Array = [];
         walk(win,labels,texts);
         var hasStart:Boolean = labels.indexOf("Start") >= 0;
         var hasHint:Boolean = hint == null || texts.indexOf(hint) >= 0;
         var info:String = "";
         for each(var t:String in texts)
         {
            if(t.indexOf("Max level") >= 0 && t.indexOf("code") >= 0)
            {
               info = t;
            }
         }
         var hasSizes:Boolean = info.indexOf(" v ") >= 0;
         trace("ROOMS " + what + ": buttons [" + labels.join(", ") + "], info \"" + info + "\"");
         if(hasStart != start)
         {
            failures.push(what + ": Start " + (hasStart ? "shown" : "missing"));
         }
         if(!hasHint)
         {
            failures.push(what + ": no \"" + hint + "\"");
         }
         if(hasSizes != sizes)
         {
            failures.push(what + ": N v N " + (hasSizes ? "shown" : "missing"));
         }
      }

      // The labels of the buttons (CLIK: a `label`) and the texts under `at`, shown ones only.
      private function walk(at:DisplayObject, labels:Array, texts:Array) : void
      {
         if(!at.visible)
         {
            return;
         }
         if(at is TextField)
         {
            texts.push(TextField(at).text);
            return;
         }
         try
         {
            var label:* = Object(at)["label"];
            if(label is String && label != "")
            {
               labels.push(label);
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
            walk(box.getChildAt(i),labels,texts);
         }
      }

      private function finish(result:String) : void
      {
         trace("ROOMS RESULT " + result);
      }
   }
}
