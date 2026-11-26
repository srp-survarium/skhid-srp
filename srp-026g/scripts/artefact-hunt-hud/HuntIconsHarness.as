package
{
   import flash.display.*;
   import flash.events.*;
   import flash.net.URLRequest;
   import flash.system.*;

   // Artifact Hunt's carriers over their heads (docs/notes/artefact_hunt.md "The carrier") with the
   // player icons movie's own root, fed as the game feeds it: the match's markers and enemy list
   // (set_enemy_icon_ids), the mode's root.srp_show_carriers(true), then render_name's props with the
   // game's carrier mark (modifier_visible). Checks:
   // - a teammate carrying shows as ever, his mark the artefact's orb, not the battery's box;
   // - an enemy never shows, carrying or not, by props or by set_player_icon_container: the
   //   minimaps alone show the carrier to the other team (the owner, 2026-11-26: "player with art is
   //   still seen in ui, not only on map");
   // - a replay (srp_show_enemies(true)): an enemy carrier shows, red, his mark the orb;
   // - another mode (srp_show_carriers(false)): an enemy carrier hidden, a teammate's mark the box;
   // - told before the root's frame 1 made its layer, the layer takes it.
   // "HUNT SHOT icons" for check.sh's screenshot; ends with "HUNT RESULT pass" or "... fail: ...".
   public class HuntIconsHarness extends Sprite
   {
      private var icons:Loader = new Loader();
      private var failures:Array = [];
      private var root_:*;
      private var frames:int = 0;
      private var phase:int = 0;

      public function HuntIconsHarness()
      {
         icons.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
            function(e:UncaughtErrorEvent):void { e.preventDefault(); failures.push(String(e.error)); });
         icons.load(new URLRequest(loaderInfo.parameters.movie || "player_icons.swf"),new LoaderContext(false,new ApplicationDomain()));
         addEventListener(Event.ENTER_FRAME,tick);
      }

      private function check(ok:Boolean, message:String) : void
      {
         trace("HUNT " + (ok ? "ok " : "FAIL ") + message);
         if(!ok) failures.push(message);
      }

      private static function props(id:int, name:String, rgb:uint, carry:* = undefined) : Object
      {
         var p:Object = {id:id,visible:true,x:160 + id * 130,y:300,z:1.6,name:name,name_visible:true,rgb:rgb};
         if(carry !== undefined) p.modifier_visible = carry;
         return p;
      }

      // The marker's arrow (every part of it, "renderer") and name colours, hex: "<arrow>/<name>", or
      // "mixed" for parts of different colours.
      private static function colour(marker:*) : String
      {
         var arrow:String = null;
         for(var i:int = 0; i < marker.icon.numChildren; i++)
         {
            var part:* = marker.icon.getChildAt(i);
            if(part.name != "renderer") continue;
            var c:String = part.transform.colorTransform.color.toString(16);
            arrow = arrow == null || arrow == c ? c : "mixed";
         }
         return arrow + "/" + marker.textField.textColor.toString(16);
      }

      private function setup(root:*) : void
      {
         root.create_player_icon(1,0,1);
         root.create_player_icon(2,0,1);
         root.create_player_icon(3,0,1);
         // the game stops each marker on its kind (set_player_icon_type), or the arrow runs through its frames
         for(var id:int = 1; id <= 3; id++) root.set_player_icon_type(id,0);
         root.set_enemy_icon_ids([2,3]);
      }

      private function tick(e:Event) : void
      {
         try
         {
            frames++;
            if(phase == 0)
            {
               if(icons.content == null || Object(icons.content).iconControl == null) return;
               root_ = icons.content;
               addChild(icons);
               setup(root_);
               root_.srp_show_carriers(true);
               // 1 a teammate carrying, 2 an enemy carrying, 3 an enemy not; the game gave the enemy
               // carrier his own team's white (the owner's test, 2026-11-21)
               root_.set_player_icons_props([props(1,"Wolf",0xFFFFFF,true),props(2,"Raven",0xFFFFFF,true),props(3,"Fox",0xFF0000,false)]);
               var ally:* = root_.iconControl.getChildAt(1);
               var enemy:* = root_.iconControl.getChildAt(2);
               var other:* = root_.iconControl.getChildAt(3);
               check(ally.visible && ally.artefactMarkShown && !ally.icon_box.visible,"a teammate carrying: shown, the artefact's mark, not the box");
               check(!enemy.visible,"an enemy carrying: never shown (the minimaps alone show him)");
               check(!other.visible,"an enemy not carrying: never shown");
               check(colour(ally) == "ffffff/ffffff","the teammate carrier white: " + colour(ally));
               root_.set_player_icon_visible(2,true);
               check(!enemy.visible,"an enemy carrier at set_player_icon_visible: still hidden");
               phase = 4;
               frames = 0;
               return;
            }
            if(phase == 4)
            {
               // the names show after the markers' delay (500 ms)
               if(frames < 50) return;
               trace("HUNT SHOT icons");
               phase = 1;
               frames = 0;
               return;
            }
            if(phase == 1)
            {
               if(frames < 75) return;
               enemy = root_.iconControl.getChildAt(2);
               root_.set_player_icons_props([props(2,"Raven",0xFF0000,false)]);
               root_.set_player_icon_container(2,true);
               root_.set_player_icons_props([props(2,"Raven",0xFF0000)]);
               check(!enemy.visible,"an enemy marked by set_player_icon_container, then props: still hidden");
               // a replay shows every enemy, the carrier with the artefact's mark, red though sent white
               root_.srp_show_enemies(true);
               root_.set_player_icons_props([props(2,"Raven",0xFFFFFF,true)]);
               check(enemy.visible && enemy.artefactMarkShown && !enemy.icon_box.visible,"a replay: the enemy carrier shown with the artefact's mark");
               check(colour(enemy) == "ff0000/ff0000","a replay's enemy carrier red, though sent white: " + colour(enemy));
               root_.set_player_color(2,255,255,255);
               check(colour(enemy) == "ff0000/ff0000","and stays red at set_player_color white: " + colour(enemy));
               root_.srp_show_enemies(false);
               // another mode: the battery's game
               root_.srp_show_carriers(false);
               root_.set_player_icons_props([props(1,"Wolf",0xFFFFFF,true),props(2,"Raven",0xFF0000,true)]);
               ally = root_.iconControl.getChildAt(1);
               check(!enemy.visible,"another mode: an enemy carrier never shown");
               check(ally.visible && ally.icon_box.visible && !ally.artefactMarkShown,"another mode: a teammate's mark the battery's box");
               var fresh:* = new (icons.contentLoaderInfo.applicationDomain.getDefinition("player_icons_fla.MainTimeline") as Class)();
               var made:Boolean = fresh.iconControl != null;
               fresh.srp_show_carriers(true);
               phase = 2;
               frames = 0;
               addChild(fresh);
               root_ = fresh;
               check(!made,"a fresh root has no layer before its first frame");
               return;
            }
            if(phase == 2)
            {
               if(root_.iconControl == null) return;
               check(root_.iconControl.carriersShown,"told before frame 1: the layer made after takes it");
               trace("HUNT RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
               phase = 3;
            }
         }
         catch(error:Error)
         {
            trace("HUNT RESULT fail: " + error + " " + error.getStackTrace());
            phase = 3;
         }
      }
   }
}
