package
{
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.Loader;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.utils.setTimeout;

   // The lobby scene's fade (exe patch 81, docs/notes/lobby-scenes.md) with the lobby movie's own
   // root (LobbyMenu.srp_scene_fade), a panel of ours on it as the lobby's panels are:
   // - to black (1, 250 ms): the black the root's first child, under the panel; drawn black where
   //   no panel is after the fade, the panel still drawn over it; no click on it (not mouse enabled);
   // - from black (2, 400 ms): gone after the fade, off the display list;
   // - gone at once (0) during a fade to black: off the display list, no tween left to bring it back;
   // - a second fade to black while it is out: one black, still the first child.
   // Prints "FADE ..." lines (trace) and ends with "FADE RESULT pass" or "FADE RESULT fail: ...".
   public class FadeHarness extends Sprite
   {
      private var loader:Loader = new Loader();

      private var lobby:Object;

      private var panel:Sprite = new Sprite();

      // the black, as the first fade put it
      private var shade:Sprite;

      private var failures:Array = [];

      public function FadeHarness()
      {
         var movie:String = loaderInfo.parameters.movie || "lobby_menu.swf";
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         // the lobby's own root fails outside the game (no exe): its fade does not need it
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
         });
         loader.load(new URLRequest(movie),new LoaderContext(false,new ApplicationDomain()));
      }

      private function loaded(e:Event) : void
      {
         try
         {
            lobby = loader.content;
            addChild(loader);
            // a panel of the lobby's: a white square at 100,100
            panel.graphics.beginFill(0xFFFFFF,1);
            panel.graphics.drawRect(100,100,100,100);
            panel.graphics.endFill();
            lobby.addChild(panel);
            lobby.srp_scene_fade(1,250);
         }
         catch(err:Error)
         {
            finish("fail: the fade to black: " + err);
            return;
         }
         setTimeout(this.black,600);
      }

      private function fade() : Sprite
      {
         return lobby.getChildAt(0) as Sprite;
      }

      // The colour drawn at x,y of the lobby's root (the root alone, as the game draws it over the scene).
      private function at(x:int, y:int) : uint
      {
         var shot:BitmapData = new BitmapData(400,400,true,0);
         shot.draw(DisplayObject(lobby));
         return shot.getPixel32(x,y);
      }

      private function black() : void
      {
         var first:Sprite = this.fade();
         shade = first;
         if(first == null || first == panel || first.mouseEnabled)
            failures.push("to black: the first child is not the black (" + first + ")");
         else if(first.alpha < 0.99)
            failures.push("to black: alpha " + first.alpha + " after the fade");
         if(lobby.getChildIndex(panel) <= 0)
            failures.push("to black: the panel under the black");
         var outside:uint = at(300,300);
         var inside:uint = at(150,150);
         trace("FADE to black: " + outside.toString(16) + " where no panel is, " + inside.toString(16) + " on the panel");
         if(outside != 0xFF000000)
            failures.push("to black: not black where no panel is (" + outside.toString(16) + ")");
         if(inside != 0xFFFFFFFF)
            failures.push("to black: the panel not drawn over it (" + inside.toString(16) + ")");
         lobby.srp_scene_fade(1,250);
         if(lobby.getChildAt(0) != first || lobby.getChildAt(1) == first)
            failures.push("a second fade to black: not one black, the first child");
         lobby.srp_scene_fade(2,400);
         setTimeout(this.cleared,800);
      }

      private function cleared() : void
      {
         trace("FADE from black: " + at(300,300).toString(16) + " where no panel is");
         if(shade.parent != null)
            failures.push("from black: the black still on the root");
         if(at(300,300) == 0xFF000000)
            failures.push("from black: still black");
         lobby.srp_scene_fade(1,250);
         setTimeout(function():void
         {
            lobby.srp_scene_fade(0,0);
            setTimeout(gone,600);
         },100);
      }

      private function gone() : void
      {
         trace("FADE gone at once: " + at(300,300).toString(16) + " where no panel is");
         if(shade.parent != null || at(300,300) == 0xFF000000)
            failures.push("gone at once: still there, or brought back by the tween");
         finish(failures.length ? "fail: " + failures.join("; ") : "pass");
      }

      private function finish(result:String) : void
      {
         trace("FADE RESULT " + result);
      }
   }
}
