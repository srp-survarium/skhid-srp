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

   // The lobby scene's black (exe patch 81, docs/notes/lobby-scenes.md) with the lobby movie's own
   // root (LobbyMenu.srp_scene_fade(per mille, loading), the exe stepping it a frame each), a panel
   // of ours on it as the lobby's panels are:
   // - half (500, 0): the black the root's first child, under the panel, half drawn; no click on it;
   // - whole with the loading spinner (1000, 1): black where no panel is, the spinner on it, the
   //   panel drawn over it; a second call: one black, still the first child;
   // - gone (0, 0): off the display list;
   // - the lobby window (srp_lobby_window, its button's toggle): four cards, the state the exe
   //   tells (srp_lobby_state: the pick 3 marked), opened and closed, then left open (check.sh's
   //   window.png).
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
            // The English texts, as the exe gives them (set_localization_data).
            loader.contentLoaderInfo.applicationDomain.getDefinition("survarium.data.LocalizationData").setData(HarnessText.DATA.english);
            addChild(loader);
            // a panel of the lobby's: a white square at 100,100
            panel.graphics.beginFill(0xFFFFFF,1);
            panel.graphics.drawRect(100,100,100,100);
            panel.graphics.endFill();
            lobby.addChild(panel);
            lobby.srp_scene_fade(500,0);
         }
         catch(err:Error)
         {
            finish("fail: the black: " + err);
            return;
         }
         setTimeout(this.half,100);
      }

      private function half() : void
      {
         var first:Sprite = this.fade();
         shade = first;
         if(first == null || first == panel || first.mouseEnabled || Math.abs(first.alpha - 0.5) > 0.01)
            failures.push("half: the first child is not the half black (" + first + ")");
         var outside:uint = at(300,300);
         trace("FADE half: " + outside.toString(16) + " where no panel is");
         if(outside != 0x80000000 && (outside >>> 24) < 0x70)
            failures.push("half: not half black (" + outside.toString(16) + ")");
         lobby.srp_scene_fade(1000,1);
         lobby.srp_scene_fade(1000,1);
         setTimeout(this.black,100);
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
         if(lobby.getChildAt(0) != shade || lobby.getChildAt(1) == shade)
            failures.push("whole: not one black, the first child");
         if(lobby.getChildIndex(panel) <= 0)
            failures.push("whole: the panel under the black");
         var spinner:Object = shade.numChildren > 0 ? shade.getChildAt(0) : null;
         if(spinner == null || !spinner.visible)
            failures.push("whole: no loading spinner");
         var outside:uint = at(20,20);
         var inside:uint = at(150,150);
         trace("FADE whole: " + outside.toString(16) + " where no panel is, " + inside.toString(16) + " on the panel");
         if(outside != 0xFF000000)
            failures.push("whole: not black where no panel is (" + outside.toString(16) + ")");
         if(inside != 0xFFFFFFFF)
            failures.push("whole: the panel not drawn over it (" + inside.toString(16) + ")");
         lobby.srp_scene_fade(0,0);
         if(shade.parent != null)
            failures.push("gone: the black still on the root");
         lobby.srp_lobby_state(3,2);
         lobby.srp_lobby_window();
         setTimeout(this.window,200);
      }

      private function window() : void
      {
         var w:Object = null;
         for(var i:int = 0; i < lobby.numChildren; i++)
         {
            if(lobby.getChildAt(i).name == "srpLobbyWindow")
               w = lobby.getChildAt(i);
         }
         if(w == null)
         {
            failures.push("the lobby window not opened");
         }
         else
         {
            trace("FADE window: " + int(w.width) + " x " + int(w.height) + " at " + int(w.x) + "," + int(w.y));
            // two by two of vote-sized cards (280 x 190)
            if(w.width < 2 * 280 || w.height < 2 * 190)
               failures.push("the lobby window too small: " + w.width + " x " + w.height);
            lobby.srp_lobby_window();
            if(w.parent != null)
               failures.push("the lobby window not closed by its toggle");
            lobby.srp_lobby_window();
         }
         finish(failures.length ? "fail: " + failures.join("; ") : "pass");
      }

      private function finish(result:String) : void
      {
         trace("FADE RESULT " + result);
      }
   }
}
