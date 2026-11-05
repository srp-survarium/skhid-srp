package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.*;
 import flash.system.*;
 public class MedicalSkillsHarness extends Sprite {
  // The medical tab's skills and each's embedded artwork (bitmaps/<class>.png, icons-069.tsv).
  private static const ARTWORK:Object = {301:301,302:302,303:301,311:311,312:312,313:313,321:321,322:322,323:323,
   331:331,332:332,341:341,342:342};
  private var loader:Loader = new Loader();
  private var failures:Array = [];
  private var icons:Object;
  private var frames:int = 0;
  private var domain:ApplicationDomain;
  private var edits:Object;
  private var pending:int = 2;
  private var sources:Array = [];
  private var stage_:String = "";
  public function MedicalSkillsHarness() {
   loader.contentLoaderInfo.addEventListener(Event.INIT,function(e:Event):void {ready();});
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void {e.preventDefault();failures.push("uncaught "+e.error);});
   loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
   var list:URLLoader = new URLLoader();
   list.addEventListener(Event.COMPLETE,function(e:Event):void {edits=JSON.parse(String(list.data));ready();});
   list.addEventListener(IOErrorEvent.IO_ERROR,function(e:Event):void {failures.push("no medical-skills-1.json");finish();});
   list.load(new URLRequest("medical-skills-1.json"));
  }
  private function ready():void {if(--pending==0) loaded();}
  private function check(value:Boolean,label:String):void {if(!value) failures.push(label);}
  private function loaded():void {
   try {
    domain=loader.contentLoaderInfo.applicationDomain;
    pockets();
    tree();
    var Icons:Class=domain.getDefinition("SrvSkillRendererCh") as Class;
    icons=new Icons();addChild(DisplayObject(icons));
    icons.data={id:341,name:"Medic",branch:2,level:5,maxValue:1,value:1,newValue:1,icon:14,description_1:"Healing",description_2:"they glow you shine",antiskills:[]};
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error) {failures.push("setup at "+stage_+": "+err);finish();}
  }
  private function pockets():void {
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
  }
  // English text of a key, as the edit list adds it (the stock keys the exe reads are given here).
  private function english(key:String):String {
   var stock:Object={st_percent_postfix:"%",st_current_value:"\nCurrent value:"};
   if(stock[key]!=null) return stock[key];
   for each(var row:Object in edits.add_key) if(row.key==key && row.file=="localization/english/localization") return row.text;
   return null;
  }
  // The exe's fill_skills_properties rows (0x223a70, the loop at 0x223e45): for rank i of 0..levels the
  // description, [0], the postfix and [2]; below the maximum also st_current_value, [1], postfix, [3].
  private function described(ui:Object,levels:int):Array {
   if(ui.postfix==null) return [english(ui.description)];
   var out:Array=[];
   var postfix:String=english(ui.postfix);
   for(var i:int=0;i<=levels;i++) {
    var row:Array=ui.skill_descriptions[i];
    var text:String=english(ui.description)+row[0]+postfix+row[2];
    if(i<levels) text+=english("st_current_value")+row[1]+postfix+row[3];
    out.push(text);
   }
   return out;
  }
  private function tree():void {
   var skills:Object={};var props:Array=[];var values:Array=[];
   for each(var entry:Object in edits.append_entry) {
    var v:Object=entry.value;skills[v.id]=v;
    check(v.modifiers.length==v.levels_amount,"modifiers per rank "+v.id);
    if(v.levels_amount>1) check(v.ui.skill_descriptions.length==v.levels_amount+1,"rank rows "+v.id);
    props.push({id:v.id,branch:int(v.id/100)-1,max_value:v.levels_amount,name:english(v.ui.name),
     description_1:english(v.ui.description),description_2:described(v.ui,v.levels_amount),icon:v.ui.icon,
     column:v.ui.position[0],row:v.ui.position[1],antiskills:v.preconditions.antiskills,
     branch_points:v.preconditions.branch_points});
    values.push({id:v.id,value:0});
   }
   check(skills[341]==null,"Medic left the medical tree");
   var layout:Object={301:[2,1,3,0],302:[3,1,2,0],311:[1,2,5,3],313:[3,2,2,3],312:[4,2,3,3],
    342:[1,3,2,10],303:[2,3,2,10],321:[4,3,3,10],332:[2,4,5,15],323:[3,4,4,15],331:[2,5,1,20],322:[3,5,1,20]};
   var total:int=0;
   for(var id:String in layout) {
    var want:Array=layout[id];var have:Object=skills[id];
    check(have!=null && have.ui.position[0]==want[0] && have.ui.position[1]==want[1] && have.levels_amount==want[2]
     && have.preconditions.branch_points==want[3],"layout "+id);
    if(have) total+=have.levels_amount;
   }
   check(total==33,"33 ranks, 32 points with one exclusive unlock");
   for each(var line:String in described(skills[301].ui,3)) {
    check(line.indexOf("<green>")>=0 && /%.*s<\/green>/.test(line.split("\n")[0]),"Potent Painkiller green percent and seconds: "+line);
   }
   check(described(skills[301].ui,3)[3].indexOf("<green> +3% / +4.5 s</green>")>=0,"Potent Painkiller maximum");
   check(described(skills[301].ui,3)[1].indexOf("Current value: +1% / +1.5 s")>=0,"Potent Painkiller current value");

   var Window:Class=domain.getDefinition("SrvCharWindow") as Class;
   var Renderer:Class=domain.getDefinition("SrvSkillRendererCh") as Class;
   // The menu as the lobby drives it: properties, skills, points, then the medical tab.
   stage_="window";var win:Object=new Window();win.registerRenderer(Renderer);addChild(DisplayObject(win));
   stage_="init";win.init(new Event(Event.RENDER));
   stage_="properties";win.fillSkillsProperties(props);
   stage_="skills";win.fillSkills(values);
   stage_="points";win.setData({points_available:40});
   stage_="medical tab";win.skillsTabPage.selectedIndex=2;
   stage_="checks";var shown:Object=renderers(win,Renderer);
   check(shown[341]==null,"no Medic in the menu");
   var step:Point=new Point(shown[302].x-shown[301].x,shown[311].y-shown[301].y);
   check(step.x>0 && step.y>0,"renderer steps");
   for(id in layout) check(shown[id]!=null && shown[id].x==step.x*(layout[id][0]-1) && shown[id].y==step.y*(layout[id][1]-1),"menu position "+id);
   for(id in layout) check(pips(shown[id])==layout[id][2],"rank pips "+id+" "+pips(shown[id]));
   var Ex:Class=domain.getDefinition("scaleform.gfx.MouseEventEx") as Class;
   function click(id:int,right:Boolean=false):int {
    var r:Object=shown[id];r.validateNow();
    var e:Object=new Ex(MouseEvent.CLICK);e.buttonIdx=right?1:0;r.dispatchEvent(e);
    r.validateNow();
    return r.data.newValue;
   }
   function take(id:int,times:int):int {var v:int=0;for(var k:int=0;k<times;k++) v=click(id);return v;}
   check(take(301,2)==2,"Potent Painkiller 2");
   check(take(311,1)==0,"tier 2 refused at 2 points");
   check(take(301,2)==3,"Potent Painkiller stops at 3");
   check(take(311,1)==1,"tier 2 opens at 3");
   check(take(302,3)==2,"Fast Bandaging stops at 2");
   check(shown[301].data.description_2_string==shown[301].data.description_2[3],"Potent Painkiller tooltip at maximum");
   check(String(shown[301].data.description_2_string).indexOf("+3% / +4.5 s")>=0,"Potent Painkiller tooltip text");
   check(take(303,1)==0,"tier 3 refused at 6 points");
   check(take(311,4)==5,"Metabolism 5: 10 points");
   check(take(303,3)==2,"tier 3 at 10; Numb Legs stops at 2");
   check(take(321,1)==1 && take(342,1)==1,"tier 3 at 10");
   click(321,true);click(342,true);
   check(shown[321].data.newValue==0 && shown[342].data.newValue==0,"tier 3 points taken back");
   check(take(313,3)==2,"Take Cover stops at 2: 14 points");
   check(take(323,1)==0,"tier 4 refused at 14 points");
   check(take(312,4)==3,"Medical Training stops at 3");
   click(312,true);click(312,true);
   check(shown[312].data.newValue==1,"Medical Training back to 1: 15 points");
   check(take(323,5)==4,"tier 4 at 15; Unflinching stops at 4");
   check(take(331,1)==0 && take(322,1)==0,"tier 5 refused at 19 points");
   check(take(332,1)==1,"20 points");
   check(take(331,2)==1,"Supply Pocket at 20, one rank");
   check(take(322,1)==0,"Additional Medicine refused beside Supply Pocket");
   click(331,true);
   check(shown[331].data.newValue==0,"Supply Pocket taken back");
   check(take(322,2)==1,"Additional Medicine at 20, one rank");
   check(take(331,1)==0,"Supply Pocket refused beside Additional Medicine");
   // One renderer shows skills of 5, 2, 3, 1 and 4 ranks in turn (tabs reuse renderers).
   var reused:Object=new Renderer();addChild(DisplayObject(reused));
   for each(var ranks:int in [5,2,2,3,1,4,5]) {
    reused.data={id:300+ranks,name:"r",branch:2,level:1,maxValue:ranks,value:0,newValue:1,icon:1,description_1:"d",description_2:["a","b","c","d","e","f"],antiskills:null};
    reused.validateNow();
    check(pips(reused)==ranks,"reused renderer pips "+ranks+": "+pips(reused));
   }
  }
  private function renderers(win:Object,Renderer:Class):Object {
   var out:Object={};
   for(var i:int=0;i<win.numChildren;i++) {
    var c:Object=win.getChildAt(i);
    if(!(c is DisplayObjectContainer)) continue;
    for(var j:int=0;j<c.numChildren;j++) {
     var r:Object=c.getChildAt(j);
     if(r is Renderer && r.data) out[r.data.id]=r;
    }
   }
   return out;
  }
  private function pips(r:Object):int {
   if(r==null) return -1;
   r.validateNow();
   var Pip:Class=domain.getDefinition("SkillProgressIcon") as Class;
   var n:int=0;var s:Object=r.skillProgressIndicator;
   for(var i:int=0;i<s.numChildren;i++) if(s.getChildAt(i) is Pip) n++;
   return n;
  }
  private function tick(e:Event):void {
   if(++frames<3) return;
   removeEventListener(Event.ENTER_FRAME,tick);
   var glyph:Object=icons.iconContainer.getChildByName("medicalSkillIcon");
   check(glyph!=null && glyph.visible,"medical renderer uses original artwork");
   if(glyph!=null) {
    for(var key:String in ARTWORK) {
     var id:int=int(key);
     glyph.show(id);
     var original:Bitmap = glyph.getChildAt(0) as Bitmap;
     check(original != null && original.bitmapData != null,"medical bitmap " + id);
     if(original != null && original.bitmapData != null) {
      check(original.bitmapData.width == 77 && original.bitmapData.height == 77,"original bitmap dimensions " + id);
      check(!original.bitmapData.getColorBoundsRect(0xFF000000,0,false).isEmpty(),"nonempty original icon " + id);
      sources.push([id,original.bitmapData]);
     }
     check(glyph.width>0 && glyph.height>0,"medical icon "+id);
     check(glyph.width<120 && glyph.height<120,"medical icon bounds "+id);
    }
   }
   compareNext();
  }
  // Embedded pixels against the committed PNG (bitmaps/MedicalSkill069<artwork>.png), within rounding.
  private function compareNext():void {
   if(!sources.length) {finish();return;}
   var item:Array=sources.shift();
   var png:Loader=new Loader();
   png.contentLoaderInfo.addEventListener(Event.COMPLETE,function(e:Event):void {
    var want:BitmapData=Bitmap(png.content).bitmapData;var have:BitmapData=item[1];
    var worst:int=0;
    if(want.width!=have.width || want.height!=have.height) worst=256;
    else for(var y:int=0;y<want.height;y++) for(var x:int=0;x<want.width;x++) {
     var a:uint=want.getPixel32(x,y),b:uint=have.getPixel32(x,y);
     for(var s:int=0;s<32;s+=8) worst=Math.max(worst,Math.abs(int((a>>>s)&255)-int((b>>>s)&255)));
    }
    check(worst<=2,"embedded pixels match source "+item[0]+" (max difference "+worst+")");
    compareNext();
   });
   png.contentLoaderInfo.addEventListener(IOErrorEvent.IO_ERROR,function(e:Event):void {failures.push("no source png "+item[0]);compareNext();});
   png.load(new URLRequest("bitmaps/MedicalSkill069"+ARTWORK[item[0]]+".png"));
  }
  private function finish():void {
   trace("MEDICALSKILLS RESULT "+(failures.length ? "fail: "+failures.join("; ") : "pass"));
   fscommand("quit");
  }
 }
}
