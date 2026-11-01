package {
 import flash.display.*;
 import flash.events.*;
 import flash.net.URLRequest;
 import flash.system.*;
 import flash.utils.Dictionary;
 import flash.geom.ColorTransform;
 public class ActivityHarness extends Sprite {
  private var loader:Loader = new Loader();
  private var slots:Object;
  private var frame:int = 0;
  private var failures:Array = [];
  public function ActivityHarness() {
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void { e.preventDefault(); });
   loader.load(new URLRequest("hud.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function loaded(e:Event):void {
   try {
    var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
    var Slots:Class = domain.getDefinition("SrvQuickSlots") as Class;
    var Icons:Class = domain.getDefinition("survarium.data.IconContainer") as Class;
    var Sheet:Class = domain.getDefinition("hudItems") as Class;
    slots = new Slots();
    var icons:Object = new Icons("activity-test",171,83,new Sheet(),true,0,0,0,false);
    slots.registerIconContainer(icons);
    var gray:Dictionary = new Dictionary();
    // Distinct gray icon instance, as supplied by HUD.makeGrayscale in-game.
    gray[0] = icons.getIcon(0).clone();
    slots.setGrayIcons(gray);
    slots.x = 160; slots.y = 160;
    addChild(DisplayObject(slots));
    slots.fillSlot(item(1,true));
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error) { finish("setup: " + err); }
  }
  private function item(count:uint, enabled:Boolean, artifact:Boolean=false):Object {
   return {slot_id:1, icon_row:18, icon_column:1, count:count, enabled:enabled,
           hotkey:"4", timer:artifact ? 12 : 0, cooldown:0, is_artefact:artifact};
  }
  private function check(seconds:String, activation:Boolean, length:uint=1):void {
   try {
    if(slots.slots_list.dataProvider.length != length) throw new Error("slot count");
    if(length == 0) return;
    var renderer:Object = slots.slots_list.getRendererAt(0);
    if(renderer.count_timer.text != seconds) throw new Error("seconds " + renderer.count_timer.text + " expected " + seconds);
    var arc:DisplayObject = renderer.getChildByName("consumableActivityRing");
    if(arc != null) throw new Error("custom activity ring still present");
    if(Boolean(renderer.overCooldown && renderer.overCooldown.width > 0) != activation) throw new Error("activation sweep");
    if(activation && renderer.content != renderer.data.grayIcon) throw new Error("waiting icon not gray");
    if(!activation && seconds != "" && renderer.content != renderer.data.icon) throw new Error("active icon not colored");
    checkIconPixels(renderer,activation && !renderer.data.is_artefact);
    if(renderer.timerBack.visible != (seconds != "")) throw new Error("timer backdrop");
    trace("ACTIVITY visual " + seconds + " activation=" + activation);
   } catch(err:Error) { failures.push("frame " + frame + ": " + err); }
  }
  // The icon tinted red (green and blue x0.3, red kept) or not tinted at all;
  // the slot still there, its colored icon, no seconds, no sweep.
  private function red(tinted:Boolean):void {
   try {
    if(slots.slots_list.dataProvider.length != 1) throw new Error("slot count");
    var renderer:Object = slots.slots_list.getRendererAt(0);
    if(renderer.content != renderer.data.icon) throw new Error("not the colored icon");
    if(tinted && (renderer.count_timer.text != "" || (renderer.overCooldown && renderer.overCooldown.width > 0))) throw new Error("seconds or sweep while red");
    var source:BitmapData = renderer.content.bitmap.bitmapData;
    var rendered:BitmapData = new BitmapData(source.width,source.height,true,0);
    rendered.draw(renderer.content,null,renderer.content.transform.colorTransform);
    var samples:uint = 0;
    for(var y:uint=0; y<source.height; y++) for(var x:uint=0; x<source.width; x++) {
     var before:uint = source.getPixel32(x,y);
     if((before >>> 24) != 255 || ((before >> 8) & 255) < 100) continue;
     var after:uint = rendered.getPixel32(x,y);
     var green:Number = (before >> 8) & 255;
     if(Math.abs(((after >> 16) & 255) - ((before >> 16) & 255)) > 2) throw new Error("red channel changed");
     if(Math.abs(((after >> 8) & 255) - (tinted ? green * 0.3 : green)) > 2) throw new Error("green " + ((after >> 8) & 255) + " from " + green + (tinted ? " (red expected)" : " (no red expected)"));
     samples++;
    }
    rendered.dispose();
    if(samples == 0) throw new Error("no opaque icon pixels checked");
    trace("ACTIVITY red=" + tinted);
   } catch(err:Error) { failures.push("frame " + frame + ": " + err); }
  }
  private function checkIconPixels(renderer:Object, waiting:Boolean):void {
   var source:BitmapData = renderer.content.bitmap.bitmapData;
   var rendered:BitmapData = new BitmapData(source.width,source.height,true,0);
   // BitmapData.draw does not inherit the source root's transform; pass it as
   // the display list would. Sample opaque real-icon pixels, not just identity.
   var transform:ColorTransform = renderer.content.transform.colorTransform;
   rendered.draw(renderer.content,null,transform);
   var samples:uint = 0;
   for(var y:uint=0; y<source.height; y++) for(var x:uint=0; x<source.width; x++) {
    var before:uint = source.getPixel32(x,y);
    if((before >>> 24) != 255) continue;
    var red:uint = (before >> 16) & 255;
    if(red < 100) continue;
    var after:uint = (rendered.getPixel32(x,y) >> 16) & 255;
    var expected:Number = waiting ? red / 2 : red;
    if(Math.abs(after - expected) > 2) throw new Error("icon pixel brightness " + after + " expected " + expected);
    samples++;
   }
   rendered.dispose();
   if(samples == 0) throw new Error("no opaque icon pixels checked");
  }
  private function tick(e:Event):void {
   frame++;
   switch(frame) {
    case 10: check("",false); slots.setActivity(1,1,1000,1500); slots.fillSlot(item(0,false)); break;
    case 20: check("",true); slots.setActivity(1,1,1,1500); break;
    case 25: check("",true); slots.setActivity(1,2,7500,7500); break;
    case 30: check("08",false); slots.setActivity(1,2,4501,7500); break;
    case 40: check("05",false); slots.setActivity(1,2,1,7500); break;
    case 50: check("01",false); slots.setActivity(1,0,0,0); break;
    case 60: check("",false,0); slots.fillSlot(item(2,true)); slots.setActivity(1,2,2000,7500); break;
    case 70: check("02",false); slots.setActivity(1,0,0,0); break;
    case 80: check("",false); slots.fillSlot(item(1,true,true)); break;
    case 90: check("12",false); slots.clean(); break;
    case 100: check("",false,0); slots.fillSlot(item(1,true)); break;
    case 110: check("",false); slots.setActivity(1,1,750,1500); break;
    case 120: check("",true); slots.setActivity(1,0,0,0); break;
    case 130: check("",false); slots.scaleX = slots.scaleY = 0.65; slots.setActivity(1,1,500,1500); break;
    case 140: check("",true); slots.clean(); break;
    case 150: check("",false,0); slots.fillSlot(item(1,true)); break;
    case 160: check("",false); var artifact:Object = item(1,true,true); artifact.cooldown = 50; artifact.timer = 0; slots.fillSlot(artifact); break;
    case 170: check("",true); slots.fillSlot(item(1,true,true)); break;
    case 180: check("12",false); slots.clean(); slots.fillSlot(item(1,true)); slots.setActivity(1,1,1000,1500); break;
    // Patch 84: phase 3, a drug a bullet stopped, tints the icon red (the hook
    // sends it for a second of game time, then 0); nothing else is red.
    case 190: check("",true); slots.setActivity(1,3,0,0); break;
    case 200: red(true); slots.setActivity(1,0,0,0); break;
    case 210: check("",false); red(false); slots.setActivity(1,2,4000,7500); break;
    case 220: check("04",false); red(false); slots.setActivity(1,3,0,0); break;
    case 230: red(true); slots.fillSlot(item(0,false)); break;
    case 240: red(true); slots.setActivity(1,0,0,0); break;
    case 250: check("",false,0); slots.fillSlot(item(1,true)); slots.setActivity(1,2,50,7500); break;
    case 260: check("01",false); slots.setActivity(1,0,0,0); break;
    case 270: check("",false); red(false); slots.setActivity(1,3,0,0); slots.clean(); break;
    case 280: check("",false,0); slots.fillSlot(item(1,true)); break;
    case 290: check("",false); red(false); finish(null); break;
   }
  }
  private function finish(error:String):void {
   removeEventListener(Event.ENTER_FRAME,tick);
   if(error) failures.push(error);
   trace("ACTIVITY RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
   fscommand("quit");
  }
 }
}
