package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.URLRequest;
 import flash.system.*;
 // The quick slots' item icons: each oxygen tank (data/type 1, the back slot's use action) shows its
 // hud icon, the stock ones from hudItems, srp's from their bitmaps (QuickSlots.SRP_HUD_ICONS).
 public class QuickIconsHarness extends Sprite {
  private var loader:Loader = new Loader();
  private var failures:Array = [];
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
    var slots:Object = new Slots();
    var icons:Object = new Icons("quick-icons-test",171,83,sheet,true,0,0,0,false);
    slots.registerIconContainer(icons);
    addChild(DisplayObject(slots));
    // [name, ui_desc/icon row, column, the bitmap its slot must show]
    var tanks:Array = [
     ["scavenger_oxygen_1", 14, 1, cell(sheet,6)],
     ["edge_back_3", 21, 2, cell(sheet,7)],
     ["ren_back_5", 2, 4, cell(sheet,8)],
     ["ren_back_3", 9, 4, cell(sheet,9)],
     ["duty_exo_026", 1, 6, cell(sheet,28)],
     ["931 Double cylinder", 17, 14, new (domain.getDefinition("HudItemDoubleCylinder") as Class)()]
    ];
    for each(var tank:Array in tanks) {
     slots.clean();
     slots.fillSlot({slot_id:6, icon_row:tank[1], icon_column:tank[2], count:1, enabled:true,
                     hotkey:"G", timer:0, cooldown:0, is_artefact:false});
     var shown:Object = slots.slots_list.dataProvider[0];
     var bitmap:BitmapData = shown && shown.icon && shown.icon.bitmap ? shown.icon.bitmap.bitmapData : null;
     if(bitmap == null) { failures.push(tank[0] + ": no icon"); continue; }
     if(bitmap.getColorBoundsRect(0xFF000000,0,false).isEmpty()) { failures.push(tank[0] + ": an empty icon"); continue; }
     if(bitmap.compare(tank[3]) != 0) { failures.push(tank[0] + ": not its icon"); continue; }
     trace("QUICK ICONS " + tank[0] + " shows its icon");
    }
   } catch(err:Error) { failures.push("setup: " + err); }
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
