package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.*;
 import flash.system.*;
 import flash.text.*;
 import flash.utils.*;
 // The perks' slots in the kit (docs/notes/artefact-skills.md, "The slot a pocket again, by kit"),
 // with the lobby movie's own player panel (SrvLocalPlayerProfile) and paper doll: srp's `K` row of
 // the presets list (LocalPlayerProfile.presetsTake) for the kit shown, a perk each.
 // - Artifact Container: the mark on the first container as the container and on the last pocket
 //   as the pocket, the hint, a click showing the other look at once and asking for it (the mark's
 //   ask, given here in place of the exe's request), one ask at a time, the slot kept while srp's
 //   answer comes (an older list's word changes nothing), a pocket holding an item kept as it is,
 //   no mark without the skill or on another kit.
 // - Deep Pockets or Supply Pocket: taken, the yellow on the pocket it adds and the second weapon
 //   slot locked red; a click on the red lock opens the weapon slot at once (yellow) and closes the
 //   pocket (red, where it was), srp's answer moving nothing; a click on the red pocket, or on the
 //   empty weapon slot, takes it again; a weapon slot holding a weapon keeps its click.
 // Every language's hints. Then the looks before and after: Artifact Container's slot at three
 // times (zoom.png; before, the first flat 2 px border), the weapon perk's slots at their size
 // (weapon.png; before, the red lock alone). The slot labels are the stock English (english.tsv).
 // Traces PERK lines, `PERK shot <name>` when a capture is due, and `PERK RESULT pass|fail: ...`.
 public class PerkSlotsHarness extends Sprite {
  private static const W:int=2100, H:int=1450;
  private static const LOOKS:Array=["before","after"];
  private static const CONTAINER_CELLS:Array=["container","container, hover","pocket","pocket, hover"];
  private static const WEAPON_CELLS:Array=["perk: pocket","pocket, hover","second weapon","second weapon, hover"];
  private static const WEAPON_2:int=10;
  private var loader:Loader=new Loader();
  private var domain:ApplicationDomain;
  private var lobby:Object, profile:Object, doll:Object, Profile:Object, Mark:Object;
  private var stock:Object={};
  private var asked:Array=[];
  private var failures:Array=[];
  private var step:int=0, wait:int=0, frames:int=0;
  private var zoom:Bitmap, weapon:Bitmap;
  private var sheetSteps:Array=[];
  public function PerkSlotsHarness() {
   var tsv:URLLoader=new URLLoader();
   tsv.addEventListener(Event.COMPLETE,function(e:Event):void {
    // the stock slot labels: the movie's key is the dictionary's without `st_` (pd_artifact's is st_pd_artefact)
    for each(var line:String in String(tsv.data).split("\n")) {
     var f:Array=line.split("\t");
     if(f.length==2 && String(f[0]).indexOf("st_")==0) stock[String(f[0]).substr(3)]=f[1];
    }
    stock.pd_artifact=stock.pd_artefact;
    loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
   });
   tsv.load(new URLRequest("english.tsv"));
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void{e.preventDefault();});
  }
  private function C(name:String):Class {return domain.getDefinition(name) as Class;}
  private function check(ok:Boolean,why:String):void {if(!ok)failures.push(why);}
  private function texts(language:String):Object {var t:Object={};for(var k:String in stock)t[k]=stock[k];for(k in HarnessText.DATA[language])t[k]=HarnessText.DATA[language][k];return t;}
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
    profile=new (C("SrvLocalPlayerProfile"))();
    profile.setAmmoShare(domain.getDefinition("survarium.ammo.AmmoShareLite").getInstance());
    lobby.player_profile=profile;
    lobby.addChild(profile);
    var inventory:*=new (C("SrvInventoryReskin"))();
    inventory.name="inventory_list";
    lobby.inventory_list=inventory;
    lobby.addChild(inventory);
    inventory.x=W*3;
    inventory.registerItemDescriptor(new (C("survarium.data.ItemDescriptor"))("perk-slots-check"));
    profile.registerInventory(inventory);
    var shop:*=new (C("survarium.controls.GangShop"))();
    profile.paperDoll.ammoControl=new (C("survarium.ammo.AmmoShareControll"))();
    profile.paperDoll.ammoControl.registerShop(shop);
    profile.paperDoll.ammoControl.registerInventory(inventory);
    lobby.addChild(tips);
    doll=profile.paperDoll;
    Profile=domain.getDefinition("survarium.player.LocalPlayerProfile");
    Mark=domain.getDefinition("survarium.player.PerkSlotMark");
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error) {finish("setup at "+at+": "+err+" "+err.getStackTrace());}
  }
  // The marks, in the paper doll's order: Artifact Container's, then the weapon perk's.
  private function marks():Array {
   var d:DisplayObjectContainer=doll as DisplayObjectContainer, out:Array=[];
   for(var i:int=0;i<d.numChildren;i++) if(d.getChildAt(i) is (Mark as Class)) out.push(d.getChildAt(i));
   return out;
  }
  private function slot(quick:int):DisplayObject {return doll.getSlotAt(12+quick) as DisplayObject;}
  private function weaponSlot():DisplayObject {return doll.getSlotAt(WEAPON_2) as DisplayObject;}
  // The kit shown: its id and its pockets and containers, as the exe fills the panel.
  private function kit(id:int,pockets:int,containers:int):void {profile.fillProfileItems({profileId:id,items:[],item_slots:pockets,artifact_slots:containers});}
  private function list(k:String):void {
   var rows:Array=["P2;L;0;0;1;0;-1","1;1;0","2;0;-1","3;0;-1","4;0;-1","5;0;-1",k];
   var out:Array=[];for(var i:int=0;i<rows.length;i++) out.push({place:2090000000+i,nickname:rows[i],rating:0,change:0});
   check(Profile.presetsTake(out),"the presets' rows are taken");
  }
  private function on(m:Object,s:DisplayObject):Boolean {
   var a:Rectangle=s.getBounds(doll as DisplayObject);
   return m.visible && m.slot==s && Math.abs(m.x-a.x)<=1 && Math.abs(m.y-a.y)<=1;
  }
  private function hint(m:Object):String {var c:DisplayObjectContainer=m as DisplayObjectContainer;var tip:Object=c.getChildAt(c.numChildren-1);return tip.tipData?String(tip.tipData.name):"";}
  // The red lock over `s`: its tip's area (a click goes there), null: none.
  private function lockOn(s:DisplayObject):Object {
   var d:DisplayObjectContainer=doll as DisplayObjectContainer, at:Rectangle=s.getBounds(d);
   var Tip:Class=C("survarium.player.ReasonsTip"), Lock:Class=C("SkillLockSpriteRed");
   var area:Object=null, red:Boolean=false;
   for(var i:int=0;i<d.numChildren;i++) {
    var c:DisplayObject=d.getChildAt(i);
    if(c is Sprite && Sprite(c).numChildren==1 && Sprite(c).getChildAt(0) is Tip && Math.abs(c.x-at.x)<=1 && Math.abs(c.y-at.y)<=1) area=Sprite(c).getChildAt(0);
    if(c is Lock && c.visible) { var lb:Rectangle=c.getBounds(d); if(at.containsPoint(new Point(lb.x+lb.width/2,lb.y+lb.height/2))) red=true; }
   }
   return red?area:null;
  }
  private function ask(perk:int,state:int):void {asked.push(perk+":"+state);}
  private function clickOn(s:DisplayObject):void {var e:*=new (C("scaleform.gfx.MouseEventEx"))("click");e.buttonIdx=0;s.dispatchEvent(e);}
  private function label(b:BitmapData,text:String,x:Number,y:Number):void {
   var t:TextField=new TextField();t.defaultTextFormat=new TextFormat("_sans",14,0xC8D0D6);t.autoSize=TextFieldAutoSize.LEFT;t.text=text;
   b.draw(t,new Matrix(1,0,0,1,x,y));
  }
  // A sheet: a row a look, a column a state; a cell is `area` of the paper doll at `scale`.
  private function cellX(c:int,w:Number):Number {return 110+c*(w+24);}
  private function cellY(r:int,h:Number):Number {return 40+r*(h+24);}
  private function sheet(cells:Array,area:Rectangle,scale:Number):Bitmap {
   var b:Bitmap=new Bitmap(new BitmapData(W,H,false,0x14181B));
   for(var r:int=0;r<LOOKS.length;r++) label(b.bitmapData,LOOKS[r],12,cellY(r,area.height*scale)+area.height*scale/2);
   for(var c:int=0;c<cells.length;c++) label(b.bitmapData,cells[c],cellX(c,area.width*scale),10);
   return b;
  }
  private function cell(b:Bitmap,area:Rectangle,scale:Number,r:int,c:int):void {
   var x:Number=cellX(c,area.width*scale), y:Number=cellY(r,area.height*scale);
   b.bitmapData.draw(doll as DisplayObject,new Matrix(scale,0,0,scale,x-area.x*scale,y-area.y*scale),null,null,new Rectangle(x,y,area.width*scale,area.height*scale),false);
  }
  private function frame(b:Bitmap,area:Rectangle,scale:Number,r:int,c:int,s:DisplayObject):void {
   var a:Rectangle=s.getBounds(doll as DisplayObject);
   var x:Number=cellX(c,area.width*scale)+(a.x-area.x)*scale, y:Number=cellY(r,area.height*scale)+(a.y-area.y)*scale, w:Number=a.width*scale, h:Number=a.height*scale, t:Number=2*scale;
   for each(var q:Rectangle in [new Rectangle(x,y,w,t),new Rectangle(x,y+h-t,w,t),new Rectangle(x,y,t,h),new Rectangle(x+w-t,y,t,h)]) b.bitmapData.fillRect(q,0xFFFFD23F);
  }
  // The sheets' steps, a frame apart each pair (a state set, its fades settled, then drawn).
  private function planSheets():void {
   var cm:Object=marks()[0], wm:Object=marks()[1];
   var z:Rectangle=slot(4).getBounds(doll as DisplayObject);z.inflate(6,6);
   zoom=sheet(CONTAINER_CELLS,z,3);
   for(var r:int=0;r<2;r++) for(var c:int=0;c<4;c++) {
    var pocket:Boolean=c>=2, over:Boolean=c%2==1, before:Boolean=r==0, rr:int=r, cc:int=c;
    sheetSteps.push(function(pocket:Boolean,over:Boolean):Function {return function():void {kit(2,pocket?4:3,pocket?0:1);doll.perkSlots(2,[pocket?2:1,0],ask);cm.hover(over);};}(pocket,over));
    sheetSteps.push(function(before:Boolean,rr:int,cc:int):Function {return function():void {
     cm.visible=!before;cell(zoom,z,3,rr,cc);if(before) frame(zoom,z,3,rr,cc,slot(4));cm.visible=true;};}(before,rr,cc));
   }
   var a:Rectangle=weaponSlot().getBounds(doll as DisplayObject);
   for(var q:int=1;q<=4;q++) a=a.union(slot(q).getBounds(doll as DisplayObject));
   a.inflate(10,10);
   weapon=sheet(WEAPON_CELLS,a,1);
   for(r=0;r<2;r++) for(c=0;c<4;c++) {
    if(r==0 && c>0) continue;
    var declined:Boolean=c>=2;
    over=c%2==1;before=r==0;rr=r;cc=c;
    sheetSteps.push(function(declined:Boolean,over:Boolean):Function {return function():void {
     profile.enableSecondWeapon(false);kit(2,declined?1:2,0);doll.perkSlots(2,[0,declined?2:1],ask);wm.hover(over);};}(declined,over));
    sheetSteps.push(function(before:Boolean,rr:int,cc:int):Function {return function():void {
     wm.visible=!before;cell(weapon,a,1,rr,cc);wm.visible=true;};}(before,rr,cc));
   }
  }

  private function tick(e:Event):void {
   if(++frames<10 || --wait>0) return;
   wait=5;
   try {
    var ms:Array=marks(), m:Object=ms[0], wm:Object=ms[1];
    switch(step++) {
     case 0:
      check(ms.length==2,"the paper doll has a mark a perk");
      // Kit 2: three pockets and the skill's container after them.
      kit(2,3,1);list("K;1:1:0,2:1:0,3:0:0");
      break;
     case 1:
      trace("PERK container: "+m.visible+" on "+(m.slot?m.slot.name:null)+", "+int(m.width)+" x "+int(m.height));
      check(on(m,slot(4)),"as the container: on the first container, slot 4");
      check(m.shown==Mark.TAKEN && m.mouseEnabled,"as the container the mark takes the mouse (the slot is locked)");
      check(hint(m)=="Click: pocket","the hint as the container: "+hint(m));
      check(!wm.visible,"no weapon perk: no mark");
      // A click: the pocket shown at once and asked for, once while it waits.
      doll.perkSlots(2,[1,0],ask);
      m.dispatchEvent(new MouseEvent(MouseEvent.CLICK,true,false,10,10));
      m.dispatchEvent(new MouseEvent(MouseEvent.CLICK,true,false,10,10));
      check(asked.join()=="0:2","a click asks for the pocket, once while it waits: "+asked);
      check(m.shown==Mark.DECLINED,"the pocket shown at once");
      // An older list on its way says the container still: nothing changes.
      list("K;1:1:0,2:1:0,3:0:0");
      check(m.shown==Mark.DECLINED,"an older word changes nothing while the click is asked");
      // srp's answer: the account first (four pockets), the slot kept where it is.
      kit(2,4,0);
      break;
     case 2:
      check(on(m,slot(4)),"the account before the `K` row: the mark stays on slot 4");
      list("K;1:1:0,2:2:0,3:0:0");
      break;
     case 3:
      trace("PERK pocket: "+m.visible+" on "+(m.slot?m.slot.name:null));
      check(on(m,slot(4)),"as the pocket: on the last pocket, slot 4");
      check(!m.mouseEnabled,"as the pocket the slot keeps the mouse (drops, its item)");
      slot(4).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
      check(hint(m)=="Click: artifact container","the hint as the pocket: "+hint(m));
      slot(4).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
      doll.perkSlots(2,[2,0],ask);asked.length=0;
      clickOn(slot(3));
      check(asked.length==0,"another pocket's click asks nothing");
      // Holding an item, a click is the item's: nothing asked.
      m.place(slot(4),true);
      clickOn(slot(4));
      check(asked.length==0,"a pocket holding an item keeps it: "+asked);
      m.place(slot(4),false);
      clickOn(slot(4));
      check(asked.join()=="0:1","the empty pocket's click asks for the container: "+asked);
      list("K;1:1:0,2:1:0,3:0:0");kit(2,3,1);
      break;
     case 4:
      check(on(m,slot(4)) && m.shown==Mark.TAKEN,"the container again, on slot 4");
      // The `K` row for another kit: kit 3 without the skill's slot.
      list("K;1:1:0,2:2:0,3:0:0");kit(3,1,0);
      break;
     case 5:
      check(!m.visible,"a kit with no such slot: no mark");
      kit(1,2,1);list("K;1:1:0,2:2:0,3:0:0");
      break;
     case 6:
      check(on(m,slot(3)),"kit 1 as the container: on slot 3");
      list("K;1:0:0,2:0:0,3:0:0");
      break;
     case 7:
      check(!m.visible,"without the skill: no mark");
      // Deep Pockets: two pockets (the stock one and the perk's), the second weapon slot closed.
      profile.enableSecondWeapon(false);
      kit(2,2,0);list("K;1:0:1,2:0:1,3:0:1");doll.perkSlots(2,[0,1],ask);asked.length=0;
      break;
     case 8:
      trace("PERK weapon perk taken: "+wm.visible+" on "+(wm.slot?wm.slot.name:null));
      check(on(wm,slot(2)),"taken: the yellow on the pocket it adds, slot 2");
      check(!doll.getSlotAt(WEAPON_2).enabled,"taken: the second weapon slot closed");
      var lock:Object=lockOn(weaponSlot());
      check(lock!=null && String(lock.tipData.name).indexOf("Click: second weapon")>=0,"taken: the red lock on the second weapon slot, its hint: "+(lock?lock.tipData.name:null));
      // A press on the red lock: the second weapon at once, asked for once.
      lock.dispatchEvent(new MouseEvent(MouseEvent.CLICK,true));
      lock.dispatchEvent(new MouseEvent(MouseEvent.CLICK,true));
      check(asked.join()=="1:2","the red lock's click asks for the second weapon, once: "+asked);
      check(wm.shown==Mark.DECLINED,"the second weapon shown at once");
      break;
     case 9:
      check(doll.getSlotAt(WEAPON_2).enabled,"declined: the second weapon slot open at once");
      check(on(wm,weaponSlot()),"declined: the yellow on the second weapon slot");
      check(lockOn(slot(2))!=null,"declined: the pocket closed red where it was, slot 2");
      check(lockOn(weaponSlot())==null,"declined: no red lock on the weapon slot");
      // srp's answer: the account first (one pocket), nothing moves.
      kit(2,1,0);
      break;
     case 10:
      check(lockOn(slot(2))!=null && on(wm,weaponSlot()),"the account before the `K` row: nothing moves");
      list("K;1:0:1,2:0:2,3:0:1");doll.perkSlots(2,[0,2],ask);asked.length=0;
      break;
     case 11:
      check(lockOn(slot(2))!=null && slot(2).visible,"the closed pocket shown, red, after the pockets");
      weaponSlot().dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
      check(hint(wm)=="Click: pocket","the hint on the second weapon slot: "+hint(wm));
      weaponSlot().dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
      // A weapon in the slot: its click is the weapon's.
      wm.place(weaponSlot(),true);
      clickOn(weaponSlot());
      check(asked.length==0,"a weapon slot holding a weapon keeps it: "+asked);
      wm.place(weaponSlot(),false);
      // The red pocket's click takes the perk again.
      lockOn(slot(2)).dispatchEvent(new MouseEvent(MouseEvent.CLICK,true));
      check(asked.join()=="1:1","the red pocket's click asks for the pocket: "+asked);
      kit(2,2,0);list("K;1:0:1,2:0:1,3:0:1");
      break;
     case 12:
      check(on(wm,slot(2)) && !doll.getSlotAt(WEAPON_2).enabled && lockOn(weaponSlot())!=null,"taken again: the pocket yellow, the weapon slot red");
      check(lockOn(slot(3))==null,"no red pocket once taken");
      for each(var lang:String in HarnessText.LANGUAGES) {
       var t:Object=HarnessText.DATA[lang];
       for each(var key:String in ["st_srp_loadout_slot_to_pocket","st_srp_loadout_slot_to_container","st_srp_loadout_slot_to_weapon"]) {
        check(t[key]!=null && String(t[key]).length>0 && String(t[key]).length<=40,lang+": "+key);
       }
      }
      planSheets();
      break;
     case 200:
      addChild(zoom);trace("PERK shot zoom");wait=300;break;
     case 201:
      removeChild(zoom);addChild(weapon);trace("PERK shot weapon");wait=300;break;
     case 202:
      finish(failures.length==0?"pass":"fail: "+failures.join("; "));
      break;
     default:
      if(sheetSteps.length==0) {step=200;break;}
      sheetSteps.shift()();
      wait=12;
      break;
    }
   } catch(err:Error) {finish("fail at step "+(step-1)+": "+err+"; before it: "+failures.join("; "));}
  }
  private function finish(result:String):void {
   removeEventListener(Event.ENTER_FRAME,tick);
   trace("PERK RESULT "+result);
  }
 }
}
