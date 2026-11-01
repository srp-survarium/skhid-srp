package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.URLRequest;
 import flash.system.*;
 import flash.utils.Dictionary;
 // The quick slots' item icons: each oxygen tank (data/type 1, the back slot's use action) and each
 // medicine shows its hud icon, the stock ones from hudItems, srp's from their bitmaps
 // (QuickSlots.SRP_HUD_ICONS). A used-up medicine (a stack of none, its use time running) keeps its
 // own icon, gray where the hud has a gray copy (HUD.as: cells 0 to 9 and 28 on), dimmed.
 public class QuickIconsHarness extends Sprite {
  private var loader:Loader = new Loader();
  private var failures:Array = [];
  private var slots:Object;
  private var medicines:Array;
  private var frame:int = 0;
  public function QuickIconsHarness() {
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
    var sheet:BitmapData = new Sheet();
    slots = new Slots();
    var icons:Object = new Icons("quick-icons-test",171,83,sheet,true,0,0,0,false);
    slots.registerIconContainer(icons);
    // The gray copies HUD.as makes (a copy here: the test is that a slot takes its own cell's).
    var gray:Dictionary = new Dictionary();
    var Quick:Object = domain.getDefinition("survarium.controls.QuickSlots");
    var srpEnd:uint = Quick.SRP_FIRST_CELL + Quick.SRP_HUD_ICONS.length;
    for(var i:uint = 0; i < srpEnd; i++) if(i < 10 || i >= 28) gray[i] = icons.getIcon(i).clone();
    slots.setGrayIcons(gray);
    slots.x = 160; slots.y = 160;
    addChild(DisplayObject(slots));
    // [name, ui_desc/icon row, column, the bitmap its slot must show]
    var tanks:Array = [
     ["scavenger_oxygen_1", 14, 1, cell(sheet,6)],
     ["edge_back_3", 21, 2, cell(sheet,7)],
     ["ren_back_5", 2, 4, cell(sheet,8)],
     ["ren_back_3", 9, 4, cell(sheet,9)],
     ["duty_exo_026", 1, 6, cell(sheet,28)],
     ["931 Double cylinder", 17, 14, srp(domain,"HudItemDoubleCylinder")]
    ];
    for each(var tank:Array in tanks) {
     slots.clean();
     slots.fillSlot(item(6,tank,1,true));
     var shown:Object = slots.slots_list.dataProvider[0];
     var problem:String = differs(shown ? shown.icon : null,tank[3]);
     if(problem) { failures.push(tank[0] + ": " + problem); continue; }
     trace("QUICK ICONS " + tank[0] + " shows its icon");
    }
    // [name, row, column, its bitmap, a gray copy]: scripts/medicines/medicines.tsv's icon cells.
    medicines = [
     ["bandages", 2, 2, cell(sheet,13), false],
     ["medkit", 15, 1, cell(sheet,10), false],
     ["military_medkit", 16, 3, srp(domain,"HudItemMilitaryMedkit"), true],
     ["blackmarket_medkit", 17, 3, srp(domain,"HudItemBlackMarketMedkit"), true],
     ["science_medkit", 18, 3, srp(domain,"HudItemScienceMedkit"), true]
    ];
    slots.clean();
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error) { finish("setup: " + err); }
  }
  private function srp(domain:ApplicationDomain, name:String):BitmapData {
   return new (domain.getDefinition(name) as Class)();
  }
  private function item(slot:uint, row:Array, count:uint, enabled:Boolean):Object {
   return {slot_id:slot, icon_row:row[1], icon_column:row[2], count:count, enabled:enabled,
           hotkey:"4", timer:0, cooldown:0, is_artefact:false};
  }
  // Each medicine: shown in colour; used up (a stack of none in its use time) still its own icon,
  // gray if it has a copy, dimmed, with the sweep; gone when its effect ends.
  private function tick(e:Event):void {
   frame++;
   if(frame % 10 != 0) return;
   var step:int = frame / 10 - 1;
   var medicine:Array = medicines[int(step / 3)];
   try {
    switch(step % 3) {
     case 0: slots.fillSlot(item(1,medicine,2,true)); break;
     case 1: shows(medicine,false); slots.setActivity(1,1,750,1500); slots.fillSlot(item(1,medicine,0,false)); break;
     case 2:
      shows(medicine,true);
      slots.setActivity(1,0,0,0);
      if(slots.slots_list.dataProvider.length != 0) throw new Error("still shown when its effect ended");
      trace("QUICK ICONS " + medicine[0] + " shows its icon, used up " + (medicine[4] ? "gray" : "dimmed"));
      break;
    }
   } catch(err:Error) { failures.push(medicine[0] + ": " + err.message); slots.clean(); }
   if(step == medicines.length * 3 - 1) finish(null);
  }
  private function shows(medicine:Array, usedUp:Boolean):void {
   if(slots.slots_list.dataProvider.length != 1) throw new Error((usedUp ? "used up: " : "") + "no slot");
   var renderer:Object = slots.slots_list.getRendererAt(0);
   var data:Object = renderer.data;
   var problem:String = differs(data.icon,medicine[3]);
   if(problem) throw new Error(problem);
   if(Boolean(data.grayIcon) != medicine[4]) throw new Error(medicine[4] ? "no gray copy" : "a gray copy");
   if(data.grayIcon && (problem = differs(data.grayIcon,medicine[3]))) throw new Error("gray copy: " + problem);
   var expected:Object = usedUp && data.grayIcon ? data.grayIcon : data.icon;
   if(renderer.content != expected) throw new Error((usedUp ? "used up" : "in use") + ": not its " + (expected == data.grayIcon ? "gray " : "") + "icon");
   var shade:Number = renderer.content.transform.colorTransform.redMultiplier;
   if(shade != (usedUp ? 0.5 : 1)) throw new Error("brightness " + shade);
   if(usedUp && !(renderer.overCooldown && renderer.overCooldown.width > 0)) throw new Error("used up: no sweep");
  }
  private function differs(icon:Object, want:BitmapData):String {
   var bitmap:BitmapData = icon && icon.bitmap ? icon.bitmap.bitmapData : null;
   if(bitmap == null) return "no icon";
   if(bitmap.getColorBoundsRect(0xFF000000,0,false).isEmpty()) return "an empty icon";
   if(bitmap.compare(want) != 0) return "not its icon";
   return null;
  }
  private function finish(error:String):void {
   removeEventListener(Event.ENTER_FRAME,tick);
   if(error) failures.push(error);
   trace("QUICK ICONS RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
   fscommand("quit");
  }
  // hudItems' cell (vertical: 24 a column), as IconContainer cuts it.
  private function cell(sheet:BitmapData, index:uint):BitmapData {
   var out:BitmapData = new BitmapData(171,83,true,0);
   var m:Matrix = new Matrix();
   m.translate(-171 * int(index / 24),-83 * (index % 24));
   out.draw(sheet,m,null,null,new Rectangle(0,0,171,83));
   return out;
  }
 }
}
