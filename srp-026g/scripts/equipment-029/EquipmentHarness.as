package {
 import flash.display.*;
 import flash.events.*;
 import flash.net.*;
 import flash.system.*;
 public class EquipmentHarness extends Sprite {
  private var loader:Loader = new Loader();
  public function EquipmentHarness() {
   loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR, function(e:UncaughtErrorEvent):void { e.preventDefault(); });
   loader.load(new URLRequest("lobby_menu.swf"), new LoaderContext(false, new ApplicationDomain()));
  }
  private function require(ok:Boolean, reason:String):void {
   if(!ok) throw new Error(reason);
  }
  private function loaded(e:Event):void {
   try {
    var factory:Object = loader.contentLoaderInfo.applicationDomain.getDefinition("survarium.item.ItemFactory");
    // Each faction, a new plain parent, an existing parent, and the special set;
    // equipment-046's premium copy (Equipment046.as) of the stock Bandit's jacket.
    for each(var row:Array in [[905,824,128],[907,347,129],[911,754,130],[977,196,131],[939,938,128],[1863,351,129]]) {
     var id:int = row[0], plain:int = row[1];
     require(factory.isGolden(id), "premium " + id);
     require(factory.plainOf(id) == plain, "plain " + id);
     require(factory.isCraftDonor(id,plain) && factory.isCraftDonor(plain,id), "craft " + id);
     require(!factory.isCraftDonor(id,31), "unrelated craft " + id);
     var modifiers:Array = factory.startingModifiers(id);
     require(modifiers.length == 2, "bonus count " + id);
     require(modifiers[0].props[0].prop_id == 127 && modifiers[0].props[0].prop_value == "0.05", "XP " + id);
     require(modifiers[1].props[0].prop_id == row[2] && modifiers[1].props[0].prop_value == "0.05", "reputation " + id);
     require(!factory.isGolden(plain) && factory.startingModifiers(plain).length == 0, "plain bonuses " + plain);
    }
    trace("EQUIPMENT RESULT pass");
   } catch(error:Error) { trace("EQUIPMENT RESULT fail: " + error + " " + error.getStackTrace()); }
   fscommand("quit");
  }
 }
}
