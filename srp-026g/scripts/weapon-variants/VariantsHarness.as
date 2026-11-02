package {
 import flash.display.*;
 import flash.events.*;
 import flash.net.*;
 import flash.system.*;
 // The compiled lobby movie and the skins of live guns (pack weapon-variants): each a golden weapon
 // of its gun (gold background, the gun as its craft donor both ways, the silver bonus as its first
 // modification) and its icon drawn in its cell of the weapon atlas.
 public class VariantsHarness extends Sprite {
  private var loader:Loader = new Loader();
  public function VariantsHarness() {
   loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR, function(e:UncaughtErrorEvent):void { e.preventDefault(); });
   loader.load(new URLRequest("lobby_menu.swf"), new LoaderContext(false, new ApplicationDomain()));
  }
  private function require(ok:Boolean, reason:String):void {
   if(!ok) throw new Error(reason);
  }
  private function loaded(e:Event):void {
   try {
    var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
    var factory:Object = domain.getDefinition("survarium.item.ItemFactory");
    var atlasClass:Class = domain.getDefinition("LobbyMenu_WeaponIcons") as Class;
    var atlas:BitmapData = (new atlasClass() as Bitmap).bitmapData;
    // [dict id, its gun, its cell row, column] (variants.tsv)
    for each(var row:Array in [[2300,306,11,6],[2301,80,12,6],[2302,2211,13,6],[2303,88,14,6],[2304,2229,15,6],
                               [2305,367,16,6],[2306,14,17,6],[2307,15,18,6],[2308,109,19,6],[2309,124,20,6],
                               [2310,321,21,6],[2311,63,22,6],[2312,12,13,5],[2313,12,14,5],[2314,2214,15,5],
                               [2315,2225,16,5]]) {
     var id:int = row[0], plain:int = row[1];
     require(factory.isGolden(id), "golden " + id);
     require(factory.plainOf(id) == plain, "plain " + id);
     require(factory.isCraftDonor(id, plain) && factory.isCraftDonor(plain, id), "craft " + id);
     require(!factory.isCraftDonor(id, 31), "unrelated craft " + id);
     var modifiers:Array = factory.startingModifiers(id);
     require(modifiers.length == 1 && modifiers[0].props[0].prop_id == 126 && modifiers[0].props[0].prop_value == "0.10",
             "silver bonus " + id);
     require(factory.startingModifiers(plain).length == 0 || factory.isGolden(plain), "plain bonuses " + plain);
     var drawn:int = 0;
     for(var y:int = (row[2] - 1) * 91; y < (row[2] - 1) * 91 + 83; y += 2) {
      for(var x:int = (row[3] - 1) * 171; x < row[3] * 171; x += 2) {
       if((atlas.getPixel32(x, y) >>> 24) > 128) drawn++;
      }
     }
     require(drawn > 300, "icon " + id + " in cell " + row[2] + "," + row[3] + ": " + drawn + " pixels");
    }
    trace("VARIANTS RESULT pass");
   } catch(error:Error) { trace("VARIANTS RESULT fail: " + error + " " + error.getStackTrace()); }
   fscommand("quit");
  }
 }
}
