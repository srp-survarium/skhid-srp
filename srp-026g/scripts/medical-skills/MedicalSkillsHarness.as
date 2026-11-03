package {
 import flash.display.*;
 import flash.events.*;
 import flash.net.*;
 import flash.system.*;
 public class MedicalSkillsHarness extends Sprite {
  private var loader:Loader = new Loader();
  private var failures:Array = [];
  private var icons:Object;
  private var frames:int = 0;
  public function MedicalSkillsHarness() {
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void {e.preventDefault();});
   loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function check(value:Boolean,label:String):void {if(!value) failures.push(label);}
  private function loaded(e:Event):void {
   try {
    var domain:ApplicationDomain=loader.contentLoaderInfo.applicationDomain;
    var Info:Object=domain.getDefinition("survarium.controls.PlayerInfo");
    var Doll:Object=domain.getDefinition("survarium.player.LocalPaperDoll");
    var Data:Class=domain.getDefinition("survarium.data.ItemData") as Class;
    var size:Object=domain.getDefinition("survarium.constants.QuickSlotSize");
    var Slots:Object=domain.getDefinition("survarium.constants.ProfileSlot");
    for each(var count:int in [0,1,4,5,6,7,20]) {
     check(Slots.pocketCount(count)==Math.min(count,6),"six-pocket cap " + count);
     check(Slots.artifactCount(count,8)==6-Math.min(count,6),"six combined positions " + count);
    }
    check(Slots.artifactCount(5,0)==0,"no free artifact unlock");
    check(Slots.artifactCount(3,2)==2,"existing artifact capacity");
    Info.pocketSize=3;
    Info.medicalSkills([]);
    check(Info.pocketUnits(10,size.HALF)==3,"base medicine");
    check(Info.pocketUnits(10,size.WHOLE)==1,"base whole pocket");
    Info.medicalSkills([{id:322,value:0,newValue:1}]);
    check(Info.pocketUnits(10,size.HALF)==3,"uncommitted preview");
    Info.medicalSkills([{id:322,value:1}]);
    check(Info.pocketUnits(10,size.HALF)==4,"medicine bonus");
    check(Info.pocketUnits(10,size.WHOLE)==1,"booster stays one");
    check(Info.pocketUnits(11,size.HALF)==3,"nonmedicine unchanged");
    var held:Object=new Data();held.dictId=67;held.value=3;
    var item:Object=new Data();item.dictId=67;item.category=10;item.quickSlotItemSize=size.HALF;
    check(Doll.pocketUnits(held,item)==4,"drag adds fourth medicine");
    held.value=4;check(Doll.pocketUnits(held,item)==4,"drag capacity limit");
    item.dictId=68;check(Doll.pocketUnits(held,item)==1,"different medicine starts at one");
    Info.medicalSkills([]);
    check(Info.pocketUnits(10,size.HALF)==3,"reset removes bonus");
    for each(var capacity:int in [4,5,6,20]) {
     Info.pocketSize=capacity;
     Info.medicalSkills([]);
     check(Info.pocketUnits(10,size.HALF)==Math.min(5,capacity),"plain capacity cap " + capacity);
     Info.medicalSkills([{id:322,value:1}]);
     check(Info.pocketUnits(10,size.HALF)==5,"medicine bonus capped " + capacity);
     check(Info.pocketUnits(11,size.HALF)==Math.min(5,capacity),"grenade capacity cap " + capacity);
     check(Info.pocketUnits(10,size.WHOLE)==1,"booster stays one " + capacity);
     check(Info.pocketUnits(11,size.WHOLE)==1,"whole nonmedicine keeps one " + capacity);
     check(Info.pocketUnits(10,size.HALF,capacity)==5,"dialog override capped " + capacity);
     held.value=5;item.dictId=held.dictId;
     check(Doll.pocketUnits(held,item)==5,"cannot drag sixth medicine " + capacity);
    }
    var Icons:Class=domain.getDefinition("SrvSkillRendererCh") as Class;
    icons=new Icons();addChild(DisplayObject(icons));
    icons.data={id:341,name:"Medic",branch:2,level:5,maxValue:1,value:1,newValue:1,icon:14,description_1:"Healing",description_2:"they glow you shine",antiskills:[]};
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error) {failures.push("setup "+err);finish();}
  }
  private function tick(e:Event):void {
   if(++frames<3) return;
   removeEventListener(Event.ENTER_FRAME,tick);
   var glyph:Object=icons.iconContainer.getChildByName("medicalSkillIcon");
   check(glyph!=null && glyph.visible,"medical renderer uses original artwork");
   if(glyph!=null) {
    for each(var id:int in [301,302,311,312,321,322,331,332,341,342]) {
     glyph.show(id);
     var original:Bitmap = glyph.getChildAt(0) as Bitmap;
     check(original != null && original.bitmapData != null,"medical bitmap " + id);
     if(original != null && original.bitmapData != null) {
      check(original.bitmapData.width == 77 && original.bitmapData.height == 77,"original bitmap dimensions " + id);
      check(!original.bitmapData.getColorBoundsRect(0xFF000000,0,false).isEmpty(),"nonempty original icon " + id);
     }
     check(glyph.width>0 && glyph.height>0,"medical icon "+id);
     check(glyph.width<120 && glyph.height<120,"medical icon bounds "+id);
    }
   }
   finish();
  }
  private function finish():void {
   trace("MEDICALSKILLS RESULT "+(failures.length ? "fail: "+failures.join("; ") : "pass"));
   fscommand("quit");
  }
 }
}
