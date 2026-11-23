package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.*;
 import flash.system.*;
 import flash.text.*;
 import flash.utils.*;
 // The perks' slots in the kit (docs/notes/artefact-skills.md, "The slot a pocket again, by kit"),
 // switched by the right mouse button (a left click asks nothing),
 // with the lobby movie's own player panel (SrvLocalPlayerProfile) and paper doll: srp's `K` row of
 // the presets list (LocalPlayerProfile.presetsTake) for the kit shown, a perk each.
 // - Artifact Container: the mark on the first container as the container and on the last pocket
 //   as the pocket, the hint, a click showing the other look at once and asking for it (the mark's
 //   ask, given here in place of the exe's request), one ask at a time, the slot kept while srp's
 //   answer comes (an older list's word changes nothing), a pocket holding an item kept as it is,
 //   no mark without the skill or on another kit.
 // - Deep Pockets or Supply Pocket: taken, the yellow on the pocket it adds and the second weapon
 //   slot locked red; a click on the red lock opens the weapon slot at once (yellow) and closes the
 //   pocket (red, where it was), srp's answer moving nothing, the pocket's item sent back with its
 //   slot shaken; a click on the red pocket takes it again; the open weapon slot keeps its own
 //   click.
 // - Every kit given as the exe gives it, its items then its counts (KitShown makes them one).
 //   From three pockets and a container to another kit's four, a medkit in the fourth: the items
 //   alone draw nothing, and the whole kit keeps the medkit, nothing sent back; so after a fill
 //   that carried its counts and a counts call, and after two kit switches in a row; counts alone,
 //   the kit drawn's pockets fewer (a smaller backpack), still send it back.
 // Every language's hints. Then the looks before and after: Artifact Container's slot at three
 // times (zoom.png; before, the first flat 2 px border), the weapon perk's slots at their size
 // (weapon.png; before, the red lock alone). The slot labels are the stock English (english.tsv).
 // Traces PERK lines, `PERK shot <name>` when a capture is due, and `PERK RESULT pass|fail: ...`.
 public class PerkSlotsHarness extends Sprite {
  private static const W:int=3400, H:int=1500;
  private static const LOOKS:Array=["before","after"];
  private static const CONTAINER_CELLS:Array=["container","container, hover","pocket","pocket, hover"];
  private static const WEAPON_CELLS:Array=["perk: pocket","pocket, hover","second weapon","second weapon, hover","pocket holding an item, hover"];
  private static const WEAPON_2:int=10;
  private var loader:Loader=new Loader();
  private var domain:ApplicationDomain;
  private var lobby:Object, profile:Object, doll:Object, Profile:Object, Mark:Object;
  private var stock:Object={};
  private var asked:Array=[];
  private var relocated:Array=[];
  private var descriptor:Object, inventory:Object;
  private static const MEDKIT:int=219, MEDKIT_ID:int=50, QUICK_4:int=16;
  private var failures:Array=[];
  private var step:int=0, wait:int=0, frames:int=0;
  private var zoom:Bitmap, weapon:Bitmap;
  private var sheetSteps:Array=[];
  // The lobby's tooltips' layer (ToolTip.tipContainer), drawn into the sheets' cells too.
  private var tipsLayer:Sprite;
  // The quick places (the slots' own at the start); one watched for its shake (whichever slot
  // is shown there), and the most it moved.
  private var places:Array, shakePlace:int=-1, shakenMost:Number=0;
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
    tipsLayer=tips;
    C("scaleform.clik.controls.ToolTip")["tipContainer"]=tips;
    var Container:Class=C("survarium.data.IconContainer");
    for each(var icons:String in ["equipment","camo","consumables","bundles","factionItemIcons"]) new Container(icons,1,1,new BitmapData(1,1,true,0),false,1,0,0,false);
    at="the panel";
    domain.getDefinition("survarium.controls.SrvContextMenu").registerStage(stage,lobby);
    profile=new (C("SrvLocalPlayerProfile"))();
    profile.setAmmoShare(domain.getDefinition("survarium.ammo.AmmoShareLite").getInstance());
    lobby.player_profile=profile;
    // The requests the profile's perk switch sends (kit x 10 + perk x 2 + state), as perk:state.
    profile.perkSwitch.send=function(r:int):void {var perk:int=int((r%10-1)/2);asked.push(perk+":"+(r%10-perk*2));};
    lobby.addChild(profile);
    inventory=new (C("SrvInventoryReskin"))();
    inventory.name="inventory_list";
    lobby.inventory_list=inventory;
    lobby.addChild(inventory);
    inventory.x=W*3;
    descriptor=new (C("survarium.data.ItemDescriptor"))("perk-slots-check");
    inventory.registerItemDescriptor(descriptor);
    // The relocations the panel asks of the exe, as item:source slot.
    profile._relocateFunction=function(moves:Array):void {for each(var r:Object in moves) relocated.push(r.id+":"+r.sourceSlot);};
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
  // As the exe gives it (on_profile_changed 0x222330): its items, then its counts.
  private function kit(id:int,pockets:int,containers:int):void {profile.fillProfileItems({profileId:id,items:[]});profile.updateSlots(containers,pockets);}
  // Kit `id`'s items: a medkit in the fourth pocket.
  private function medkitKit(id:int):void {profile.fillProfileItems({profileId:id,items:[{itemId:MEDKIT_ID,slotId:QUICK_4,condition_or_stack:4}]});}
  private function fourth():Object {return doll.getSlotAt(QUICK_4);}
  private function keptMedkit(how:String):void {
   check(relocated.length==0,how+": no pocket's item sent back: "+relocated);
   check(fourth().visible && fourth().enabled && fourth().data!=null && int(fourth().data.itemId)==MEDKIT_ID,how+": the fourth pocket shows its medkit");
  }
  private function list(k:String):void {
   var rows:Array=["P2;L;0;0;1;0;-1","1;1;0","2;0;-1","3;0;-1","4;0;-1","5;0;-1",k];
   var out:Array=[];for(var i:int=0;i<rows.length;i++) out.push({place:2090000000+i,nickname:rows[i],rating:0,change:0});
   check(Profile.presetsTake(out),"the presets' rows are taken");
  }
  // The answer to a perk switch: srp's list whose header names the request it answers
  // (`done` 3, set, or 4, refused; `preset` the request's kit x 10 + perk x 2 + state). The
  // profile then hands the paper doll its own way to ask (the harness's `ask` set again after).
  private function answer(k:String,request:int,refused:Boolean=false):void {
   var rows:Array=["P2;L;"+(refused?4:3)+";"+request+";1;0;-1","1;1;0","2;0;-1","3;0;-1","4;0;-1","5;0;-1",k];
   var out:Array=[];for(var i:int=0;i<rows.length;i++) out.push({place:2090000000+i,nickname:rows[i],rating:0,change:0});
   check(Profile.presetsTake(out),"the presets' rows are taken");
  }
  // What the quick slots show: each visible one by its number, a pocket's yellow (p), the
  // container's mark (c) or a red lock (r) on it; then the weapon slot's yellow (wW) or red (wR).
  // The quick slot shown at place `p` (1 the first), if one is visible there.
  private function atPlace(p:int):DisplayObject {
   for(var q:int=1;q<=6;q++) if(slot(q).visible && Math.abs(slot(q).x-places[p-1].x)<12 && slot(q).y==places[p-1].y) return slot(q);
   return null;
  }
  // Every quick slot at a place: none shaken off it.
  private function atRest():Boolean {
   for(var q:int=1;q<=6;q++) {
    var found:Boolean=false;
    for each(var pl:Point in places) found=found || (slot(q).x==pl.x && slot(q).y==pl.y);
    if(!found) return false;
   }
   return true;
  }
  private function layout():String {
   var ms:Array=marks(), out:Array=[];
   for(var p:int=1;p<=6;p++) {
    var s:DisplayObject=atPlace(p);
    if(s==null) continue;
    var t:String=String(p);
    if(ms[0].visible && ms[0].slot==s) t+=ms[0].shown==Mark.TAKEN?"c":"k";
    if(ms[1].visible && ms[1].slot==s) t+="p";
    if(lockOn(s)!=null) t+="r";
    out.push(t);
   }
   out.push(ms[1].visible && ms[1].slot==weaponSlot() ? "wW" : (lockOn(weaponSlot())!=null ? "wR" : "w"));
   return out.join(" ");
  }
  // The red lock drawn over `s` (the symbol's second child, its circle hidden) is in its centre.
  private function lockCentred(s:DisplayObject):Boolean {
   var d:DisplayObjectContainer=doll as DisplayObjectContainer, at:Rectangle=s.getBounds(d), Lock:Class=C("SkillLockSpriteRed");
   for(var i:int=0;i<d.numChildren;i++) {
    var c:DisplayObject=d.getChildAt(i);
    if(c is Lock && c.visible) {
     var shown:DisplayObject=Sprite(c).numChildren==2?Sprite(c).getChildAt(1):c, lb:Rectangle=shown.getBounds(d);
     if(at.containsPoint(new Point(lb.x+lb.width/2,lb.y+lb.height/2))) return Math.abs(lb.x+lb.width/2-(at.x+at.width/2))<=1 && Math.abs(lb.y+lb.height/2-(at.y+at.height/2))<=1;
    }
   }
   return false;
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
    if(c is Sprite && Sprite(c).numChildren==1 && Sprite(c).getChildAt(0) is Tip && Math.abs(c.x-at.x)<=8 && Math.abs(c.y-at.y)<=1) area=Sprite(c).getChildAt(0);
    if(c is Lock && c.visible) { var lb:Rectangle=c.getBounds(d); if(at.containsPoint(new Point(lb.x+lb.width/2,lb.y+lb.height/2))) red=true; }
   }
   return red?area:null;
  }
  // Kit 2's perks as srp lists them (no switch asked).
  private function states(container:int,weapon:int):void {list("K;1:0:0,2:"+container+":"+weapon+",3:0:0");}
  // What the paper doll draws, a frame: the layout, and each quick slot's place (the nearest, a
  // shake aside), visibility and alpha. A switch's frames are its old one, then its new one.
  private function drawn():String {
   var out:Array=[layout()];
   for(var q:int=1;q<=6;q++) {
    var at:int=-1;
    for(var i:int=0;i<places.length;i++) if(slot(q).y==places[i].y && Math.abs(slot(q).x-places[i].x)<12) at=i;
    out.push(q+"@"+at+(slot(q).visible?"v":"h")+slot(q).alpha.toFixed(2));
   }
   return out.join(" ");
  }
  private var watched:Array=null;
  private function watch():void {watched=[drawn()];}
  // The frames since watch(): the old drawing, then `after`'s layout, nothing between.
  private function watchedOnly(after:String,what:String):void {
   var seen:Array=[];
   for each(var d:String in watched) if(seen.length==0 || seen[seen.length-1]!=d) seen.push(d);
   var last:String=seen[seen.length-1];
   check(seen.length<=2 && last.indexOf(after+" ")==0,what+": the frames: "+seen.join(" / "));
   watched=null;
  }
  // A click as Scaleform gives it, of the left button (0) or the right one (1): the switch is
  // the right one's (the owner, 2026-11-22: "lets make it so rmb switches not lmb").
  private function clickOn(s:Object,button:int=1):void {var e:*=new (C("scaleform.gfx.MouseEventEx"))("click");e.buttonIdx=button;s.dispatchEvent(e);}
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
   var m:Matrix=new Matrix(scale,0,0,scale,x-area.x*scale,y-area.y*scale), clip:Rectangle=new Rectangle(x,y,area.width*scale,area.height*scale);
   b.bitmapData.draw(doll as DisplayObject,m,null,null,clip,false);
   // the tooltips shown, where they are over the paper doll
   var t:Matrix=tipsLayer.transform.concatenatedMatrix.clone(), d:Matrix=DisplayObject(doll).transform.concatenatedMatrix.clone();
   d.invert();t.concat(d);t.concat(m);
   b.bitmapData.draw(tipsLayer,t,null,null,clip,true);
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
    sheetSteps.push(function(pocket:Boolean,over:Boolean):Function {return function():void {cm.hover(false);kit(2,pocket?4:3,pocket?0:1);states(pocket?2:1,0);cm.hover(over);};}(pocket,over && !before));
    sheetSteps.push(function(before:Boolean,rr:int,cc:int):Function {return function():void {
     cm.visible=!before;cell(zoom,z,3,rr,cc);if(before) frame(zoom,z,3,rr,cc,slot(4));cm.visible=true;};}(before,rr,cc));
   }
   var a:Rectangle=weaponSlot().getBounds(doll as DisplayObject);
   for(var q:int=1;q<=4;q++) a=a.union(slot(q).getBounds(doll as DisplayObject));
   // room for the tips: above the slots, and right of them
   a.inflate(10,10);a.top-=40;a.right+=200;
   weapon=sheet(WEAPON_CELLS,a,1);
   for(r=0;r<2;r++) for(c=0;c<5;c++) {
    if(r==0 && c>0) continue;
    var declined:Boolean=c==2 || c==3, held:Boolean=c==4;
    over=c%2==1 || held;before=r==0;rr=r;cc=c;
    sheetSteps.push(function(declined:Boolean,over:Boolean):Function {return function():void {
     wm.hover(false);profile.enableSecondWeapon(false);kit(2,declined?1:2,0);states(0,declined?2:1);wm.hover(over);};}(declined,over && !before && !held));
    sheetSteps.push(function(before:Boolean,held:Boolean,rr:int,cc:int):Function {return function():void {
     // an item in the yellow pocket: its own card (the lobby's, not drawn here) and the slot's skill above it
     if(held) {wm.place(slot(2),true);wm.hover(true);}
     wm.visible=!before;cell(weapon,a,1,rr,cc);wm.visible=true;
     if(held) {wm.hover(false);wm.place(slot(2),false);}};}(before,held,rr,cc));
   }
  }

  private function tick(e:Event):void {
   if(shakePlace>=0) for(var sq:int=1;sq<=6;sq++) {
    var sp:Point=places[shakePlace], sd:Number=Math.abs(slot(sq).x-sp.x);
    if(slot(sq).y==sp.y && sd<12) shakenMost=Math.max(shakenMost,sd);
   }
   if(watched!=null) watched.push(drawn());
   if(++frames<10 || --wait>0) return;
   wait=5;
   try {
    var ms:Array=marks(), m:Object=ms[0], wm:Object=ms[1];
    switch(step++) {
     case 0:
      check(ms.length==2,"the paper doll has a mark a perk");
      places=[];for(var pq:int=1;pq<=6;pq++) places.push(new Point(slot(pq).x,slot(pq).y));
      // Kit 2: three pockets and the skill's container after them.
      kit(2,3,1);list("K;1:1:0,2:1:0,3:0:0");
      break;
     case 1:
      trace("PERK container: "+m.visible+" on "+(m.slot?m.slot.name:null)+", "+int(m.width)+" x "+int(m.height));
      check(on(m,slot(4)),"as the container: on the first container, slot 4");
      check(m.shown==Mark.TAKEN && m.mouseEnabled,"as the container the mark takes the mouse (the slot is locked)");
      check(hint(m)=="Artifact Container — Right-click: pocket","the tip as the container: its skill, what a right click does: "+hint(m));
      check(!wm.visible,"no weapon perk: no mark");
      // A left click does nothing here; a right click asks for the pocket, once while it waits, and
      // changes nothing until srp answers (the owner, 2026-11-23: "the operation is not atomic").
      clickOn(m,0);
      check(asked.length==0,"the container's left click asks nothing: "+asked);
      watch();
      clickOn(m);
      clickOn(m);
      check(asked.join()=="0:2","a click asks for the pocket, once while it waits: "+asked);
      check(m.shown==Mark.TAKEN,"nothing changes until srp answers");
      // An older list on its way says the container still; then srp's account (four pockets),
      // held until its answer.
      list("K;1:1:0,2:1:0,3:0:0");
      kit(2,4,0);
      break;
     case 2:
      check(layout()=="1 2 3 4c w","the account before its answer: held, the kit as it was: "+layout());
      answer("K;1:1:0,2:2:0,3:0:0",12);
      break;
     case 3:
      watchedOnly("1 2 3 4k w","the container to the pocket");
      trace("PERK pocket: "+m.visible+" on "+(m.slot?m.slot.name:null));
      check(on(m,slot(4)),"as the pocket: on the last pocket, slot 4");
      check(!m.mouseEnabled,"as the pocket the slot keeps the mouse (drops, its item)");
      slot(4).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
      check(hint(m)=="Artifact Container — Right-click: artifact container","the tip as the pocket: "+hint(m));
      slot(4).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
      asked.length=0;
      clickOn(slot(3));
      check(asked.length==0,"another pocket's click asks nothing");
      // Holding an item, a click is the item's: nothing asked.
      m.place(slot(4),true);
      clickOn(slot(4));
      check(asked.length==0,"a pocket holding an item keeps it: "+asked);
      m.place(slot(4),false);
      clickOn(slot(4),0);
      check(asked.length==0,"the empty pocket's left click is the slot's own: "+asked);
      watch();
      clickOn(slot(4));
      check(asked.join()=="0:1","the empty pocket's right click asks for the container: "+asked);
      kit(2,3,1);answer("K;1:1:0,2:1:0,3:0:0",11);
      break;
     case 4:
      watchedOnly("1 2 3 4c w","the pocket to the container");
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
      kit(2,2,0);list("K;1:0:1,2:0:1,3:0:1");asked.length=0;
      break;
     case 8:
      trace("PERK weapon perk taken: "+wm.visible+" on "+(wm.slot?wm.slot.name:null));
      check(on(wm,slot(2)),"taken: the yellow on the pocket it adds, slot 2");
      check(!doll.getSlotAt(WEAPON_2).enabled,"taken: the second weapon slot closed");
      var lock:Object=lockOn(weaponSlot());
      check(lock!=null && String(lock.tipData.name)=="Deep Pockets — Right-click: second weapon","taken: the red lock on the second weapon slot, its hint: "+(lock?lock.tipData.name:null));
      check(lockCentred(weaponSlot()),"taken: the red lock in the weapon slot's centre, as the stock game draws it");
      // A press on the red lock, a medkit in the yellow pocket: asked once; the medkit goes back,
      // its place shaken; nothing else changes until srp answers.
      wm.place(slot(2),true);wm.closing(slot(2),true);
      shakePlace=1;shakenMost=0;
      clickOn(lock,0);
      check(asked.length==0,"the red lock's left click asks nothing: "+asked);
      watch();
      clickOn(lock);
      clickOn(lock);
      check(asked.join()=="1:2","the red lock's click asks for the second weapon, once: "+asked);
      check(wm.shown==Mark.TAKEN,"nothing changes until srp answers");
      kit(2,1,0);
      break;
     case 9:
      check(shakenMost>=1,"the pocket the medkit left shakes: "+shakenMost);
      check(layout()=="1 2p wR","the account before its answer: held: "+layout());
      answer("K;1:0:1,2:0:2,3:0:1",14);
      wait=40;
      break;
     case 10:
      watchedOnly("1 2r wW","the pocket to the second weapon");
      check(doll.getSlotAt(WEAPON_2).enabled,"declined: the second weapon slot open");
      check(on(wm,weaponSlot()),"declined: the yellow on the second weapon slot");
      check(lockOn(weaponSlot())==null,"declined: no red lock on the weapon slot");
      check(atRest() && lockOn(slot(2))!=null && slot(2).visible,"declined: the pocket closed red where it was, slot 2");
      check(lockCentred(slot(2)),"declined: the red lock in the pocket's centre");
      // The open second weapon slot: its tip; a left click is its own; holding a weapon, the
      // weapon's tip line and menu (nothing asked); empty, a right click takes the pocket again.
      asked.length=0;
      weaponSlot().dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
      check(hint(wm)=="Deep Pockets — Right-click: pocket","the open weapon slot's tip: "+hint(wm));
      weaponSlot().dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
      clickOn(weaponSlot(),0);
      check(asked.length==0,"the empty second weapon slot's left click asks nothing: "+asked);
      wm.place(weaponSlot(),true);
      weaponSlot().dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
      check(hint(wm)=="Slot: Deep Pockets","a held weapon: the slot's skill beside its card: "+hint(wm));
      weaponSlot().dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
      clickOn(weaponSlot());
      check(asked.length==0,"a weapon slot holding a weapon keeps it: "+asked);
      wm.place(weaponSlot(),false);
      watch();
      clickOn(weaponSlot());
      check(asked.join()=="1:1","the empty second weapon slot's right click asks for the pocket: "+asked);
      kit(2,2,0);answer("K;1:0:1,2:0:1,3:0:1",13);
      break;
     case 11:
      watchedOnly("1 2p wR","the second weapon to the pocket");
      check(on(wm,slot(2)) && !doll.getSlotAt(WEAPON_2).enabled && lockOn(weaponSlot())!=null,"taken again: the pocket yellow, the weapon slot red");
      check(lockOn(slot(3))==null,"no red pocket once taken");
      for each(var lang:String in HarnessText.LANGUAGES) {
       var t:Object=HarnessText.DATA[lang];
       for each(var key:String in ["st_srp_loadout_slot_to_pocket","st_srp_loadout_slot_to_container","st_srp_loadout_slot_to_weapon","st_srp_loadout_slot_skill"]) {
        check(t[key]!=null && String(t[key]).length>0 && String(t[key]).length<=40,lang+": "+key);
       }
      }
      // The perk declined again (srp's word, no click), to try its red pocket.
      kit(2,1,0);states(0,2);asked.length=0;
      break;
     case 12:
      check(lockOn(slot(2))!=null && String(lockOn(slot(2)).tipData.name)=="Deep Pockets — Right-click: pocket","the red pocket's tip: "+(lockOn(slot(2))?lockOn(slot(2)).tipData.name:null));
      clickOn(lockOn(slot(2)),0);
      check(asked.length==0,"the red pocket's left click asks nothing: "+asked);
      watch();
      clickOn(lockOn(slot(2)));
      check(asked.join()=="1:1","the red pocket's right click asks for the pocket: "+asked);
      kit(2,2,0);answer("K;1:0:1,2:0:1,3:0:1",13);
      break;
     case 13:
      watchedOnly("1 2p wR","the red pocket to the pocket");
      // Both perks, Artifact Container declined and Deep Pockets taken: three pockets, by srp's
      // order (the owner, 2026-11-22: "[free pockets - 1] [additional pocket] [free pocket for
      // switching with artis]") the stock one, the added one, then the container's.
      kit(2,3,0);states(2,1);asked.length=0;
      break;
     case 14:
      trace("PERK both: "+layout());
      check(layout()=="1 2p 3k wR","both: the added pocket, then the container's: "+layout());
      // A medkit in the container's pocket: its right click is its menu (nothing asked). One in the
      // added pocket: the red lock's sends it back, the added pocket's place shaken.
      m.place(slot(3),true);
      clickOn(slot(3));
      check(asked.length==0,"both: the container's pocket holding an item keeps it: "+asked);
      wm.closing(slot(2),true);
      shakePlace=1;shakenMost=0;
      watch();
      clickOn(lockOn(weaponSlot()));
      check(asked.join()=="1:2","both: the red lock asks for the second weapon: "+asked);
      kit(2,2,0);
      wait=40;
      break;
     case 15:
      check(shakenMost>=1 && atRest(),"both: the added pocket's place shakes: "+shakenMost);
      check(layout()=="1 2p 3k wR","both: the account before its answer: held: "+layout());
      answer("K;1:0:1,2:2:2,3:0:1",14);
      break;
     case 16:
      // The added pocket blocked in its place (the owner, 2026-11-22), the container's last; the
      // drawing goes from the one to the other in one frame.
      trace("PERK both declined: "+layout());
      watchedOnly("1 2r 3k wW","both: the added pocket to the second weapon");
      // Rapid clicks (the owner, 2026-11-22: "when I click too fast, 'fake' pocket on top can
      // appear for a moment", "there is data race there somewhere"): both perks taken, one
      // pocket and Artifact Container's container.
      kit(2,1,1);states(1,1);asked.length=0;
      break;
     case 17:
      trace("PERK race before: "+layout());
      check(layout()=="1p 2c wR","race: before: "+layout());
      // The red weapon lock and, at once, the container: one switch at a time a kit.
      watch();
      clickOn(lockOn(weaponSlot()));
      clickOn(m);
      check(asked.join()=="1:2","race: a second switch while the first is asked is not sent: "+asked);
      // A list asked before (the movie asks one at every account) comes first, the old state;
      // then srp's account for the weapon switch, held.
      list("K;1:0:1,2:1:1,3:0:1");
      kit(2,0,1);
      break;
     case 18:
      trace("PERK race account: "+layout());
      check(layout()=="1p 2c wR","race: the account before its answer: held: "+layout());
      answer("K;1:0:1,2:1:2,3:0:1",14);
      break;
     case 19:
      trace("PERK race answered: "+layout());
      watchedOnly("1r 2c wW","race: answered");
      // Now the container switch is taken; refused (srp's word: nothing changed).
      asked.length=0;watch();clickOn(m);
      check(asked.join()=="0:2","race: the next switch once answered: "+asked);
      answer("K;1:0:1,2:1:2,3:0:1",12,true);
      break;
     case 20:
      trace("PERK race refused: "+layout());
      check(m.shown==Mark.TAKEN,"race: a refused switch changes nothing");
      watchedOnly("1r 2c wW","race: refused");
      // The account a switch asked for another kit shown meanwhile is no longer held.
      asked.length=0;clickOn(m);
      check(asked.join()=="0:2","another kit: the switch asked: "+asked);
      kit(1,2,1);list("K;1:1:0,2:1:2,3:0:1");
      break;
     case 21:
      check(on(m,slot(3)),"another kit shown while a switch is asked: drawn at once: "+layout());
      answer("K;1:1:0,2:1:2,3:0:1",12,true);
      // Kit 2 with three pockets and a container, then kit 1 with four, a medkit in the fourth.
      descriptor.setItemsDictionary([{dictId:MEDKIT,category:C("survarium.constants.ItemCategory").MEDKITS,name:"Medkit",level:1,icon_row:1,icon_column:1,is_stack:true,modifiers:[],item_properties:[]}]);
      inventory.fillSellers([]);
      inventory.sellersTabPage.selectedIndex=0;
      inventory.toggleShopMode(false);
      inventory.filterItems(null);
      inventory.optionControl.checkbox_all_traders.selected=true;
      inventory.setupShopItems([]);
      inventory.setupBundleData([]);
      inventory.setupInventoryData([{id:MEDKIT_ID,dictId:MEDKIT,faction_id:0,condition_or_stack:20,modifiers:[],rarity_level:0,repair_time:0,reroll_time:0,is_attached_to:0}]);
      kit(2,3,1);list("K;1:0:0,2:0:0,3:0:0");
      break;
     case 22:
      // As the exe shows another kit (0x222330): its items, then its counts. The items alone
      // draw nothing (they wait for their counts).
      relocated.length=0;
      medkitKit(1);
      check(fourth().data==null && relocated.length==0,"the items alone: nothing drawn, nothing sent back: "+relocated);
      profile.updateSlots(0,4);
      break;
     case 23:
      keptMedkit("the exe's order: items, then counts");
      // A fill that carries its counts, then a counts call anyway (the same kit's, drawn again):
      // the next kit still pairs its own items and counts.
      profile.fillProfileItems({profileId:2,items:[],item_slots:3,artifact_slots:1});
      profile.updateSlots(1,3);
      medkitKit(1);
      profile.updateSlots(0,4);
      break;
     case 24:
      keptMedkit("after a fill that carried its counts and a counts call");
      // Two kit switches in a row, in one go: kit 1 to kit 2 and back.
      kit(2,3,1);
      medkitKit(1);
      profile.updateSlots(0,4);
      break;
     case 25:
      keptMedkit("two kit switches in a row");
      // Counts alone after a draw: the kit drawn's own (a smaller backpack put on), its fourth
      // pocket lost: its item goes back, as in the stock lobby. Ruffle stops in the stock
      // send-back (ItemPaperDollSlot.dropSlot reads ItemData's condition_or_stack, which it lacks;
      // Scaleform reads undefined and sends the relocation), so its error is the send-back here.
      var sentBack:String="";
      try {profile.updateSlots(0,3);} catch(err:Error) {sentBack=String(err);}
      check(sentBack.indexOf("condition_or_stack")>=0 || relocated.length==1,"the same kit's pocket lost: its item sent back: "+sentBack+" "+relocated);
      break;
     case 26:
      kit(2,0,1);
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
