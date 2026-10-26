package
{
   import flash.display.DisplayObject;
   import flash.display.Loader;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.UncaughtErrorEvent;
   import flash.net.URLRequest;
   import flash.system.ApplicationDomain;
   import flash.system.LoaderContext;
   import flash.system.fscommand;

   // Blackout's markers over heads (docs/notes/blackout.md) with the player icons movie itself (its
   // root, MainTimeline, as the game loads it): a player's icon (id 1) and a match object's (id 40,
   // HUDIconLayer.FIRST_OBJECT on), fed as update_player_icons feeds them, then the states
   // Blackout (patch 65) tells: not forced, both shown; forced, none; forced with Tab held, the player's only
   // (F11, hide_interface, doing nothing then);
   // Tab let go, none; not forced again, both.
   // Prints "ICONS ..." lines (trace) and ends with "ICONS RESULT pass" or "ICONS RESULT fail: ...".
   public class IconsHarness extends Sprite
   {
      private var loader:Loader = new Loader();

      private var movie:MovieClip;

      private var frame:int = 0;

      private var failures:Array = [];

      public function IconsHarness()
      {
         loader.contentLoaderInfo.addEventListener(Event.INIT,this.loaded);
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            trace("ICONS the movie threw: " + e.error);
            e.preventDefault();
         });
         loader.load(new URLRequest(loaderInfo.parameters.movie || "player_icons.swf"),
            new LoaderContext(false,new ApplicationDomain()));
      }

      private function loaded(e:Event) : void
      {
         movie = loader.content as MovieClip;
         addChild(loader);
         addEventListener(Event.ENTER_FRAME,this.tick);
      }

      // Whether the icon of `id` is on the screen: itself and every parent up to the movie visible. The
      // layer's children are its icons in the order made (no Research markers here): 1, then 40.
      private function shown(id:uint) : Boolean
      {
         var icon:DisplayObject = movie.iconControl.getChildAt(id == 1 ? 0 : 1);
         for(var at:DisplayObject = icon; at != null && at != movie; at = at.parent)
         {
            if(!at.visible)
            {
               return false;
            }
         }
         return icon != null && movie.visible;
      }

      private function expect(what:String, player:Boolean, object:Boolean) : void
      {
         // the feed, as every frame
         movie.set_player_icons_props([{id:1,visible:true,x:100,y:100,z:1,name:"teammate",name_visible:true},
            {id:40,visible:true,x:200,y:100,z:1}]);
         var p:Boolean = shown(1), o:Boolean = shown(40);
         trace("ICONS " + what + ": the player's " + (p ? "shown" : "hidden") + ", the object's " + (o ? "shown" : "hidden"));
         if(p != player || o != object)
         {
            failures.push(what + ": player " + p + " (want " + player + "), object " + o + " (want " + object + ")");
         }
      }

      private function tick(e:Event) : void
      {
         frame++;
         if(frame == 3)
         {
            if(movie == null || movie.iconControl == null)
            {
               finish("fail: no icon layer (frame 1 did not run)");
               return;
            }
            movie.create_player_icon(1,0,1);
            movie.create_player_icon(40,1,1);
            movie.set_player_icon_type(40,1);
            expect("a plain match",true,true);
            movie.force_interface_hidden(true);
            expect("Blackout",false,false);
            movie.force_interface_tab(true);
            expect("Blackout, Tab held",true,false);
            movie.hide_interface(true);
            expect("Blackout, Tab held, F11 (hide_interface) pressed: it does nothing",true,false);
            movie.force_interface_tab(false);
            expect("Blackout, Tab let go",false,false);
            movie.force_interface_tab(true);
            movie.force_interface_hidden(false);
            expect("the next plain match (Tab held)",true,true);
            movie.force_interface_tab(false);
            expect("the next plain match",true,true);
            finish(failures.length ? "fail: " + failures.join("; ") : "pass");
         }
      }

      private function finish(result:String) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.tick);
         trace("ICONS RESULT " + result);
         fscommand("quit");
      }
   }
}
