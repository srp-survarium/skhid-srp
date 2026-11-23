package
{
   import flash.display.*;
   import flash.events.*;
   import flash.geom.Point;
   import flash.net.URLRequest;
   import flash.system.*;

   // Exe patch 200's crosshair in the hud movie (docs/notes/muzzle-shots.md): its own root fed a
   // match's start and root.srp_crosshair_at(x, y) as the game calls it. Checks:
   // - a place given in parts of the screen is that pixel of the stage, at the stage's size;
   // - 0.5, 0.5 is exactly where the movie's own layout had the crosshair (the centre's pixel);
   // - the crosshair's size and its being shown are untouched by the call.
   // Ends with "CROSSHAIR RESULT pass" or "... fail: ...".
   public class CrosshairHarness extends Sprite
   {
      private var loader:Loader = new Loader();
      private var hud:*;
      private var failures:Array = [];
      private var phase:int = 0;
      private var frames:int = 0;

      public function CrosshairHarness()
      {
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
            function(e:UncaughtErrorEvent):void { e.preventDefault(); failures.push(String(e.error)); trace("CROSSHAIR uncaught " + e.error); });
         loader.contentLoaderInfo.addEventListener(Event.INIT,function(e:Event):void { stage.addChild(loader.content); });
         loader.load(new URLRequest("hud.swf"),new LoaderContext(false,new ApplicationDomain()));
         addEventListener(Event.ENTER_FRAME,tick);
      }

      private function check(ok:Boolean, message:String) : void
      {
         trace("CROSSHAIR " + (ok ? "ok " : "FAIL ") + message);
         if(!ok) failures.push(message);
      }

      // The crosshair's own origin on the stage.
      private function place() : Point
      {
         return hud.crosshair.localToGlobal(new Point(0,0));
      }

      private function tick(e:Event) : void
      {
         try
         {
            frames++;
            if(phase == 0)
            {
               if(loader.content == null || Object(loader.content).progress_indicator == null) return;
               hud = loader.content;
               hud.reset();
               hud.set_game_type(1);
               hud.set_local_player("Hunter");
               hud.show_crosshair(true);
               hud.set_crosshair_size(40);
               phase = 1;
               frames = 0;
               return;
            }
            if(phase == 1)
            {
               if(frames < 30) return;
               var w:Number = stage.stageWidth, h:Number = stage.stageHeight;
               var laid:Point = place();
               var shown:Boolean = hud.crosshair.visible;
               var size:Number = hud.crosshair.width;
               trace("CROSSHAIR stage " + w + "x" + h + ", laid out at " + laid.x + ", " + laid.y);
               check(Math.abs(laid.x - w / 2) <= 1 && Math.abs(laid.y - h / 2) <= 1,"the layout's crosshair at the stage's centre");
               hud.srp_crosshair_at(0.75,0.25);
               var moved:Point = place();
               check(Math.abs(moved.x - 0.75 * w) < 0.01 && Math.abs(moved.y - 0.25 * h) < 0.01,
                  "0.75, 0.25 of the screen: " + moved.x + ", " + moved.y);
               hud.srp_crosshair_at(0.515,0.492);
               moved = place();
               check(Math.abs(moved.x - 0.515 * w) < 0.01 && Math.abs(moved.y - 0.492 * h) < 0.01,
                  "a place a little off the centre (a gun held ready): " + moved.x + ", " + moved.y);
               hud.srp_crosshair_at(0.5,0.5);
               var back:Point = place();
               check(Math.abs(back.x - laid.x) < 0.001 && Math.abs(back.y - laid.y) < 0.001,
                  "0.5, 0.5 is where the layout had it: " + back.x + ", " + back.y);
               check(hud.crosshair.visible == shown && hud.crosshair.width == size,"its size and its being shown untouched");
               trace("CROSSHAIR RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
               phase = 2;
            }
         }
         catch(error:Error)
         {
            trace("CROSSHAIR RESULT fail: " + error + " " + error.getStackTrace());
            phase = 2;
         }
      }
   }
}
