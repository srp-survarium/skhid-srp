package
{
   import flash.display.*;
   import flash.events.*;
   import flash.geom.Matrix;
   import flash.geom.Point;
   import flash.net.URLRequest;
   import flash.system.*;
   import flash.text.TextField;
   import flash.utils.getTimer;

   public class ResearchUiHarness extends Sprite
   {
      private var hud:Loader = new Loader();
      private var icons:Loader = new Loader();
      private var marker:*;
      private var layer:*;
      private var world:Sprite;
      private var units:*;
      private var topSwords:DisplayObject;
      private var phase:int = 0;
      private var fadeStart:int;
      private var lastAlpha:Number;
      private function packet(aiming:Boolean, legacy:Boolean = false):Array
      {
         var a:Array = [0,1,0,1,200,200,20,1,1,70,0,1,1];
         if(!legacy) a.push(aiming ? 1 : 0);
         return a;
      }
      private var swords:Sprite;
      private var started:int;
      private var sample:int;
      private var rate:int = 30;
      private var initialized:Boolean = false;

      public function ResearchUiHarness()
      {
         for each(var loader:Loader in [hud,icons])
         {
            // The game's HUD root expects game callbacks; test its controls alone.
            loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
               function(e:UncaughtErrorEvent):void { e.preventDefault(); });
         }
         hud.load(new URLRequest("hud.swf"),new LoaderContext(false,new ApplicationDomain()));
         icons.load(new URLRequest("player_icons.swf"),new LoaderContext(false,new ApplicationDomain()));
         addEventListener(Event.ENTER_FRAME,tick);
      }

      private function check(ok:Boolean, message:String):void
      {
         if(!ok) throw new Error(message);
         trace("PASS " + message);
      }

      private function startPulse():void
      {
         stage.frameRate = rate;
         marker.update(0,1,1,70,0,true,0,20,false);
         marker.update(0,1,1,70,0,true,0,20,true);
         units.update([{cp_id:0,owner:1,capturing:1,progress:70,inside_0:1,inside_1:1,local:true,left:0}],0);
         topSwords = Sprite(units.getChildAt(0)).getChildAt(0);
         layer.research_markers(packet(true));
         started = getTimer();
         sample = 1;
         check(swords.visible && swords.alpha == 1,"swords immediately visible at " + rate + " FPS");
      }

      private function checkCountdownLayout():void
      {
         var domain:ApplicationDomain = hud.contentLoaderInfo.applicationDomain;
         var cls:Class = domain.getDefinition("survarium.controls.ResearchUnits") as Class;
         var points:* = new cls();
         points.update([{cp_id:0,owner:2,capturing:0,progress:40,inside_0:1,inside_1:0,local:true,left:7000}],0);
         points.x = 400; points.y = 80; addChild(points);
         var indicator:Sprite = new Sprite();
         indicator.x = 400; indicator.y = 30;
         indicator.scaleX = indicator.scaleY = 0.8;
         addChild(indicator);
         cls = domain.getDefinition("DefaultLabel33") as Class;
         var respawn:* = new cls();
         indicator.addChild(respawn);
         respawn.autoSize = "center";
         respawn.text = "Respawn in 00:10";
         respawn.validateNow();
         var oldSize:Object = respawn.textField.defaultTextFormat.size;
         var oldY:Number = respawn.y;
         cls = domain.getDefinition("SrvContextInfoPregame") as Class;
         var pregame:* = new cls();
         pregame.layoutData = {alignH:"center",alignV:"top"};
         pregame.contextText.text = "Match starts in 00:10";
         addChild(pregame);
         cls = domain.getDefinition("survarium.controls.ResearchCountdownLayout") as Class;
         var placement:* = new cls(respawn,pregame);
         placement.place(points);
         var countdown:TextField = indicator.getChildAt(indicator.numChildren - 1) as TextField;
         check(countdown != null && !respawn.visible && respawn.textField.defaultTextFormat.size == oldSize,
            "Research uses its own countdown without resizing the native label");
         check(pregame.contextText.getBounds(this).top >= points.getBounds(this).bottom + 3,
            "pre-match countdown clears the Research icons and capture timers");
         check(countdown.getBounds(this).top >= pregame.contextText.getBounds(this).bottom + 5,
            "simultaneous countdowns do not overlap at a scaled indicator");
         check(countdown.defaultTextFormat.size == 18 && pregame.contextText.defaultTextFormat.size == 18,
            "Research countdown text is larger");
         pregame.visible = false;
         placement.place(points);
         check(countdown.getBounds(this).top >= points.getBounds(this).bottom + 3,
            "respawn countdown keeps its own row after pre-match ends");
         var withCapture:Number = countdown.getBounds(this).top;
         points.update([{cp_id:0,owner:0,capturing:0,progress:100,inside_0:0,inside_1:0,local:false,left:0}],0);
         placement.place(points);
         var withoutCapture:Number = countdown.getBounds(this).top;
         check(withoutCapture < withCapture - 10,
            "a finished capture removes the hidden timer's empty row above respawn");
         var iconBottom:Number = points.getChildAt(0).getBounds(this).bottom;
         check(withoutCapture >= iconBottom + 3 && withoutCapture <= iconBottom + 6,
            "respawn sits close below the points when no capture timer is visible");
         for each(var message:String in ["","Respawn in 00:10","Respawn in 00:09","","Respawn in 00:10"])
         {
            respawn.text = message;
            placement.place(points);
            check(countdown.text == message && countdown.visible == (message.length > 0),
               "death/respawn text changes synchronously: " + message);
            var width:Number = countdown.getBounds(this).width;
            var left:Number = countdown.getBounds(this).left;
            // CLIK can validate after our frame handler. It must not resize or
            // shift the Research countdown on this frame or a subsequent death.
            respawn.invalidateSize();
            respawn.validateNow();
            check(Math.abs(countdown.getBounds(this).width - width) < 0.01 &&
               Math.abs(countdown.getBounds(this).left - left) < 0.01,
               "late native validation cannot squeeze or shift the countdown");
            if(message.length > 0)
            {
               check(countdown.width >= countdown.textWidth && countdown.textWidth > 60 &&
                  Math.abs(countdown.getBounds(this).left + width / 2 - points.x) <= 0.5,
                  "first death frame has full-width centred text");
            }
         }
         for each(var frame:int in [1,10,11,20])
         {
            respawn.gotoAndStop(frame);
            respawn.validateNow();
            check(countdown.text == "Respawn in 00:10" && countdown.width >= countdown.textWidth &&
               Math.abs(countdown.getBounds(this).width - width) < 0.01,
               "native label animation cannot squeeze Research text at frame " + frame);
         }
         placement.restore();
         check(countdown.parent == null && respawn.visible && respawn.textField.defaultTextFormat.size == oldSize && respawn.y == oldY &&
            pregame.layoutData.alignH == "center" && pregame.layoutData.alignV == "top",
            "leaving Research restores the other modes' timer style and layout");
         removeChild(points); removeChild(indicator); removeChild(pregame);
      }

      private function checkResearchCentering():void
      {
         var cls:Class = hud.contentLoaderInfo.applicationDomain.getDefinition("survarium.controls.ResearchUnits") as Class;
         var points:* = new cls();
         addChild(points);
         var container:Sprite = new Sprite();
         container.x = 37;
         container.scaleX = container.scaleY = 0.8;
         addChild(container);
         var indicator:Sprite = new Sprite();
         indicator.scaleX = indicator.scaleY = 0.75;
         indicator.y = 30;
         container.addChild(indicator);
         for each(var initialX:Number in [400,900])
         {
            indicator.x = initialX;
            points.followIndicator(indicator,17,45);
            check(Math.abs(indicator.localToGlobal(new Point(17,0)).x - stage.stageWidth / 2) < 0.01,
               "score bars centre before resize from initial x=" + initialX);
            check(Math.abs(points.localToGlobal(new Point()).x - stage.stageWidth / 2) <= 0.5,
               "points stay centred with scaled and translated parents");
         }
         // A label or score widening must not move the group's centre.
         indicator.graphics.beginFill(0);
         indicator.graphics.drawRect(-400,0,900,10);
         indicator.graphics.endFill();
         points.followIndicator(indicator,17,45);
         check(Math.abs(indicator.localToGlobal(new Point(17,0)).x - stage.stageWidth / 2) < 0.01,
            "changing indicator bounds does not move Research off centre");
         removeChild(points); removeChild(container);
      }

      private function tick(e:Event):void
      {
         try
         {
            if(!initialized)
            {
               if(hud.content == null || icons.content == null) return;
               initialized = true;
               checkCountdownLayout();
               checkResearchCentering();
               var cls:Class = hud.contentLoaderInfo.applicationDomain.getDefinition("survarium.controls.ResearchUnits") as Class;
               units = new cls();
               addChild(units);
               var point:Object = {cp_id:0,owner:2,capturing:0,progress:40,inside_0:1,inside_1:0,local:true,left:7000};
               for each(var owner:uint in [2,1,0])
               {
                  point.owner = owner;
                  units.update([point],0);
                  check(units.numChildren == 2,"no caption child for owner " + owner);
                  check(Math.abs(units.getChildAt(0).scaleX - 34 / 24) < 0.001,"inside icon remains larger");
                  var timer:TextField = units.getChildAt(1) as TextField;
                  check(timer != null && timer.visible && timer.text == "0:07","capture timer retains its font and text");
               }
               point.local = false;
               point.left = 0;
               units.update([point],0);
               check(units.getChildAt(0).scaleX == 1 && !timer.visible,"leaving point restores size and hides timer");
               cls = icons.contentLoaderInfo.applicationDomain.getDefinition("survarium.controls.ResearchMarker") as Class;
               marker = new cls();
               addChild(marker);
               swords = Sprite(Sprite(marker.getChildAt(0)).getChildAt(2));
               marker.update(0,1,1,70,7000,false,0,20,false);
               var outside:Number = marker.scaleX;
               marker.update(0,1,1,70,7000,true,0,20,false);
               check(Math.abs(marker.scaleX / outside - 34 / 24) < 0.001,"world marker remains larger inside");
               check(!swords.visible,"no swords without an opponent contesting");
               cls = icons.contentLoaderInfo.applicationDomain.getDefinition("survarium.controls.HUDIconLayer") as Class;
               layer = new cls(); addChild(layer);
               world = Sprite(layer.getChildAt(0));
               layer.research_markers(packet(false));
               check(world.alpha == 1 && world.numChildren == 1,"world layer starts visible");
               layer.objectsHidden = true;
               check(!world.visible,"hide-interface key hides Research world markers");
               layer.objectsHidden = false;
               check(world.visible,"show-interface restores Research world markers");
               check(Math.abs(world.getChildAt(0).alpha - 0.55) < 0.01,"world marker opacity is 55 percent");
               var art:DisplayObjectContainer = DisplayObjectContainer(DisplayObjectContainer(world.getChildAt(0)).getChildAt(0));
               var bmp:BitmapData = new BitmapData(40,40,true,0);
               bmp.draw(art.getChildAt(0),new Matrix(1,0,0,1,20,20));
               var fill:uint = bmp.getPixel32(12,20) >>> 24;
               check(fill > 20 && fill < 40,"circle interior has faint fill, no opaque backing: " + fill);
               startPulse();
               return;
            }
            if(phase > 0)
            {
               var elapsed:int = getTimer() - fadeStart;
               if(phase == 1 && elapsed >= 100)
               {
                  check(world.alpha > 0 && world.alpha < 1,"aim exit is gradual");
                  var t:Number = Math.min(1,elapsed / 300);
                  lastAlpha = t*t*(3-2*t);
                  layer.research_markers(packet(true));
                  check(Math.abs(world.alpha-lastAlpha) < 0.03,"rapid reversal preserves current alpha");
                  phase=2; fadeStart=getTimer();
               }
               else if(phase == 2)
               {
                  layer.research_markers(packet(true));
                  if(elapsed >= 280)
                  {
                     check(world.alpha == 0,"repeated aim packets do not restart fade");
                     layer.research_markers([0,0]);
                     check(!world.getChildAt(0).visible,"match end hides world children");
                     layer.research_markers(packet(false,true));
                     phase=3; fadeStart=getTimer();
                  }
               }
               else if(phase == 3 && elapsed >= 380)
               {
                  check(world.alpha == 1,"legacy sender restores visibility smoothly");
                  marker.update(0,2,0,40,7000,true,0,20,false);
                  check(!marker.hasEventListener(Event.ENTER_FRAME),"capture alone starts no blinking animation");
                  phase=4; fadeStart=getTimer();
               }
               else if(phase == 4 && elapsed >= 1000)
               {
                  check(Sprite(marker.getChildAt(0)).alpha == 1,"capture icon stays steady");
                  check(!swords.visible,"capture alone has no crossed swords");
                  removeEventListener(Event.ENTER_FRAME,tick);
                  trace("RESEARCH UI PASS");
               }
               return;
            }
            if(getTimer() - started < sample * 500) return;
            var expected:Number = [1,0.5,0,0.5,1][sample];
            check(Math.abs(swords.alpha - expected) < 0.15,
               rate + " FPS, " + sample * 500 + " ms: alpha " + swords.alpha.toFixed(3) + " near " + expected);
            check(world.alpha == 0,"world stays hidden through contested pulse");
            check(units.alpha == 1 && units.visible,"top HUD stays visible while scoped");
            check(Math.abs(topSwords.alpha-expected) < 0.15,"top swords have the same two-second period");
            if(++sample <= 4) return;
            marker.update(0,1,1,70,0,true,0,20,false);
            check(!swords.visible && !marker.hasEventListener(Event.ENTER_FRAME),"contest ends: swords and animation stop");
            if(rate == 30)
            {
               rate = 144;
               startPulse();
               return;
            }
            units.update([],0);
            check(!units.hasEventListener(Event.ENTER_FRAME),"top animation stops when units disappear");
            layer.research_markers(packet(false));
            check(world.alpha == 0,"aim exit does not pop visible");
            phase=1; fadeStart=getTimer();
         }
         catch(error:Error)
         {
            removeEventListener(Event.ENTER_FRAME,tick);
            trace("RESEARCH UI FAIL " + error.message);
         }
      }
   }
}
