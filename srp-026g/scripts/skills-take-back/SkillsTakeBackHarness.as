package {
 import flash.display.*;
 import flash.events.*;
 import flash.net.*;
 import flash.system.*;
 // The skills window's right click (the owner, 2026-11-27): a point taken back, confirmed or not,
 // pending until Confirm; refused, never cascaded, when a row would keep points its branch no
 // longer reaches, with those skills' red locks while the pointer stays on the skill. The lobby
 // movie's own SrvCharWindow and SrvSkillRendererCh on a made-up branch of the stock tree's shape:
 //   101 102     row 1, open at 0 (5 ranks each)
 //   103 111 113 row 2, open at 5 in row 1 (111 and 113 exclude each other)
 //   105         row 3, open at 10 in rows 1 and 2
 // The exe's side is check.sh's HarnessExternal (apply_skills recorded with its set).
 public class SkillsTakeBackHarness extends Sprite {
  private var loader:Loader = new Loader();
  private var failures:Array = [];
  private var domain:ApplicationDomain;
  private var stage_:String = "";
  public function SkillsTakeBackHarness() {
   loader.contentLoaderInfo.addEventListener(Event.INIT,function(e:Event):void {loaded();});
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void {e.preventDefault();failures.push("uncaught "+e.error);});
   loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function check(value:Boolean,label:String):void {if(!value) failures.push(stage_+": "+label);}
  private function loaded():void {
   try {
    domain=loader.contentLoaderInfo.applicationDomain;
    run();
   } catch(err:Error) {failures.push("at "+stage_+": "+err+" "+err.getStackTrace());}
   finish();
  }
  private function prop(id:int,branch:int,column:int,row:int,ranks:int,needs:int,anti:Array):Object {
   var texts:Array=[];
   for(var k:int=0;k<=ranks;k++) texts.push("rank "+k);
   return {id:id,branch:branch,max_value:ranks,name:"skill "+id,description_1:"d",description_2:texts,icon:1,
    column:column,row:row,antiskills:anti,branch_points:needs};
  }
  private function run():void {
   var props:Array=[prop(101,0,1,1,5,0,[]),prop(102,0,2,1,5,0,[]),prop(103,0,1,2,3,5,[]),
    prop(111,0,2,2,1,5,[113]),prop(113,0,3,2,1,5,[111]),prop(105,0,1,3,3,10,[]),prop(201,1,1,1,5,0,[])];
   var External:Object=domain.getDefinition("HarnessExternal");
   var Window:Class=domain.getDefinition("SrvCharWindow") as Class;
   var Renderer:Class=domain.getDefinition("SrvSkillRendererCh") as Class;
   var Ex:Class=domain.getDefinition("scaleform.gfx.MouseEventEx") as Class;
   var level:int=20;
   // The server's skills: fill_skills, then fill_char_info's points (level - the points held).
   function server(held:Object):void {
    var values:Array=[];var spent:int=0;
    for each(var p:Object in props) {values.push({id:p.id,value:int(held[p.id]||0)});spent+=int(held[p.id]||0);}
    win.fillSkills(values);
    win.setData({points_available:level-spent});
    win.fillSkillsProgress([{points:spent},{points:0},{points:0},{points:0},{points:0}]);
   }
   stage_="window";var win:Object=new Window();win.registerRenderer(Renderer);addChild(DisplayObject(win));
   win.init(new Event(Event.RENDER));
   win.fillSkillsProperties(props);
   server({101:5,102:1,103:3,111:1,105:1});
   win.skillsTabPage.selectedIndex=1;win.skillsTabPage.selectedIndex=0;
   var shown:Object={};
   for(var i:int=0;i<win.numChildren;i++) {
    var c:Object=win.getChildAt(i);
    if(!(c is DisplayObjectContainer)) continue;
    for(var j:int=0;j<c.numChildren;j++) {var r:Object=c.getChildAt(j);if(r is Renderer && r.data) shown[r.data.id]=r;}
   }
   function rank(id:int):int {shown[id].validateNow();return shown[id].data.newValue;}
   function pips(id:int):int {shown[id].validateNow();return shown[id].skillProgressIndicator.value;}
   function click(id:int,right:Boolean=false):void {
    var r:Object=shown[id];r.validateNow();
    r.dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
    var e:Object=new Ex(MouseEvent.CLICK);e.buttonIdx=right?1:0;r.dispatchEvent(e);
    r.validateNow();
   }
   function leave(id:int):void {shown[id].dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));}
   function red(id:int):Boolean {return shown[id].lockSpriteRed.visible;}
   function points():int {return int(win.point_available_value.text);}
   function ranks(want:Object,label:String):void {
    for(var id:String in shown) check(rank(int(id))==int(want[id]||0) && pips(int(id))==int(want[id]||0),label+": "+id+" has "+rank(int(id))+", want "+int(want[id]||0));
   }
   function sent():Array {
    var out:Array=[];
    for each(var call:Object in External.calls) if(call.name=="apply_skills") {
     var set:Array=[];for each(var s:Object in call.args[0]) set.push(s.id+":"+s.value);
     set.sort();out.push(set.join(","));
    }
    return out;
   }

   stage_="confirmed ranks";
   check(shown[101]!=null && shown[105]!=null && shown[201]==null,"the first tab's skills shown");
   ranks({101:5,102:1,103:3,111:1,105:1},"drawn");
   check(points()==9,"points "+points());
   check(!win.btn_set.enabled && !win.btn_cancel.enabled,"nothing to confirm");
   check(red(113) && !red(111),"113 locked by 111");

   stage_="refused, confirmed";
   // 101 holds row 3: rows 1 and 2 would have 9 of its 10.
   click(101,true);
   ranks({101:5,102:1,103:3,111:1,105:1},"unchanged");
   check(points()==9,"points "+points());
   check(!win.btn_set.enabled,"nothing to confirm");
   check(red(105),"105's red lock while on 101");
   check(!red(101) && !red(102) && !red(103),"no other red lock");
   leave(101);
   check(!red(105),"105's red lock gone with the pointer");
   check(red(113),"113 still locked by 111");

   stage_="taken back, confirmed";
   click(105,true);
   ranks({101:5,102:1,103:3,111:1},"105 taken back");
   check(points()==10,"points "+points());
   check(win.btn_set.enabled && win.btn_cancel.enabled,"Confirm and Cancel");
   check(sent().length==0,"nothing sent before Confirm");

   stage_="cancel";
   win.btn_cancel.dispatchEvent(new (domain.getDefinition("scaleform.clik.events.ButtonEvent") as Class)("buttonClick"));
   ranks({101:5,102:1,103:3,111:1,105:1},"restored");
   check(points()==9,"points "+points());
   check(!win.btn_set.enabled,"nothing to confirm");

   stage_="confirm";
   click(105,true);
   click(102,true);
   ranks({101:5,103:3,111:1},"105 and 102 taken back");
   check(points()==11,"points "+points());
   win.btn_set.dispatchEvent(new (domain.getDefinition("scaleform.clik.events.ButtonEvent") as Class)("buttonClick"));
   check(sent().join(" / ")=="101:5,103:3,111:1","apply_skills sent "+sent().join(" / "));
   for each(var call:Object in External.calls) check(call.name!="reroll_ok_clicked","no reset sent");
   server({101:5,103:3,111:1});
   ranks({101:5,103:3,111:1},"the server's");
   check(points()==11,"points "+points());
   check(!win.btn_set.enabled,"nothing to confirm");

   stage_="refused, pending";
   click(102);
   click(105);
   ranks({101:5,102:1,103:3,111:1,105:1},"two pending");
   click(102,true);
   ranks({101:5,102:1,103:3,111:1,105:1},"no cascade");
   check(red(105),"105's red lock while on 102");
   leave(102);
   check(!red(105),"105's red lock gone");
   click(105,true);
   click(102,true);
   ranks({101:5,103:3,111:1},"both pending taken back");
   check(points()==11,"points "+points());
   check(!win.btn_set.enabled,"back to the server's");

   stage_="antiskill";
   click(111,true);
   ranks({101:5,103:3},"111 taken back");
   check(!red(113),"113 open");
   click(113);
   ranks({101:5,103:3,113:1},"113 taken");
   check(red(111),"111 locked by 113");
   win.btn_set.dispatchEvent(new (domain.getDefinition("scaleform.clik.events.ButtonEvent") as Class)("buttonClick"));
   check(sent()[1]=="101:5,103:3,113:1","apply_skills sent "+sent().join(" / "));
  }
  private function finish():void {
   trace("SKILLSTAKEBACK RESULT "+(failures.length ? "fail: "+failures.join("; ") : "pass"));
   fscommand("quit");
  }
 }
}
