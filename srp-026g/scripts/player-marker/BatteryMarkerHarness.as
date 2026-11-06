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

   // Exercise the actual movie's public feed, including partial property updates.
   // Combined hide/carry updates are a UI contract test, not an established
   // reproduction of a particular native gameplay sequence.
   public class BatteryMarkerHarness extends Sprite
   {
      private var loader:Loader = new Loader();
      private var movie:MovieClip;
      private var frame:int = 0;
      private var failures:Array = [];

      public function BatteryMarkerHarness()
      {
         loader.contentLoaderInfo.addEventListener(Event.INIT,function(e:Event):void
         {
            movie = loader.content as MovieClip;
            addChild(loader);
            addEventListener(Event.ENTER_FRAME,tick);
         });
         loader.contentLoaderInfo.addEventListener("ioError",function(e:Event):void
         {
            finish("fail: " + e);
         });
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void
         {
            e.preventDefault();
            finish("fail: " + e.error);
         });
         loader.load(new URLRequest("player_icons.swf"),new LoaderContext(false,new ApplicationDomain()));
      }

      private function make(id:uint, kind:uint = 0) : Object
      {
         movie.create_player_icon(id,kind,1);
         return movie.iconControl.getChildAt(movie.iconControl.numChildren - 1);
      }

      private function send(props:Object) : void
      {
         movie.set_player_icons_props([props]);
      }

      private function check(label:String, condition:Boolean) : void
      {
         if(!condition)
         {
            failures.push(label);
         }
         trace("BATTERY " + label + ": " + (condition ? "pass" : "FAIL"));
      }

      private function tick(e:Event) : void
      {
         if(++frame != 3)
         {
            return;
         }
         var player:Object = make(12);
         send({id:12,visible:true,modifier_visible:true,x:100,y:100,z:1});
         check("visible pickup",player.visible && player.icon_box.visible);
         send({id:12,modifier_visible:false});
         check("visible drop",player.visible && !player.icon_box.visible);

         send({id:12,modifier_visible:true});
         send({id:12,visible:false});
         send({id:12,visible:true,modifier_visible:false});
         check("reveal with current carry state",player.visible && !player.icon_box.visible);

         send({id:12,modifier_visible:true});
         send({id:12,visible:false,modifier_visible:false});
         check("clear does not reveal hidden player",!player.visible);
         send({id:12,visible:true});
         check("no stale battery after hidden drop",player.visible && !player.icon_box.visible);

         send({id:12,modifier_visible:false});
         send({id:12,visible:false,modifier_visible:true});
         check("pickup does not reveal hidden player",!player.visible);
         send({id:12,visible:true});
         check("battery present after hidden pickup",player.visible && player.icon_box.visible);
         send({id:12,modifier_visible:true});
         send({id:12,x:150,y:100,z:1});
         check("omitted modifier preserves carry state",player.icon_box.visible);
         send({id:12,modifier_visible:true});
         check("repeated modifier preserves carry state",player.icon_box.visible);

         movie.hide_interface(true);
         send({id:12,modifier_visible:false});
         check("state update does not reveal whole HUD",!movie.visible);
         movie.hide_interface(false);
         check("whole HUD hiding still consumes state",!player.icon_box.visible);

         var enemy:Object = make(13);
         movie.set_enemy_icon_ids([13]);
         send({id:13,visible:true,modifier_visible:true});
         check("enemy remains hidden",!enemy.visible);
         send({id:13,modifier_visible:false});
         check("enemy clear remains hidden",!enemy.visible);
         movie.set_enemy_icon_ids([]);
         send({id:13,visible:true});
         check("allowed marker reveals with current state",enemy.visible && !enemy.icon_box.visible);

         var objectMarker:Object = make(40,1);
         movie.force_interface_hidden(true);
         send({id:40,visible:true,modifier_visible:true});
         check("hidden world object remains hidden",!objectMarker.visible);
         movie.force_interface_hidden(false);
         send({id:40,visible:true,modifier_visible:false});
         check("world object shows normally",objectMarker.visible);

         var count:int = movie.iconControl.numChildren;
         send({id:999,visible:true,modifier_visible:true});
         check("unknown marker is ignored",movie.iconControl.numChildren == count);
         finish(failures.length ? "fail: " + failures.join("; ") : "pass");
      }

      private function finish(result:String) : void
      {
         removeEventListener(Event.ENTER_FRAME,tick);
         trace("BATTERY RESULT " + result);
         fscommand("quit");
      }
   }
}
