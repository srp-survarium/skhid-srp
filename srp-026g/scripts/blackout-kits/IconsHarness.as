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
   // root, MainTimeline, as the game loads it): a teammate's icon (id 1), an enemy's (id 2, in
   // set_enemy_icon_ids) and a match object's (id 40, HUDIconLayer.FIRST_OBJECT on), fed as
   // update_player_icons feeds them, then the states Blackout (patch 65) tells: not forced, the
   // teammate's and the object's shown; forced, the teammate's only (F11, hide_interface, doing
   // nothing then); not forced again, the teammate's and the object's. The enemy's never.
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
      // layer can also contain Research's container; remember each icon when it is made.
      private var player:DisplayObject;

      private var enemy:DisplayObject;

      private var objectMarker:DisplayObject;

      private function shown(id:uint) : Boolean
      {
         var icon:DisplayObject = id == 1 ? player : id == 2 ? enemy : objectMarker;
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
            {id:2,visible:true,x:150,y:100,z:1,name:"enemy",name_visible:true},
            {id:40,visible:true,x:200,y:100,z:1}]);
         var p:Boolean = shown(1), e:Boolean = shown(2), o:Boolean = shown(40);
         trace("ICONS " + what + ": the teammate's " + (p ? "shown" : "hidden") + ", the enemy's "
            + (e ? "shown" : "hidden") + ", the object's " + (o ? "shown" : "hidden"));
         if(p != player || e || o != object)
         {
            failures.push(what + ": teammate " + p + " (want " + player + "), enemy " + e + " (want false), object "
               + o + " (want " + object + ")");
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
            player = movie.iconControl.getChildAt(movie.iconControl.numChildren - 1);
            movie.create_player_icon(2,0,1);
            enemy = movie.iconControl.getChildAt(movie.iconControl.numChildren - 1);
            movie.set_enemy_icon_ids([2]);
            movie.create_player_icon(40,1,1);
            objectMarker = movie.iconControl.getChildAt(movie.iconControl.numChildren - 1);
            movie.set_player_icon_type(40,1);
            expect("a plain match",true,true);
            movie.force_interface_hidden(true);
            expect("Blackout",true,false);
            movie.hide_interface(true);
            expect("Blackout, F11 (hide_interface) pressed: it does nothing",true,false);
            movie.force_interface_hidden(false);
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
