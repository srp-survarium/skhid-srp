package
{
   import flash.display.*;
   import flash.events.*;
   import flash.net.URLRequest;
   import flash.system.*;

   // The markers over thrown grenades (docs/notes/grenade-icons.md) with the player icons movie's own
   // root, fed as the game feeds it: initialize_match's six grenade markers (ids 47..52, a marker,
   // scale grenade_icon_size 0.5, type 0), then update_player_icons' props for a grenade 4 m away
   // (z 2.94, alpha 251, white). CONFIG::kinds: each grenade's kind told (set_player_icon_type
   // MarkerType.GRENADE, SMOKE, TOXIC_SMOKE); else the stock feed, which tells none.
   // Columns: a teammate's frag, smoke and toxic smoke, and an enemy's grenade (the game marks only
   // its own team's: none); rows on a dark and on a light ground. "GRENADES SHOT" for check.sh's
   // screenshot, then (kinds) a marker's type changed back and forth; "GRENADES RESULT pass" or
   // "... fail: ...".
   public class GrenadeIconsHarness extends Sprite
   {
      public static const CELL:int = 160;
      public static const GROUNDS:Array = [0x30373B,0xB4BEC4];
      public static const FIRST_GRENADE:int = 47;
      // MarkerType: GRENADE, TRAP, SMOKE, TOXIC_SMOKE
      public static const GRENADE:uint = 0;
      public static const TRAP:uint = 1;
      public static const SMOKE:uint = 3;
      public static const TOXIC_SMOKE:uint = 4;
      public static const KINDS:Array = [GRENADE,SMOKE,TOXIC_SMOKE];

      private var icons:Loader = new Loader();
      private var failures:Array = [];
      private var frames:int = 0;
      private var done:Boolean = false;

      public function GrenadeIconsHarness()
      {
         for(var row:int = 0; row < GROUNDS.length; row++)
         {
            graphics.beginFill(GROUNDS[row]);
            graphics.drawRect(0,row * CELL,CELL * 4,CELL);
            graphics.endFill();
         }
         icons.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
            function(e:UncaughtErrorEvent):void { e.preventDefault(); failures.push(String(e.error)); });
         icons.load(new URLRequest("player_icons.swf"),new LoaderContext(false,new ApplicationDomain()));
         addEventListener(Event.ENTER_FRAME,tick);
      }

      private function check(ok:Boolean, message:String) : void
      {
         trace("GRENADES " + (ok ? "ok " : "FAIL ") + message);
         if(!ok) failures.push(message);
      }

      // What a marker shows: "icon <frame>" (the stock icon), "mark" (one drawn mark beside a hidden
      // icon), or what else it holds.
      private static function shows(marker:*) : String
      {
         var marks:int = marker.numChildren - 1;
         if(marker.icon.visible && marks == 0) return "icon " + marker.icon.currentFrame;
         if(!marker.icon.visible && marks == 1) return "mark";
         return "icon visible " + marker.icon.visible + ", " + marks + " other children";
      }

      private function tick(e:Event) : void
      {
         if(done) return;
         try
         {
            var root_:* = icons.content;
            // the frames count from the first the movie's root has its layer (a slow load waits)
            if(root_ == null || root_.iconControl == null) return;
            frames++;
            var layer:* = root_.iconControl;
            if(frames == 1)
            {
               addChild(icons);
               var props:Array = [];
               for(var slot:int = 0; slot < 6; slot++)
               {
                  var id:int = FIRST_GRENADE + slot;
                  root_.create_player_icon(id,1,0.5);
                  root_.set_player_icon_type(id,GRENADE);
                  CONFIG::kinds
                  {
                     root_.set_player_icon_type(id,KINDS[slot % 3]);
                  }
                  props.push({id:id,visible:true,x:(slot % 3) * CELL + CELL / 2,y:int(slot / 3) * CELL + CELL * 0.62,z:2.94,a:251,rgb:0xFFFFFF});
               }
               root_.set_player_icons_props(props);
               for(slot = 0; slot < 6; slot++)
               {
                  var marker:* = layer.getChildAt(layer.numChildren - 6 + slot);
                  check(marker.visible,"slot " + slot + " shown");
                  var expected:String = "icon 1";
                  CONFIG::kinds
                  {
                     expected = KINDS[slot % 3] == GRENADE ? "icon 1" : "mark";
                  }
                  check(shows(marker) == expected,"slot " + slot + ": " + expected + ", shows " + shows(marker));
               }
               return;
            }
            if(frames == 10)
            {
               trace("GRENADES SHOT");
               return;
            }
            if(frames == 120)
            {
               CONFIG::kinds
               {
                  // a slot's grenade changes from frame to frame: no mark is left behind or doubled
                  var first:* = layer.getChildAt(layer.numChildren - 6);
                  for each(var step:Array in [[SMOKE,"mark"],[TOXIC_SMOKE,"mark"],[GRENADE,"icon 1"],[TOXIC_SMOKE,"mark"],[TRAP,"icon 2"],[SMOKE,"mark"],[GRENADE,"icon 1"]])
                  {
                     root_.set_player_icon_type(FIRST_GRENADE,step[0]);
                     check(shows(first) == step[1],"type " + step[0] + ": " + step[1] + ", shows " + shows(first));
                  }
               }
               trace("GRENADES RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
               done = true;
            }
         }
         catch(error:Error)
         {
            trace("GRENADES RESULT fail: " + error + " " + error.getStackTrace());
            done = true;
         }
      }
   }
}
