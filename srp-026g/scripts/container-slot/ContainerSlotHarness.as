package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.*;
 import flash.system.*;
 import flash.utils.*;
 // Artifact Container's slot in the kit (docs/notes/artefact-skills.md, "Artifact Container (522)"),
 // with the lobby movie's own player panel (SrvLocalPlayerProfile) and paper doll: srp's `K` row of
 // the presets list (LocalPlayerProfile.presetsTake) for the kit shown, the yellow outline on the
 // first container as the container and on the last pocket as the pocket, the hint, a click asking
 // for the other (the mark's ask, given here in place of the exe's request), one ask at a time, a
 // pocket holding an item kept as it is, no mark without the skill or on another kit; every
 // language's two hints. Traces CONTAINER lines, `CONTAINER shot <name>` when a capture is due, and
 // `CONTAINER RESULT pass|fail: ...`.
 public class ContainerSlotHarness extends Sprite {
  private static const W:int=900, H:int=700;
  private static const YELLOW:uint=0xFFD23F;
  private var loader:Loader=new Loader();
  private var domain:ApplicationDomain;
  private var lobby:Object, profile:Object, doll:Object, Profile:Object, Mark:Object;
  private var asked:Array=[];
  private var failures:Array=[];
  private var step:int=0, wait:int=0, frames:int=0;
  public function ContainerSlotHarness() {
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void{e.preventDefault();});
   loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function C(name:String):Class {return domain.getDefinition(name) as Class;}
  private function check(ok:Boolean,why:String):void {if(!ok)failures.push(why);}
  private function texts(language:String):Object {var t:Object={};for(var k:String in HarnessText.DATA[language])t[k]=HarnessText.DATA[language][k];t.sell="Sell";return t;}
  private function loaded(e:Event):void {
   var at:String="the classes";
   try {
    domain=loader.contentLoaderInfo.applicationDomain;
    lobby=loader.content;
    addChild(loader);
    domain.getDefinition("MainScene").stage=lobby;
    domain.getDefinition("survarium.data.LocalizationData").setData(texts("english"));
    var tips:Sprite=new Sprite();
    C("scaleform.clik.controls.ToolTip")["tipContainer"]=tips;
    var Container:Class=C("survarium.data.IconContainer");
    for each(var icons:String in ["equipment","camo","consumables","bundles","factionItemIcons"]) new Container(icons,1,1,new BitmapData(1,1,true,0),false,1,0,0,false);
    at="the panel";
    domain.getDefinition("survarium.controls.SrvContextMenu").registerStage(stage,lobby);
    at="the profile";
    profile=new (C("SrvLocalPlayerProfile"))();
    at="its ammo share";
    profile.setAmmoShare(domain.getDefinition("survarium.ammo.AmmoShareLite").getInstance());
    at="the lobby's fields";
    lobby.player_profile=profile;
    lobby.addChild(profile);
    var inventory:*=new (C("SrvInventoryReskin"))();
    inventory.name="inventory_list";
    lobby.inventory_list=inventory;
    lobby.addChild(inventory);
    inventory.x=W*3;
    inventory.registerItemDescriptor(new (C("survarium.data.ItemDescriptor"))("container-slot-check"));
    at="the inventory";
    profile.registerInventory(inventory);
    var shop:*=new (C("survarium.controls.GangShop"))();
    profile.paperDoll.ammoControl=new (C("survarium.ammo.AmmoShareControll"))();
    profile.paperDoll.ammoControl.registerShop(shop);
    profile.paperDoll.ammoControl.registerInventory(inventory);
    lobby.addChild(tips);
    doll=profile.paperDoll;
    Profile=domain.getDefinition("survarium.player.LocalPlayerProfile");
    Mark=domain.getDefinition("survarium.player.ContainerSlotMark");
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error) {finish("setup at "+at+": "+err+" "+err.getStackTrace());}
  }
  private function mark():Object {
   var d:DisplayObjectContainer=doll as DisplayObjectContainer;
   for(var i:int=0;i<d.numChildren;i++) if(d.getChildAt(i) is (Mark as Class)) return d.getChildAt(i);
   return null;
  }
  private function slot(quick:int):DisplayObject {return doll.getSlotAt(12+quick) as DisplayObject;}
  // The kit shown: its id and its pockets and containers, as the exe fills the panel.
  private function kit(id:int,pockets:int,containers:int):void {profile.fillProfileItems({profileId:id,items:[],item_slots:pockets,artifact_slots:containers});}
  private function list(k:String):void {
   var rows:Array=["P2;L;0;0;1;0;-1","1;1;0","2;0;-1","3;0;-1","4;0;-1","5;0;-1",k];
   var out:Array=[];for(var i:int=0;i<rows.length;i++) out.push({place:2090000000+i,nickname:rows[i],rating:0,change:0});
   check(Profile.presetsTake(out),"the presets' rows are taken");
  }
  // The mark's outline: the middle of its top edge, the lobby's yellow.
  private function yellow(m:Object):Boolean {
   var b:BitmapData=new BitmapData(Math.ceil(m.width)+2,Math.ceil(m.height)+2,true,0);
   b.draw(m as DisplayObject);var c:uint=b.getPixel(int(m.width/2),1);b.dispose();
   var near:Function=function(s:int):Boolean{return Math.abs((c>>s&0xFF)-(YELLOW>>s&0xFF))<=8;};
   trace("CONTAINER outline colour "+c.toString(16));
   return near(16)&&near(8)&&near(0);
  }
  private function on(m:Object,quick:int):Boolean {
   var a:Rectangle=slot(quick).getBounds(doll as DisplayObject);
   return m.visible && m.slot==slot(quick) && Math.abs(m.x-a.x)<=1 && Math.abs(m.y-a.y)<=1 && Math.abs(m.width-a.width)<=2;
  }
  // The paper doll alone, drawn over everything for the capture (the lobby's own panels lie over it).
  private var shot:Bitmap;
  private function capture(name:String):void {
   if(shot!=null) removeChild(shot);
   var d:DisplayObject=doll as DisplayObject;var r:Rectangle=d.getBounds(d);
   var b:BitmapData=new BitmapData(W,H,false,0x1B201E);
   b.draw(d,new Matrix(1,0,0,1,10-r.x,10-r.y));
   shot=new Bitmap(b);addChild(shot);
   trace("CONTAINER shot "+name);
  }
  private function hint(m:Object):String {var tip:Object=(m as DisplayObjectContainer).getChildAt(0);return tip.tipData?String(tip.tipData.name):"";}
  private function ask(state:int):void {asked.push(state);}
  private function clickSlot(quick:int):void {var e:*=new (C("scaleform.gfx.MouseEventEx"))("click");e.buttonIdx=0;slot(quick).dispatchEvent(e);}
  private function tick(e:Event):void {
   if(++frames<10 || --wait>0) return;
   wait=5;
   try {
    var m:Object=mark();
    switch(step++) {
     case 0:
      check(m!=null,"the paper doll has the mark");
      // Kit 2: three pockets and the skill's container after them.
      kit(2,3,1);list("K;1:1,2:1,3:0");
      break;
     case 1:
      trace("CONTAINER container: "+m.visible+" on "+(m.slot?m.slot.name:null)+", "+int(m.width)+" x "+int(m.height));
      check(on(m,4),"as the container: on the first container, slot 4");
      check(yellow(m),"the outline is the lobby's yellow 0xFFD23F");
      check(m.mouseEnabled,"as the container the mark takes the mouse (the slot is locked)");
      check(hint(m)=="Click: pocket","the hint as the container: "+hint(m));
      capture("container");wait=60;break;
     case 2:
      // A click asks for the pocket, once while it waits.
      doll.containerSlot(1,ask);
      m.dispatchEvent(new MouseEvent(MouseEvent.CLICK,true,false,10,10));
      m.dispatchEvent(new MouseEvent(MouseEvent.CLICK,true,false,10,10));
      check(asked.join()=="2","a click asks for the pocket, once while it waits: "+asked);
      // srp's answer: four pockets, the last one the skill's.
      kit(2,4,0);doll.containerSlot(2,ask);
      break;
     case 3:
      trace("CONTAINER pocket: "+m.visible+" on "+(m.slot?m.slot.name:null));
      check(on(m,4),"as the pocket: on the last pocket, slot 4");
      check(yellow(m),"the pocket's outline is the same yellow");
      check(!m.mouseEnabled,"as the pocket the slot keeps the mouse (drops, its item)");
      slot(4).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
      check(hint(m)=="Click: artifact container","the hint as the pocket: "+hint(m));
      slot(4).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
      asked.length=0;
      clickSlot(3);
      check(asked.length==0,"another pocket's click asks nothing");
      clickSlot(4);
      check(asked.join()=="1","the empty pocket's click asks for the container: "+asked);
      // Holding an item, a click is the item's: nothing asked.
      doll.containerSlot(2,ask);m.place(slot(4),true);asked.length=0;
      clickSlot(4);
      check(asked.length==0,"a pocket holding an item keeps it: "+asked);
      m.place(slot(4),false);
      capture("pocket");wait=60;break;
     case 4:
      // The `K` row for another kit: kit 3 without the skill's slot, kit 1 as the container.
      list("K;1:1,2:2,3:0");kit(3,1,0);
      break;
     case 5:
      check(!m.visible,"a kit with no such slot: no mark");
      kit(1,2,1);list("K;1:1,2:2,3:0");
      break;
     case 6:
      check(on(m,3),"kit 1 as the container: on slot 3");
      // Without the skill (every kit 0): none.
      list("K;1:0,2:0,3:0");
      break;
     case 7:
      check(!m.visible,"without the skill: no mark");
      // Every language's two hints.
      for each(var lang:String in HarnessText.LANGUAGES) {
       var t:Object=HarnessText.DATA[lang];
       var a:String=t["st_srp_loadout_slot_to_pocket"], b:String=t["st_srp_loadout_slot_to_container"];
       trace("CONTAINER "+lang+": '"+a+"', '"+b+"'");
       check(a!=null && a.length>0 && a.length<=24 && b!=null && b.length>0 && b.length<=40,lang+": the hints");
      }
      finish(failures.length==0?"pass":"fail: "+failures.join("; "));
      break;
    }
   } catch(err:Error) {finish("fail at step "+(step-1)+": "+err+" "+err.getStackTrace());}
  }
  private function finish(result:String):void {
   removeEventListener(Event.ENTER_FRAME,tick);
   trace("CONTAINER RESULT "+result);
  }
 }
}
