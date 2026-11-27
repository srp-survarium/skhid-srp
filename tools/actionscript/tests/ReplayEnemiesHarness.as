package
{
   import flash.display.*;
   import flash.events.*;
   import flash.net.URLRequest;
   import flash.system.*;
   import flash.text.TextField;
   import flash.utils.getTimer;

   // The enemies over their heads in a replay (srp-026g/docs/notes/replay-map.md) with the player icons
   // movie's own root (player_icons.swf), fed as the game feeds it: initialize_match's markers and enemy
   // list (set_enemy_icon_ids), exe patch 135's srp_show_enemies, then render_name's props (visible,
   // place, name, name_visible while Alt is held, rgb: an enemy's 0xFF0000). Checks:
   // - a live match (srp_show_enemies(false)): an ally's marker shows, an enemy's never;
   // - a replay (srp_show_enemies(true), told before the enemy list, and the list sent again later):
   //   the enemy's marker shows, its name red, shown with Alt after the marker's delay; the ally's as
   //   in a live match;
   // - told before the movie's first frame made its layer: the layer made after takes it.
   // Prints "ENEMIES ..." lines and ends with "ENEMIES RESULT pass" or "ENEMIES RESULT fail: ...".
   public class ReplayEnemiesHarness extends Sprite
   {
      private var icons:Loader = new Loader();
      private var failures:Array = [];
      private var phase:int = 0;
      private var since:int = 0;
      private var root_:*;
      private var replayEnemy:*;
      private var fresh:*;
      private var made:Boolean;

      public function ReplayEnemiesHarness()
      {
         icons.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
            function(e:UncaughtErrorEvent):void { e.preventDefault(); failures.push(String(e.error)); });
         icons.load(new URLRequest("player_icons.swf"),new LoaderContext(false,new ApplicationDomain()));
         addEventListener(Event.ENTER_FRAME,tick);
      }

      private function check(ok:Boolean, message:String):void
      {
         trace("ENEMIES " + (ok ? "ok " : "FAIL ") + message);
         if(!ok) failures.push(message);
      }

      // The layer's markers, by creation (child 0 is Research's layer).
      private function marker(n:int):*
      {
         return root_.iconControl.getChildAt(n + 1);
      }

      private static function props(id:int, name:String, rgb:uint):Object
      {
         return {id:id,visible:true,x:100 + id * 60,y:200,z:1,name:name,name_visible:true,rgb:rgb};
      }

      private function tick(e:Event):void
      {
         try
         {
            if(phase == 0)
            {
               if(icons.content == null || Object(icons.content).iconControl == null) return;
               root_ = icons.content;
               addChild(icons);
               live();
               replay();
               since = getTimer();
               phase = 1;
               return;
            }
            if(phase == 1)
            {
               if(getTimer() - since < 300) return;
               var text:TextField = replayEnemy.textField;
               check(text.visible && text.text == "[KLAN] Enemy","a replay, Alt held: the enemy's name shown after the "
                  + "marker's delay (" + text.visible + ", " + text.text + ")");
               check(marker(2).textField.visible,"a replay, Alt held: the ally's name shown too");
               fresh = new (icons.contentLoaderInfo.applicationDomain.getDefinition("player_icons_fla.MainTimeline") as Class)();
               made = fresh.iconControl != null;
               fresh.srp_show_enemies(true);
               addChild(fresh);
               since = getTimer();
               phase = 2;
               return;
            }
            if(phase == 2)
            {
               if(fresh.iconControl == null && getTimer() - since < 1000) return;
               check(!made && fresh.iconControl != null && fresh.iconControl.enemiesShown,
                  "told before its frame 1 made the layer: the layer takes it");
            }
            removeEventListener(Event.ENTER_FRAME,tick);
         }
         catch(error:Error)
         {
            failures.push(error.getStackTrace());
            removeEventListener(Event.ENTER_FRAME,tick);
         }
         trace("ENEMIES RESULT " + (failures.length == 0 ? "pass" : "fail: " + failures.join("; ")));
      }

      private function live():void
      {
         root_.set_player_time_interval(1);
         root_.create_player_icon(1,0);
         root_.create_player_icon(2,0);
         root_.srp_show_enemies(false);
         root_.set_enemy_icon_ids([2]);
         root_.set_player_icons_props([props(1,"Ally",0xFFFFFF),props(2,"Enemy",0xFF0000)]);
         check(marker(0).visible,"a live match: the ally's marker shown");
         check(!marker(1).visible,"a live match: the enemy's marker never shown");
         root_.set_player_icon_visible(2,true);
         check(!marker(1).visible,"a live match: nor by set_player_icon_visible");
      }

      private function replay():void
      {
         root_.create_player_icon(3,0);
         root_.create_player_icon(4,0);
         root_.set_player_time_interval(1);
         root_.srp_show_enemies(true);
         root_.set_enemy_icon_ids([4]);
         root_.set_player_icons_props([props(3,"Ally",0xFFFFFF),props(4,"[KLAN] Enemy",0xFF0000)]);
         replayEnemy = marker(3);
         check(marker(2).visible,"a replay: the ally's marker shown");
         check(replayEnemy.visible,"a replay: the enemy's marker shown");
         check(replayEnemy.textField.textColor == 0xFF0000,"a replay: the enemy's name red ("
            + replayEnemy.textField.textColor.toString(16) + ")");
         check(marker(2).textField.textColor == 0xFFFFFF,"a replay: the ally's name white");
         root_.set_enemy_icon_ids([3,4]);
         root_.set_player_icon_visible(4,false);
         root_.set_player_icon_visible(4,true);
         check(replayEnemy.visible && marker(2).visible,"a replay: the enemy list sent again changes nothing");
      }
   }
}
