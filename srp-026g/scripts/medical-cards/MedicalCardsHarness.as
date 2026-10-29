package {
 import flash.display.*;
 import flash.events.*;
 import flash.net.*;
 import flash.system.*;
 public class MedicalCardsHarness extends Sprite {
  private var loader:Loader = new Loader();
  private var domain:ApplicationDomain;
  private var cards:Array = [];
  private var failures:Array = [];
  private var frame:int = 0;
  public function MedicalCardsHarness() {
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void { e.preventDefault(); });
   loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function loaded(e:Event):void {
   try {
    domain = loader.contentLoaderInfo.applicationDomain;
    var Props:Class = domain.getDefinition("survarium.data.PropertiesDescriptor") as Class;
    var Compare:Object = domain.getDefinition("survarium.controls.ItemCompare");
    var descriptors:Array = [];
    for each(var spec:Array in [[19,"Activation",0,"s"],[18,"Duration",1,"s"],[135,"Healing: arms / legs",1,"%"],[136,"Healing: body / head",1,"%"],[137,"Pain relief",1,"%"],[15,"Weight",0,"kg"],[12,"Healing",1,""]])
     descriptors.push({prop_id:spec[0],prop_name:spec[1],direction:spec[2],postfix:spec[3],comparable:true,min_value:0,max_value:100});
    var props:Object = new Props("medical-harness");
    props.setPropertiesDictionary(descriptors); Compare.setPropsDescriptor(props);
    var Hint:Class = domain.getDefinition("InventoryItemHint") as Class;
    for(var i:int=0;i<2;i++) {
     var card:Object = new Hint(); card.x = 25+i*280; card.y = 40; addChild(DisplayObject(card)); cards.push(card);
    }
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error) { finish("setup " + err); }
  }
  private function item(bandages:Boolean):Object {
   var Data:Class = domain.getDefinition("survarium.data.ItemData") as Class;
   var Factory:Object = domain.getDefinition("survarium.item.ItemFactory");
   var data:Object = new Data();
   data.name = bandages ? "Pack of bandages" : "Medkit"; data.category = 10;
   data.superCategory = Factory.getCategory(10); data.descriptor = 100; data.modifiers = [];
   data.itemDescription = bandages ? "Fully heals arms and legs. Restores 35% of body, head and pain." : "Basic first-aid medkit.";
   data.properties = [];
   for each(var pair:Array in [[19,bandages?1:2],[18,bandages?1:3],[135,100],[136,bandages?35:100],[137,bandages?35:0],[15,bandages?0.1:0.2]])
    data.properties.push({prop_id:pair[0],prop_value:String(pair[1])});
   return data;
  }
  private function rows(card:Object):Object {
   var result:Object = {};
   for(var i:int=1;i<=9;i++) {
    var row:Object = card.itemCompare.getChildByName("diff_"+i);
    if(row && row.visible) result[row.id] = row;
   }
   return result;
  }
  private function check(card:Object,bandages:Boolean):void {
   var shown:Object = rows(card); var count:int=0;
   for each(var row:Object in shown) {
    count++;
    var label:Object = row.indicator.textFieldLabel;
    if(label.textWidth > label.width + 1) failures.push("clipped label " + label.text);
    var bottom:Number = row.getBounds(DisplayObject(card)).bottom;
    if(bottom > card.background.y + card.background.height + 1) failures.push("clipped row " + row.id);
   }
   if(count != 6) failures.push("expected six rows, got " + count);
   if(shown[12]) failures.push("invented Healing 0");
   for each(var spec:Array in [[19,bandages?"1s":"2s"],[135,"100%"],[136,bandages?"35%":"100%"],[137,bandages?"35%":"0%"],[15,bandages?"0.1kg":"0.2kg"]])
    if(!shown[spec[0]] || shown[spec[0]].indicator.textFieldValue.text != spec[1]) failures.push("wrong/missing value " + spec[0]);
   var Colors:Object = domain.getDefinition("survarium.constants.Colors");
   if(shown[19].indicator.textFieldValue.textColor != (bandages ? Colors.GREEN_DIFF : Colors.RED_DIFF)) failures.push("activation comparison direction");
   trace("MEDICAL " + (bandages?"bandages":"medkit") + " six rows including weight, healing, pain; card bottom=" + (card.background.y+card.background.height));
  }
  private function tick(e:Event):void {
   frame++;
   try {
    if(frame == 10) { cards[0].data = item(true); cards[1].data = item(false); }
    if(frame == 20) { cards[0].itemCompare.setSecondaryWeapon(cards[1].data); cards[1].itemCompare.setSecondaryWeapon(cards[0].data); }
    if(frame == 30) { check(cards[0],true); check(cards[1],false); cards[0].itemCompare.setSecondaryWeapon({category:10,properties:[{prop_id:12,prop_value:"100"}],modifiers:[]}); }
    if(frame == 40) {
     if(rows(cards[0])[12]) failures.push("missing property synthesized from other medicine");
     cards[0].itemCompare.setSecondaryWeapon(cards[1].data);
    }
    if(frame == 50) { check(cards[0],true); finish(null); }
   } catch(err:Error) { finish("frame " + frame + ": " + err + " " + err.getStackTrace()); }
  }
  private function finish(error:String):void {
   removeEventListener(Event.ENTER_FRAME,tick); if(error) failures.push(error);
   trace("MEDICAL RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass")); if(loaderInfo.parameters.hold != "1") fscommand("quit");
  }
 }
}
