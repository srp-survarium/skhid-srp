package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.*;
 import flash.system.*;
 public class MedicalSkillsHarness extends Sprite {
  // Our trees' skills and each's embedded artwork (bitmaps/<class>.png; medical-skills and
  // artefact-skills icons-069.tsv).
  private static const ARTWORK:Object = {301:"MedicalSkill069301",302:"MedicalSkill069302",303:"MedicalSkill069301",
   311:"MedicalSkill069311",312:"MedicalSkill069312",313:"MedicalSkill069313",321:"MedicalSkill069321",
   322:"MedicalSkill069322",323:"MedicalSkill069323",331:"MedicalSkill069331",332:"MedicalSkill069332",
   333:"MedicalSkill069333",341:"MedicalSkill069341",342:"MedicalSkill069342",
   501:"ArtefactSkill069501",502:"ArtefactSkill069502",512:"ArtefactSkill069512",513:"ArtefactSkill069513",
   521:"ArtefactSkill069521",522:"ArtefactSkill069522",523:"ArtefactSkill069523",531:"ArtefactSkill069531",532:"ArtefactSkill069532",
   533:"ArtefactSkill069533",541:"ArtefactSkill069541",542:"ArtefactSkill069542"};
  private var loader:Loader = new Loader();
  private var failures:Array = [];
  private var icons:Object;
  private var frames:int = 0;
  private var domain:ApplicationDomain;
  private var edits:Object;
  private var artefactEdits:Object;
  private var catalog:Object = {};
  private var pending:int = 4;
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
   var artefacts:URLLoader = new URLLoader();
   artefacts.addEventListener(Event.COMPLETE,function(e:Event):void {artefactEdits=JSON.parse(String(artefacts.data));ready();});
   artefacts.addEventListener(IOErrorEvent.IO_ERROR,function(e:Event):void {failures.push("no artefact-skills-1.json");finish();});
   artefacts.load(new URLRequest("artefact-skills-1.json"));
   // The English catalog (client-patches/localization/english.tsv), where the artefact tree's texts are.
   var tsv:URLLoader = new URLLoader();
   tsv.addEventListener(Event.COMPLETE,function(e:Event):void {
    for each(var line:String in String(tsv.data).split("\n")) {
     var tab:int=line.indexOf("\t");
     if(tab>0) catalog[line.substr(0,tab)]=line.substr(tab+1).split("\\n").join("\n");
    }
    ready();
   });
   tsv.addEventListener(IOErrorEvent.IO_ERROR,function(e:Event):void {failures.push("no english.tsv");finish();});
   tsv.load(new URLRequest("english.tsv"));
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
   // A put fills the pocket (one-pocket-a-type.md): all it holds of the item, at most the units owned.
   var item:Object=new Data();item.dictId=67;item.category=10;item.quickSlotItemSize=size.HALF;
   item.value=10;check(Doll.pocketUnits(item)==4,"a put fills the pocket with the fourth medicine");
   item.value=2;check(Doll.pocketUnits(item)==2,"a put takes no more than is owned");
   item.quickSlotItemSize=size.WHOLE;item.value=10;check(Doll.pocketUnits(item)==1,"a put of a whole-pocket item is one");
   item.quickSlotItemSize=size.HALF;
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
    item.value=20;check(Doll.pocketUnits(item)==5,"a put never takes a sixth medicine " + capacity);
   }
  }
  // English text of a key, as the edit list adds it (the stock keys the exe reads are given here).
  private function english(key:String):String {
   var stock:Object={st_percent_postfix:"%",st_current_value:"\nCurrent value:"};
   if(stock[key]!=null) return stock[key];
   for each(var row:Object in edits.add_key) if(row.key==key && row.file=="localization/english/localization") return row.text;
   return catalog[key];
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
   // A stock tab's rows, as the stock trees ask them (0/5/10/15/20): the indicator draws its points.
   var stock:Array=[[101,5,0],[111,5,5],[121,5,10],[131,5,15],[141,1,20]];
   for(var s:int=0;s<stock.length;s++) {
    props.push({id:stock[s][0],branch:0,max_value:stock[s][1],name:"stock",description_1:"d",description_2:["a","b","c","d","e","f"],
     icon:1,column:1,row:s+1,antiskills:[],branch_points:stock[s][2]});
    values.push({id:stock[s][0],value:0});
   }
   var artefacts:Object={};
   for each(entry in artefactEdits.append_entry) {
    v=entry.value;artefacts[v.id]=v;
    check(v.modifiers.length==v.levels_amount,"modifiers per rank "+v.id);
    if(v.levels_amount>1) check(v.ui.skill_descriptions.length==v.levels_amount+1,"rank rows "+v.id);
    check(english(v.ui.name)!=null && english(v.ui.description)!=null,"artefact texts "+v.id);
    props.push({id:v.id,branch:int(v.id/100)-1,max_value:v.levels_amount,name:english(v.ui.name),
     description_1:english(v.ui.description),description_2:described(v.ui,v.levels_amount),icon:v.ui.icon,
     column:v.ui.position[0],row:v.ui.position[1],antiskills:v.preconditions.antiskills,
     branch_points:v.preconditions.branch_points});
    values.push({id:v.id,value:0});
   }
   check(skills[341]==null,"Medic left the medical tree");
   var layout:Object={342:[1,1,1,0],301:[2,1,3,0],302:[3,1,2,0],311:[1,2,5,3],313:[3,2,2,3],312:[4,2,3,3],
    303:[2,3,2,10],321:[4,3,3,10],332:[2,4,5,15],323:[3,4,4,15],333:[4,4,4,15],331:[2,5,1,20],322:[3,5,1,20]};
   var total:int=0;
   for(var id:String in layout) {
    var want:Array=layout[id];var have:Object=skills[id];
    check(have!=null && have.ui.position[0]==want[0] && have.ui.position[1]==want[1] && have.levels_amount==want[2]
     && have.preconditions.branch_points==want[3],"layout "+id);
    if(have) total+=have.levels_amount;
   }
   check(total==36,"36 ranks, 31 points with two exclusive pairs");
   for each(var line:String in described(skills[301].ui,3)) {
    check(line.indexOf("<green>")>=0 && /%.*s<\/green>/.test(line.split("\n")[0]),"Potent Painkiller green percent and seconds: "+line);
   }
   check(described(skills[301].ui,3)[3].indexOf("<green> +3% / +4.5 s</green>")>=0,"Potent Painkiller maximum");
   check(described(skills[301].ui,3)[1].indexOf("Current value: +1% / +1.5 s")>=0,"Potent Painkiller current value");
   // Brothers in Arms, one rank with a value: its text before the point and with it.
   var brothers:Array=described(skills[342].ui,1);
   check(brothers.length==2 && brothers[0].indexOf("<green> +50%</green>\nCurrent value: 0%")>=0 && brothers[1].indexOf("<green> +50%</green>")>=0
    && brothers[1].indexOf("Current value")<0,"Brothers in Arms texts: "+brothers.join(" | "));

   var Window:Class=domain.getDefinition("SrvCharWindow") as Class;
   var Renderer:Class=domain.getDefinition("SrvSkillRendererCh") as Class;
   // The menu as the lobby drives it: properties, skills, points, then the medical tab.
   stage_="window";var win:Object=new Window();win.registerRenderer(Renderer);addChild(DisplayObject(win));
   stage_="init";win.init(new Event(Event.RENDER));
   stage_="properties";win.fillSkillsProperties(props);
   stage_="skills";win.fillSkills(values);
   stage_="points";win.setData({points_available:80});
   stage_="progress";win.fillSkillsProgress([{points:0},{points:0},{points:0}]);
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
   // The progress indicator: a row's mark is POINTS_PER_LEVEL (5) further, +1 (CharWindow.setSkillProgress).
   function bar(want:Number,label:String):void {
    var have:Number=win.progressIndicator.value;
    check(Math.abs(have-want)<1e-9,"indicator "+label+": "+have+", want "+want);
   }
   check(win.progressIndicator.maximum==26,"indicator maximum "+win.progressIndicator.maximum);
   bar(1,"medical at 0");
   // Brothers in Arms is a tier-1 skill of one rank: its point counts for tier 2.
   check(take(342,2)==1,"Brothers in Arms at 0 points, stops at 1");
   check(shown[342].data.description_2_string==shown[342].data.description_2[1],"Brothers in Arms tooltip at its rank");
   check(take(301,2)==2,"Potent Painkiller 2");
   check(take(311,1)==1,"tier 2 opens at 3 with Brothers in Arms");
   click(311,true);click(342,true);
   check(shown[311].data.newValue==0 && shown[342].data.newValue==0,"Metabolism and Brothers in Arms taken back");
   bar(1+5*2/3,"medical at 2, below tier 2's mark");
   check(take(311,1)==0,"tier 2 refused at 2 points");
   check(take(301,2)==3,"Potent Painkiller stops at 3");
   bar(6,"medical tier 2 mark at 3");
   check(take(311,1)==1,"tier 2 opens at 3");
   check(take(302,3)==2,"Fast Bandaging stops at 2");
   check(shown[301].data.description_2_string==shown[301].data.description_2[3],"Potent Painkiller tooltip at maximum");
   check(String(shown[301].data.description_2_string).indexOf("+3% / +4.5 s")>=0,"Potent Painkiller tooltip text");
   check(take(303,1)==0,"tier 3 refused at 6 points");
   check(take(311,4)==5,"Metabolism 5: 10 points");
   bar(11,"medical tier 3 mark at 10");
   check(take(303,3)==2,"tier 3 at 10; Numb Legs stops at 2");
   check(take(321,1)==1,"tier 3 at 10");
   click(321,true);
   check(shown[321].data.newValue==0,"tier 3 point taken back");
   check(take(313,3)==2,"Take Cover stops at 2: 14 points");
   bar(15,"medical at 14");
   check(take(323,1)==0,"tier 4 refused at 14 points");
   check(take(312,4)==3,"Medical Training stops at 3");
   click(312,true);click(312,true);
   check(shown[312].data.newValue==1,"Medical Training back to 1: 15 points");
   bar(16,"medical tier 4 mark at 15");
   check(take(323,5)==4,"tier 4 at 15; Unflinching stops at 4");
   check(take(333,1)==0,"Concentration refused beside Unflinching");
   for(var back:int=0;back<4;back++) click(323,true);
   check(shown[323].data.newValue==0,"Unflinching taken back");
   check(take(333,5)==4,"Concentration at 15, stops at 4");
   check(take(323,1)==0,"Unflinching refused beside Concentration");
   check(take(331,1)==0 && take(322,1)==0,"tier 5 refused at 19 points");
   check(take(332,1)==1,"20 points");
   bar(21,"medical tier 5 mark at 20");
   check(take(331,2)==1,"Supply Pocket at 20, one rank");
   bar(26,"medical full with the last tier taken");
   check(take(322,1)==0,"Additional Medicine refused beside Supply Pocket");
   click(331,true);
   check(shown[331].data.newValue==0,"Supply Pocket taken back");
   check(take(322,2)==1,"Additional Medicine at 20, one rank");
   check(take(331,1)==0,"Supply Pocket refused beside Additional Medicine");
   // The artefact tab (artefact-skills.md): 0/5/10/15/20 (the stock rows), 25 ranks in all; one
   // either/or pair, Resonance or Cascade in row V (Keeper and Forest Protection go together in
   // row III): a full build is 24; without Forest Protection rows I-IV hold 19, one short of row V.
   stage_="artefact tab";win.skillsTabPage.selectedIndex=4;shown=renderers(win,Renderer);
   var tree5:Object={501:[2,1,3,0],502:[3,1,5,0],512:[3,2,5,5],513:[4,2,1,5],521:[2,3,1,10],
    522:[3,3,1,10],523:[1,3,4,10],531:[1,4,1,15],532:[2,4,1,15],533:[3,4,1,15],541:[2,5,1,20],542:[3,5,1,20]};
   var ranks5:int=0;
   for(id in tree5) {
    want=tree5[id];have=artefacts[id];
    check(have!=null && have.ui.position[0]==want[0] && have.ui.position[1]==want[1] && have.levels_amount==want[2]
     && have.preconditions.branch_points==want[3],"artefact layout "+id);
    if(have) ranks5+=have.levels_amount;
    check(shown[id]!=null && shown[id].x==step.x*(want[0]-1) && shown[id].y==step.y*(want[1]-1),"artefact menu position "+id);
    check(pips(shown[id])==want[2],"artefact rank pips "+id);
   }
   check(ranks5==25,"25 artefact ranks");
   check(artefacts[541].preconditions.antiskills[0]==542 && artefacts[542].preconditions.antiskills[0]==541,
    "Resonance and Cascade exclude each other");
   check(artefacts[521].preconditions.antiskills==null && artefacts[523].preconditions.antiskills==null,
    "Keeper and Forest Protection go together");
   check(shown[301]==null && shown[101]==null,"only the artefact tree on its tab");
   check(described(artefacts[501].ui,3)[3].indexOf("<green> -24%</green>")>=0,"Quick Hands maximum");
   check(described(artefacts[502].ui,5)[2].indexOf("Current value: +10 points")>=0,"Resistance current value");
   check(described(artefacts[502].ui,5)[5].indexOf("<green> +25 points</green>")>=0,"Resistance maximum");
   check(described(artefacts[521].ui,1).length==1,"Keeper: the description alone");
   check(described(artefacts[513].ui,1).length==1,"Artifact Sight: the description alone");
   // Every rank of rows I-IV but Forest Protection's is 19: row V (20) needs one of its ranks.
   bar(1,"artefact at 0");
   check(take(512,1)==0,"artefact tier 2 refused at 0");
   check(take(501,6)==3,"Quick Hands stops at 3");
   bar(4,"artefact at 3");
   check(take(512,1)==0 && take(513,1)==0,"artefact tier 2 refused at 3");
   check(take(502,2)==2,"5 points");
   bar(6,"artefact tier 2 mark at 5");
   check(take(513,2)==1,"Artifact Sight at 5, one rank");
   check(take(521,1)==0 && take(523,1)==0,"artefact tier 3 refused at 6");
   check(take(512,4)==4,"10 points");
   bar(11,"artefact tier 3 mark at 10");
   check(take(521,2)==1,"Keeper at 10, one rank");
   check(take(522,2)==1,"Artifact Container at 10, one rank: 12 points");
   check(take(531,1)==0,"artefact tier 4 refused at 12");
   check(take(502,3)==5,"15 points: Resistance stops at 5");
   bar(16,"artefact tier 4 mark at 15");
   check(take(531,1)==1 && take(532,1)==1 && take(533,2)==1,"tier 4 at 15, one rank each");
   check(take(541,1)==0 && take(542,1)==0,"artefact tier 5 refused at 18");
   check(take(512,2)==5,"Knowledge of Artifacts stops at 5: 19");
   check(take(541,1)==0,"artefact tier 5 refused at 19: rows I-IV without Forest Protection hold 19");
   check(take(523,1)==1,"Forest Protection beside Keeper: 20");
   bar(21,"artefact tier 5 mark at 20");
   check(take(541,2)==1,"Resonance at 20, one rank");
   bar(26,"artefact full with the last tier taken");
   check(take(542,1)==0,"Cascade refused beside Resonance");
   click(541,true);
   check(shown[541].data.newValue==0,"Resonance taken back");
   check(take(542,2)==1,"Cascade at 20, one rank: 21");
   check(take(541,1)==0,"Resonance refused beside Cascade");
   // Forest Protection beside Keeper: a full build is 24.
   check(take(523,5)==4,"Forest Protection beside Keeper, stops at 4: a full build is 24");
   check(shown[521].data.newValue==1,"Keeper kept beside Forest Protection");
   bar(26,"artefact full with Keeper and Forest Protection");
   // The tab bar's icons (SkillBranchTabCh, its icon a MovieClip whose frame is the tab, set by
   // updateText): a selected tab's keyframes place another icon symbol than the others, so each tab's
   // glyph is checked in every state against the one it shows up (placements.tsv puts the fifth right).
   stage_="tab icons";
   var tabBar:Object=win.skillsTabPage;
   function tabGlyph(tab:Object):BitmapData {
    var drawn:DisplayObject=tab.icon.getChildAt(tab.icon.numChildren-1);
    var box:Rectangle=drawn.getBounds(drawn);
    var into:BitmapData=new BitmapData(96,96,true,0);
    into.draw(drawn,new Matrix(1,0,0,1,48-box.x-box.width/2,48-box.y-box.height/2));
    return into;
   }
   tabBar.validateNow();
   // Each state as the game reaches it: selected, enabled and the mouse (CLIK's setState, drawn).
   var tabStates:Array=[["up",false,true,MouseEvent.ROLL_OUT],["over",false,true,MouseEvent.ROLL_OVER],
    ["down",false,true,MouseEvent.MOUSE_DOWN],["disabled",false,false,null],
    ["selected_up",true,true,MouseEvent.ROLL_OUT],["selected_over",true,true,MouseEvent.ROLL_OVER],
    ["selected_down",true,true,MouseEvent.MOUSE_DOWN],["selected_disabled",true,false,null]];
   tabBar.validateNow();
   for(var tabAt:int=0;tabAt<5;tabAt++) {
    var tabButton:Object=tabBar.getButtonAt(tabAt);
    check(tabButton!=null,"tab "+(tabAt+1)+" drawn");
    if(tabButton==null) continue;
    var tabUp:BitmapData=null;
    for each(var tabState:Array in tabStates) {
     tabButton.enabled=true;tabButton.dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
     tabButton.selected=tabState[1];tabButton.enabled=tabState[2];
     if(tabState[3]) tabButton.dispatchEvent(new MouseEvent(tabState[3]));
     tabButton.validateNow();
     if(tabButton.currentLabel!=tabState[0]) {
      // over: a harness's ROLL_OVER does not reach it (CLIK's focus rules); its frame then, the icon
      // set as updateText sets it
      tabButton.gotoAndStop(tabState[0]);
      if(tabButton.icon) tabButton.icon.gotoAndStop(tabButton.data.icon);
     }
     check(tabButton.currentLabel==tabState[0],"tab "+(tabAt+1)+" "+tabState[0]+": at "+tabButton.currentLabel);
     check(tabButton.icon!=null && tabButton.icon.currentFrame==tabAt+1,
      "tab "+(tabAt+1)+" "+tabState[0]+": icon frame "+(tabButton.icon?tabButton.icon.currentFrame:-1));
     var tabNow:BitmapData=tabGlyph(tabButton);
     if(tabUp==null) tabUp=tabNow;
     else check(tabUp.compare(tabNow)===0,"tab "+(tabAt+1)+" "+tabState[0]+": the glyph it shows up");
    }
    tabButton.enabled=true;tabButton.dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));tabButton.selected=false;
   }
   tabBar.selectedIndex=4;tabBar.selectedIndex=0;
   // The stock tab: the indicator at every point as the stock window drew it, min(points + 1, 21).
   stage_="stock tab";win.skillsTabPage.selectedIndex=0;shown=renderers(win,Renderer);
   check(shown[101]!=null && shown[141]!=null && shown[301]==null,"stock tab shown");
   bar(1,"stock at 0");
   for(var p:int=1;p<=20;p++) {
    check(click(stock[int((p-1)/5)][0])==(p-1)%5+1,"stock click "+p);
    bar(Math.min(p+1,21),"stock at "+p);
   }
   check(click(141)==1,"stock last tier");
   bar(26,"stock full with the last tier taken");
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
   var glyph:Object=icons.iconContainer.getChildByName("skillIcon069");
   check(glyph!=null && glyph.show(501) && !glyph.show(101),"artwork only for our trees");
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
   png.load(new URLRequest("bitmaps/"+ARTWORK[item[0]]+".png"));
  }
  private function finish():void {
   trace("MEDICALSKILLS RESULT "+(failures.length ? "fail: "+failures.join("; ") : "pass"));
   fscommand("quit");
  }
 }
}
