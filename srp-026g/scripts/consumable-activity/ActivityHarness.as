package {
 import flash.display.*;
 import flash.events.*;
 import flash.net.URLRequest;
 import flash.system.*;
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
    slots.registerIconContainer(new Icons("activity-test",171,83,new Sheet(),true,0,0,0,false));
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
  private function check(seconds:String, ring:Boolean, length:uint=1):void {
   try {
    if(slots.slots_list.dataProvider.length != length) throw new Error("slot count");
    if(length == 0) return;
    var renderer:Object = slots.slots_list.getRendererAt(0);
    if(renderer.count_timer.text != seconds) throw new Error("seconds " + renderer.count_timer.text + " expected " + seconds);
    var arc:DisplayObject = renderer.getChildByName("consumableActivityRing");
    if((arc.width > 0) != ring) throw new Error("ring visibility");
    if(renderer.timerBack.visible != (seconds != "")) throw new Error("timer backdrop");
    trace("ACTIVITY visual " + seconds + " ring=" + ring + " bounds=" + arc.getBounds(DisplayObject(renderer)));
   } catch(err:Error) { failures.push("frame " + frame + ": " + err); }
  }
  private function tick(e:Event):void {
   frame++;
   switch(frame) {
    case 10: check("",false); slots.setActivity(1,1,0,0); slots.fillSlot(item(0,false)); break;
    case 20: check("",false); slots.setActivity(1,2,7500,7500); break;
    case 30: check("08",true); slots.setActivity(1,2,4501,7500); break;
    case 40: check("05",true); slots.setActivity(1,2,1,7500); break;
    case 50: check("01",true); slots.setActivity(1,0,0,0); break;
    case 60: check("",false,0); slots.fillSlot(item(2,true)); slots.setActivity(1,2,2000,7500); break;
    case 70: check("02",true); slots.setActivity(1,0,0,0); break;
    case 80: check("",false); slots.fillSlot(item(1,true,true)); break;
    case 90: check("12",false); slots.clean(); break;
    case 100: check("",false,0); slots.fillSlot(item(1,true)); break;
    case 110: check("",false); finish(null); break;
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
