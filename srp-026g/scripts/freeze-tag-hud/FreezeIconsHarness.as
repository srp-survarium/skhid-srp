package
{
   import flash.display.*;
   import flash.events.*;
   import flash.net.URLRequest;
   import flash.system.*;

   // Freeze Tag's frozen names (docs/notes/freeze-tag.md, the HUD) with the player icons movie's own
   // root, fed as the game feeds it: the markers, render_name's props (name, rgb) and patch 160's
   // root.freeze_tag_markers(mask). Four tags: a teammate and an enemy, each as he is and frozen.
   // Over background.png in the work folder when there is one (a game screen), else a sky over
   // ground drawn here. Checks:
   // - a frozen place's name is ice blue, no label is written over it, the others keep their colour;
   // - a name or a colour the game sets while frozen keeps the blue, and the colour comes back on
   //   thaw, the newest one; a marker made while its place is frozen is blue from the start;
   // - the native label stays the game's.
   // "FREEZE SHOT tags" for check.sh's screenshot; ends with "FREEZE RESULT pass" or "... fail: ...".
   public class FreezeIconsHarness extends Sprite
   {
      // The movie's own frozen colour (HUDMarker.FROZEN_NAME_COLOR), read when it has loaded.
      private var ICE:uint;
      private static const ALLY:uint = 0xFFFFFF;
      private static const ENEMY:uint = 0xFF0000;
      private var icons:Loader = new Loader();
      private var backdrop:Loader = new Loader();
      private var failures:Array = [];
      private var root_:*;
      private var phase:int = 0;
      private var frames:int = 0;

      public function FreezeIconsHarness()
      {
         stage.scaleMode = StageScaleMode.NO_SCALE;
         stage.align = StageAlign.TOP_LEFT;
         var sky:Shape = new Shape();
         var w:Number = stage.stageWidth, h:Number = stage.stageHeight;
         sky.graphics.beginFill(0xC9D3DE);
         sky.graphics.drawRect(0,0,w,h * 0.5);
         sky.graphics.beginFill(0x5C5A49);
         sky.graphics.drawRect(0,h * 0.5,w,h * 0.5);
         addChild(sky);
         addChild(backdrop);
         backdrop.contentLoaderInfo.addEventListener(Event.COMPLETE,function(e:Event):void {
            backdrop.width = w;
            backdrop.height = h;
         });
         backdrop.contentLoaderInfo.addEventListener(IOErrorEvent.IO_ERROR,function(e:Event):void {});
         backdrop.load(new URLRequest("background.png"));
         icons.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
            function(e:UncaughtErrorEvent):void { e.preventDefault(); failures.push(String(e.error)); });
         icons.load(new URLRequest("player_icons.swf"),new LoaderContext(false,new ApplicationDomain()));
         addEventListener(Event.ENTER_FRAME,tick);
      }

      private function check(ok:Boolean, message:String) : void
      {
         trace("FREEZE " + (ok ? "ok " : "FAIL ") + message);
         if(!ok) failures.push(message);
      }

      // Two rows: the tags against the sky (row 0) and against the ground (row 1).
      private function props(id:int, name:String, rgb:uint) : Object
      {
         var w:Number = stage.stageWidth, h:Number = stage.stageHeight;
         var column:int = (id - 1) % 4;
         var row:int = int((id - 1) / 4);
         return {id:id,visible:true,x:w * (0.14 + column * 0.24),y:h * (row == 0 ? 0.28 : 0.72),
            z:h / 720,name:name,name_visible:true,rgb:rgb};
      }

      private function all() : Array
      {
         var list:Array = [];
         for(var row:int = 0; row < 2; row++)
         {
            list.push(props(1 + row * 4,"Wolf",ALLY),props(2 + row * 4,"Lynx",ALLY),
               props(3 + row * 4,"Raven",ENEMY),props(4 + row * 4,"Fox",ENEMY));
         }
         return list;
      }

      private function marker(id:int) : *
      {
         return root_.iconControl.getChildAt(id);
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
               ICE = (icons.contentLoaderInfo.applicationDomain.getDefinition("survarium.controls.HUDMarker") as Class).FROZEN_NAME_COLOR;
               trace("FREEZE colour 0x" + ICE.toString(16).toUpperCase());
               addChild(icons);
               for(var id:int = 1; id <= 8; id++) root_.create_player_icon(id,0,1);
               root_.set_player_icons_props(all());
               root_.set_player_icon_label(3,"");
               // Lynx and Fox frozen, in both rows.
               var mask:uint = (1 << 2) | (1 << 4) | (1 << 6) | (1 << 8);
               root_.freeze_tag_markers(mask);
               root_.set_player_icons_props(all());
               check(marker(2).textField.textColor == ICE && marker(4).textField.textColor == ICE,
                  "a frozen teammate's and enemy's names ice blue");
               check(marker(1).textField.textColor == ALLY && marker(3).textField.textColor == ENEMY,
                  "the others keep their colour");
               check(marker(2).textField.text == "Lynx" && marker(4).textField.text == "Fox",
                  "the names are the players' own");
               check(!("freeze_tag_label" in root_) && marker(2).numChildren == marker(1).numChildren,
                  "no label is written over a frozen player");
               trace("FREEZE SHOT tags");
               phase = 1;
               frames = 0;
               return;
            }
            if(phase == 1)
            {
               if(frames < 180) return;
               // While frozen the game renames him and gives his team's colour again.
               root_.set_player_icon_name(2,"Lynx2");
               root_.set_player_color(2,0,255,0);
               check(marker(2).textField.textColor == ICE && marker(2).textField.text == "Lynx2",
                  "a name and a colour set while frozen: still ice blue");
               root_.freeze_tag_markers((1 << 4) | (1 << 6) | (1 << 8));
               check(marker(2).textField.textColor == 0x00FF00,"thawed: the newest own colour comes back");
               check(marker(4).textField.textColor == ICE,"the others still frozen");
               root_.freeze_tag_markers(0);
               check(marker(4).textField.textColor == ENEMY && marker(6).textField.textColor == ALLY,
                  "the mask cleared (another mode, the round's restart): every own colour back");
               root_.freeze_tag_markers(1 << 12);
               root_.create_player_icon(12,0,1);
               root_.set_player_icons_props([props(12,"Bear",ALLY)]);
               check(marker(9).textField.textColor == ICE,"a marker made while its place is frozen: ice blue");
               root_.freeze_tag_markers(0);
               trace("FREEZE RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
               phase = 2;
            }
         }
         catch(error:Error)
         {
            trace("FREEZE RESULT fail: " + error + " " + error.getStackTrace());
            phase = 2;
         }
      }
   }
}
