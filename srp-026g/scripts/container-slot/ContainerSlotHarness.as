package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.*;
 import flash.system.*;
 import flash.text.*;
 import flash.utils.*;
 // Artifact Container's slot in the kit (docs/notes/artefact-skills.md, "Artifact Container (522)"),
 // with the lobby movie's own player panel (SrvLocalPlayerProfile) and paper doll: srp's `K` row of
 // the presets list (LocalPlayerProfile.presetsTake) for the kit shown, the mark on the first
 // container as the container and on the last pocket as the pocket, the hint, a click showing the
 // other look at once and asking for it (the mark's ask, given here in place of the exe's request),
 // one ask at a time, the slot kept while srp's answer comes (an older list's word changes nothing),
 // a pocket holding an item kept as it is, no mark without the skill or on another kit; every
 // language's two hints. Then the mark before (the first look, a flat 2 px border) and after (the
 // soft edge) in both states, at rest and on the hover, on two sheets: the slot with its
 // neighbours at its own size, and the slot at three times.
 // The slot labels are the stock English (english.tsv). Traces CONTAINER lines, `CONTAINER shot
 // <name>` when a capture is due, and `CONTAINER RESULT pass|fail: ...`.
 public class ContainerSlotHarness extends Sprite {
  private static const W:int=1800, H:int=700;
  private static const LOOKS:Array=["before: flat border","after: soft edge"];
  private static const CELLS:Array=["container","container, hover","pocket","pocket, hover"];
  private var loader:Loader=new Loader();
  private var domain:ApplicationDomain;
  private var lobby:Object, profile:Object, doll:Object, Profile:Object, Mark:Object;
  private var stock:Object={};
  private var asked:Array=[];
  private var failures:Array=[];
  private var step:int=0, wait:int=0, frames:int=0;
  private var sheet:Bitmap, zoom:Bitmap;
  public function ContainerSlotHarness() {
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
    inventory.registerItemDescriptor(new (C("survarium.data.ItemDescriptor"))("container-slot-check"));
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
  private function on(m:Object,quick:int):Boolean {
   var a:Rectangle=slot(quick).getBounds(doll as DisplayObject);
   return m.visible && m.slot==slot(quick) && Math.abs(m.x-a.x)<=1 && Math.abs(m.y-a.y)<=1;
  }
  private function hint(m:Object):String {var c:DisplayObjectContainer=m as DisplayObjectContainer;var tip:Object=c.getChildAt(c.numChildren-1);return tip.tipData?String(tip.tipData.name):"";}
  private function ask(state:int):void {asked.push(state);}
  private function clickSlot(quick:int):void {var e:*=new (C("scaleform.gfx.MouseEventEx"))("click");e.buttonIdx=0;slot(quick).dispatchEvent(e);}
  // The two sheets: a row a look, a column a state (at rest, on the hover).
  private function sheets():void {
   sheet=new Bitmap(new BitmapData(W,H,false,0x14181B));
   zoom=new Bitmap(new BitmapData(W,H,false,0x14181B));
   var q:Rectangle=quickArea(), z:Rectangle=zoomArea();
   for(var r:int=0;r<LOOKS.length;r++) {label(sheet.bitmapData,LOOKS[r],12,cellY(r,q.height)+q.height/2);label(zoom.bitmapData,LOOKS[r],12,cellY(r,z.height*3)+z.height*1.5);}
   for(var c:int=0;c<CELLS.length;c++) {label(sheet.bitmapData,CELLS[c],cellX(c,q.width),10);label(zoom.bitmapData,CELLS[c],cellX(c,z.width*3),10);}
  }
  private function label(b:BitmapData,text:String,x:Number,y:Number):void {
   var t:TextField=new TextField();t.defaultTextFormat=new TextFormat("_sans",14,0xC8D0D6);t.autoSize=TextFieldAutoSize.LEFT;t.text=text;
   b.draw(t,new Matrix(1,0,0,1,x,y));
  }
  private function cellX(c:int,w:Number):Number {return 150+c*(w+24);}
  private function cellY(r:int,h:Number):Number {return 40+r*(h+24);}
  // The marked slot with its neighbours, and the slot alone, in the paper doll's units.
  private function quickArea():Rectangle {var a:Rectangle=slot(4).getBounds(doll as DisplayObject);a.inflate(60,90);return a;}
  private function zoomArea():Rectangle {var a:Rectangle=slot(4).getBounds(doll as DisplayObject);a.inflate(6,6);return a;}
  // A cell of look r and column c; the first row is the first look, a 2 px frame of the full
  // yellow drawn here over the slot without the mark (the movie no longer has it).
  private function cell(r:int,c:int):void {
   var d:DisplayObject=doll as DisplayObject;
   var mk:DisplayObject=mark() as DisplayObject;
   mk.visible=r!=0;
   var s:Rectangle=slot(4).getBounds(d);
   var q:Rectangle=quickArea();
   var m:Matrix=new Matrix(1,0,0,1,cellX(c,q.width)-q.x,cellY(r,q.height)-q.y);
   sheet.bitmapData.draw(d,m,null,null,new Rectangle(cellX(c,q.width),cellY(r,q.height),q.width,q.height));
   if(r==0) frame(sheet.bitmapData,cellX(c,q.width)+s.x-q.x,cellY(r,q.height)+s.y-q.y,s.width,s.height,1);
   var z:Rectangle=zoomArea();
   m=new Matrix(3,0,0,3,cellX(c,z.width*3)-z.x*3,cellY(r,z.height*3)-z.y*3);
   zoom.bitmapData.draw(d,m,null,null,new Rectangle(cellX(c,z.width*3),cellY(r,z.height*3),z.width*3,z.height*3),false);
   if(r==0) frame(zoom.bitmapData,cellX(c,z.width*3)+(s.x-z.x)*3,cellY(r,z.height*3)+(s.y-z.y)*3,s.width*3,s.height*3,3);
   mk.visible=true;
  }
  private function frame(b:BitmapData,x:Number,y:Number,w:Number,h:Number,scale:Number):void {
   var t:Number=2*scale;
   for each(var r:Rectangle in [new Rectangle(x,y,w,t),new Rectangle(x,y+h-t,w,t),new Rectangle(x,y,t,h),new Rectangle(x+w-t,y,t,h)]) b.fillRect(r,0xFFFFD23F);
  }

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
      check(m.shown==Mark.CONTAINER && m.mouseEnabled,"as the container the mark takes the mouse (the slot is locked)");
      check(hint(m)=="Click: pocket","the hint as the container: "+hint(m));
      // A click: the pocket shown at once and asked for, once while it waits.
      doll.containerSlot(2,1,ask);
      m.dispatchEvent(new MouseEvent(MouseEvent.CLICK,true,false,10,10));
      m.dispatchEvent(new MouseEvent(MouseEvent.CLICK,true,false,10,10));
      check(asked.join()=="2","a click asks for the pocket, once while it waits: "+asked);
      check(m.shown==Mark.POCKET,"the pocket shown at once");
      // An older list on its way says the container still: nothing changes.
      list("K;1:1,2:1,3:0");
      check(m.shown==Mark.POCKET,"an older word changes nothing while the click is asked");
      // srp's answer: the account first (four pockets), the slot kept where it is.
      kit(2,4,0);
      break;
     case 2:
      check(on(m,4),"the account before the `K` row: the mark stays on slot 4");
      list("K;1:1,2:2,3:0");
      break;
     case 3:
      trace("CONTAINER pocket: "+m.visible+" on "+(m.slot?m.slot.name:null));
      check(on(m,4),"as the pocket: on the last pocket, slot 4");
      check(!m.mouseEnabled,"as the pocket the slot keeps the mouse (drops, its item)");
      slot(4).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
      check(hint(m)=="Click: artifact container","the hint as the pocket: "+hint(m));
      slot(4).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
      doll.containerSlot(2,2,ask);asked.length=0;
      clickSlot(3);
      check(asked.length==0,"another pocket's click asks nothing");
      // Holding an item, a click is the item's: nothing asked.
      m.place(slot(4),true);
      clickSlot(4);
      check(asked.length==0,"a pocket holding an item keeps it: "+asked);
      m.place(slot(4),false);
      clickSlot(4);
      check(asked.join()=="1","the empty pocket's click asks for the container: "+asked);
      list("K;1:1,2:1,3:0");kit(2,3,1);
      break;
     case 4:
      check(on(m,4) && m.shown==Mark.CONTAINER,"the container again, on slot 4");
      // The `K` row for another kit: kit 3 without the skill's slot.
      list("K;1:1,2:2,3:0");kit(3,1,0);
      break;
     case 5:
      check(!m.visible,"a kit with no such slot: no mark");
      kit(1,2,1);list("K;1:1,2:2,3:0");
      break;
     case 6:
      check(on(m,3),"kit 1 as the container: on slot 3");
      list("K;1:0,2:0,3:0");
      break;
     case 7:
      check(!m.visible,"without the skill: no mark");
      for each(var lang:String in HarnessText.LANGUAGES) {
       var t:Object=HarnessText.DATA[lang];
       var a:String=t["st_srp_loadout_slot_to_pocket"], b:String=t["st_srp_loadout_slot_to_container"];
       check(a!=null && a.length>0 && a.length<=24 && b!=null && b.length>0 && b.length<=40,lang+": the hints");
      }
      sheets();
      break;
     case 200:
      removeChild(sheet);addChild(zoom);trace("CONTAINER shot zoom");wait=300;break;
     case 201:
      finish(failures.length==0?"pass":"fail: "+failures.join("; "));
      break;
     default:
      // The sheets: look r, cell c, a step to set it and one to draw it (the fades settled).
      var n:int=step-9, r:int=int(n/8), c:int=int(n%8/2);
      if(r>=LOOKS.length) {
       addChild(sheet);trace("CONTAINER shot looks");step=200;wait=300;break;
      }
      if(n%2==0) {
       var pocket:Boolean=c>=2;
       kit(2,pocket?4:3,pocket?0:1);doll.containerSlot(2,pocket?2:1,ask);
       m.hover(c%2==1);
       wait=12;
      } else cell(r,c);
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
