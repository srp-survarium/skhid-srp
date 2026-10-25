package
{
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.system.fscommand;

   // Feeds the hud movie's own kill feed (SrvCombatLogList, with its timeline symbols) every kind of
   // row the exe sends, then checks that a row added after they have all expired stays on screen.
   // Prints "KF ..." lines (trace) and ends with "KF RESULT pass" or "KF RESULT fail: ...".
   public class KillFeedHarness extends Sprite
   {
      private var loader:Loader = new Loader();

      private var list:MovieClip;

      private var base:int;

      private var frame:int = 0;

      private var failures:Array = [];

      public function KillFeedHarness()
      {
         var movie:String = loaderInfo.parameters.movie || "hud.swf";
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         // the hud's own root fails outside the game (no exe): only its classes are used
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
         });
         loader.load(new URLRequest(movie),new LoaderContext(false,new ApplicationDomain()));
      }

      // action: 1 kill, 2 suicide, 3 found; the exe's patch 67 flags; extra: the cause icon
      private static function row(action:int, wall:int, ricochet:int, pain:int, extra:int = 0) : Object
      {
         return {
            action_id:action,
            who_name:"killer",
            victim_name:"victim",
            object_icon:3,
            extra_icon:extra,
            mastery_icon:0,
            who_team:0,
            victim_team:1,
            penetration_icon:wall,
            ricochet_icon:ricochet,
            pain_icon:pain
         };
      }

      private function rows() : int
      {
         return list.numChildren - base;
      }

      private function add(name:String, data:Object, mayThrow:Boolean = false) : void
      {
         try
         {
            list.addItem(data);
            trace("KF add " + name + ": ok, rows " + rows());
         }
         catch(e:Error)
         {
            trace("KF add " + name + ": threw " + e.message);
            if(!mayThrow)
            {
               failures.push(name + " threw " + e.message);
            }
         }
      }

      private function loaded(e:Event) : void
      {
         var ListClass:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("SrvCombatLogList") as Class;
         list = new ListClass() as MovieClip;
         base = list.numChildren;
         addChild(list);
         addEventListener(Event.ENTER_FRAME,this.tick);
      }

      private function tick(e:Event) : void
      {
         frame++;
         if(frame == 1)
         {
            add("kill",row(1,0,0,0));
            add("kill headshot",row(1,0,0,0,1));
            add("kill wall",row(1,1,0,0));
            add("kill ricochet",row(1,0,1,0));
            add("kill wall pain",row(1,1,0,1));
            add("kill ricochet pain headshot",row(1,0,1,1,1));
            add("suicide",row(2,0,0,0,2));
            add("suicide pain",row(2,0,0,1,2));
            add("found",row(3,0,0,0));
            if(rows() != 9)
            {
               failures.push("9 rows added, " + rows() + " in the list");
            }
         }
         else if(frame == 130)
         {
            // a row whose data throws (no name): the list must not be left with a stray renderer
            var bad:Object = row(1,0,0,0);
            bad.who_name = null;
            add("a row whose data throws",bad,true);
         }
         else if(frame == 300)
         {
            // every row above has run its 120 frames: a new one stays its own time
            add("kill after the others expired",row(1,0,0,0));
         }
         else if(frame == 340)
         {
            trace("KF frame 340: rows " + rows());
            if(rows() != 1)
            {
               failures.push("a row added 40 frames ago is gone (rows " + rows() + ")");
            }
            finish(failures.length ? "fail: " + failures.join("; ") : "pass");
         }
         if(frame % 20 == 0)
         {
            trace("KF frame " + frame + ": rows " + rows());
         }
      }

      private function finish(result:String) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.tick);
         trace("KF RESULT " + result);
         fscommand("quit");
      }
   }
}
