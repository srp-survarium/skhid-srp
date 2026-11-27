package {
 import flash.display.*;
 import flash.events.*;
 import flash.net.URLRequest;
 import flash.system.*;
 import flash.utils.Dictionary;
 import flash.geom.ColorTransform;
 import flash.filters.DropShadowFilter;
 import flash.filters.GlowFilter;
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
    slots = makeSlots();
    slots.x = 160; slots.y = 160;
    addChild(DisplayObject(slots));
    slots.fillSlot(item(1,true));
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error) { finish("setup: " + err); }
  }
  private function makeSlots():Object {
   var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
   var Slots:Class = domain.getDefinition("SrvQuickSlots") as Class;
   var Icons:Class = domain.getDefinition("survarium.data.IconContainer") as Class;
   var Sheet:Class = domain.getDefinition("hudItems") as Class;
   var made:Object = new Slots();
   var icons:Object = new Icons("activity-test",171,83,new Sheet(),true,0,0,0,false);
   made.registerIconContainer(icons);
   var gray:Dictionary = new Dictionary();
   // Distinct gray icon instance, as supplied by HUD.makeGrayscale in-game.
   gray[0] = icons.getIcon(0).clone();
   made.setGrayIcons(gray);
   return made;
  }
  private function item(count:uint, enabled:Boolean, artifact:Boolean=false):Object {
   return {slot_id:1, icon_row:18, icon_column:1, count:count, enabled:enabled,
           hotkey:"4", timer:artifact ? 12 : 0, cooldown:0, is_artefact:artifact};
  }
  // An oxygen tank (icon cell 6, id 13) with `seconds` of air, as the game fills its slot.
  private function tank(seconds:uint):Object {
   return {slot_id:1, icon_row:14, icon_column:1, count:1, enabled:true,
           hotkey:"4", timer:seconds, cooldown:0, is_artefact:false};
  }
  // Oxygen Rush's countdown: the hud's blue mixed with white (Colors.SPEED_BOOST), over the
  // countdown's own black shadow, in a glow of the hud's blue (Colors.HUD_BLUE, its style "blue").
  private static const BOOST_TEXT:uint = 0x9CD8EE;
  private static const HUD_BLUE:uint = 0x079DD5;
  private var normal:uint;
  private var normalFilters:Array;
  private function timerColour(expected:uint):void {
   try {
    var renderer:Object = slots.slots_list.getRendererAt(0);
    if(renderer.count_timer.textColor != expected) throw new Error("countdown colour " + renderer.count_timer.textColor.toString(16) + " expected " + expected.toString(16));
    var filters:Array = renderer.count_timer.filters;
    var boosted:Boolean = expected == BOOST_TEXT;
    if(filters.length != normalFilters.length + (boosted ? 1 : 0)) throw new Error("countdown filters " + filters.length + " (its own " + normalFilters.length + ")");
    for(var i:uint=0; i<normalFilters.length; i++) {
     if(!(filters[i] is DropShadowFilter) || filters[i].color != normalFilters[i].color || filters[i].blurX != normalFilters[i].blurX || filters[i].strength != normalFilters[i].strength) throw new Error("countdown's own shadow lost");
    }
    if(boosted && !(filters[i] is GlowFilter && filters[i].color == HUD_BLUE)) throw new Error("no glow of the hud's blue");
    trace("ACTIVITY countdown colour " + expected.toString(16) + (boosted ? " glow " + HUD_BLUE.toString(16) : ""));
   } catch(err:Error) { failures.push("frame " + frame + ": " + err); }
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
    case 290: check("",false); red(false); slots.fillSlot(tank(45)); break;
    // Patch 180: phase 4, an oxygen tank whose Oxygen Rush runs, draws its native
    // countdown in the hud's light blue with a blue glow; the rest of the slot and the next ticks as before.
    case 300: check("45",false); normal = slots.slots_list.getRendererAt(0).count_timer.textColor; normalFilters = slots.slots_list.getRendererAt(0).count_timer.filters; if(normal == BOOST_TEXT) failures.push("the countdown is blue already"); if(normalFilters.length == 0) failures.push("the countdown has no shadow"); slots.setActivity(1,4,0,0); break;
    case 310: check("45",false); timerColour(BOOST_TEXT); slots.fillSlot(tank(44)); break;
    case 320: check("44",false); timerColour(BOOST_TEXT); slots.setActivity(1,0,0,0); break;
    case 330: check("44",false); timerColour(normal); slots.fillSlot(item(1,true,true)); slots.setActivity(1,5,0,7); break;
    case 340:
     if(slots.slots_list.getRendererAt(0).count.text != "100%") failures.push("fresh artifact power");
     slots.setActivity(1,5,1,7); break;
    // The use is paid at the press: while the copy is active, the power it fires with.
    case 341:
     if(slots.slots_list.getRendererAt(0).count.text != "100%") failures.push("wear switch while active");
     check("12",false);
     var cooling:Object = item(1,true,true); cooling.cooldown = 50; cooling.timer = 0; slots.fillSlot(cooling);
     slots.setActivity(1,5,1,7); break;
    case 342:
     var label:Object = slots.slots_list.getRendererAt(0).count;
     if(label.textWidth + 4 > label.width) failures.push("clipped wear label: " + label.textWidth + " / " + label.width);
     if(label.text != "100% → 80%") failures.push("wear transition on cooldown");
     check("",true); slots.fillSlot(item(1,true,true)); slots.setActivity(1,5,4,9); break;
    case 350:
     if(slots.slots_list.getRendererAt(0).count.text != "41%") failures.push("replacement artifact power");
     slots.setActivity(1,5,21,8); break;
    case 355:
     if(slots.slots_list.getRendererAt(0).count.text != "<1%") failures.push("low artifact power");
     slots.setActivity(1,0,0,0); slots.fillSlot(item(2,true)); break;
    case 360:
     if(slots.slots_list.getRendererAt(0).count.text != "x2") failures.push("stale artifact power");
     shot(); break;
    case 380: trace("ACTIVITY SHOT boost"); break;
    case 420: finish(null); break;
   }
  }
  // SHOT: a drug's white "x4", a tank's own countdown and one in Oxygen Rush, over a dark ground
  // and a bright one (check.sh takes the screenshot at ACTIVITY SHOT).
  private function shot():void {
   try {
    removeChild(DisplayObject(slots));
    var grounds:Array = [0x1C1F22,0xD8D5CB];
    for(var row:uint=0; row<grounds.length; row++) {
     graphics.beginFill(grounds[row]);
     graphics.drawRect(0,row * 360,1280,360);
     graphics.endFill();
     var made:Object = makeSlots();
     made.x = 160; made.y = 100 + row * 360;
     addChild(DisplayObject(made));
     made.fillSlot({slot_id:0, icon_row:18, icon_column:1, count:4, enabled:true, hotkey:"3", timer:0, cooldown:0, is_artefact:false});
     for(var slot:uint=1; slot<3; slot++) made.fillSlot({slot_id:slot, icon_row:14, icon_column:1, count:1, enabled:true, hotkey:String(3 + slot), timer:126, cooldown:0, is_artefact:false});
     made.setActivity(2,4,0,0);
    }
   } catch(err:Error) { failures.push("shot: " + err); }
  }
  private function finish(error:String):void {
   removeEventListener(Event.ENTER_FRAME,tick);
   if(error) failures.push(error);
   trace("ACTIVITY RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
   fscommand("quit");
  }
 }
}
