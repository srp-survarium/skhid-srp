package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.*;
 import flash.system.*;
 import flash.text.*;
 import flash.utils.*;
 // The Character panel's face picker (docs/notes/face-choice.md), made by the lobby movie's own
 // panel (SrvLocalPlayerProfile) and fed srp's answers as PlayerRanking.fillPlayers gives them.
 // Traces FACE lines, `FACE shot <name>` when a capture is due, and `FACE RESULT pass|fail: ...`.
 public class FacePickerHarness extends Sprite {
  private var loader:Loader=new Loader();
  private var domain:ApplicationDomain;
  private var Picker:Object, Loc:Object;
  private var profile:Object, picker:Object;
  private var languages:Array, language:int=0;
  private var step:int=0, wait:int=0;
  public function FacePickerHarness() {
   stage.align=StageAlign.TOP_LEFT;stage.scaleMode=StageScaleMode.NO_SCALE;
   graphics.beginFill(0x1B201E);graphics.drawRect(0,0,1000,940);graphics.endFill();
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void{e.preventDefault();});
   loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function require(ok:Boolean,why:String):void {if(!ok)throw new Error(why);}
  private function texts(language:String):Object {var t:Object={};for(var k:String in HarnessText.DATA[language])t[k]=HarnessText.DATA[language][k];for(k in HarnessText.STOCK[language])t[k]=HarnessText.STOCK[language][k];return t;}
  private function answer(row:String):Boolean {return Picker.take([{place:2097000000,nickname:row,rating:0,change:0}]);}
  private function number():String {return picker.getChildAt(3).text;}
  private function label():TextField {return picker.getChildAt(2) as TextField;}
  private function centre(side:int):Point {var r:Rectangle=picker.getBounds(stage);return new Point(r.x+r.width*(side<0?.25:.75),r.y+r.height/2);}
  private function press(side:int):void {var p:Point=centre(side);picker.press(p.x,p.y);}
  private function expect(want:String,why:String):void {require(number()==want,why+": shows "+number()+", want "+want);}
  private function loaded(e:Event):void {
   try {
    domain=loader.contentLoaderInfo.applicationDomain;
    var Scene:Object=domain.getDefinition("MainScene");Scene.stage=this;
    Loc=domain.getDefinition("survarium.data.LocalizationData");Loc.setData(texts("english"));
    Picker=domain.getDefinition("survarium.face.FacePicker");
    var Profile:Class=domain.getDefinition("SrvLocalPlayerProfile") as Class;
    profile=new Profile();profile.x=10;profile.y=10;
    profile.setAmmoShare(domain.getDefinition("survarium.ammo.AmmoShareLite").getInstance());
    addChild(profile as DisplayObject);
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error){finish("setup: "+err+" "+err.getStackTrace());}
  }
  private function tick(e:Event):void {
   if(wait>0){wait--;return;}
   try {
    switch(step++) {
     case 0:
      picker=Picker.me;
      require(picker!=null && picker.parent==profile,"the panel makes the picker");
      require(!picker.visible,"hidden until srp says");
      require(!Picker.take([{place:2090000000,nickname:"F1;0;1;3"}]),"rows of another range pass by");
      require(answer("F2;0;0;3") && !picker.visible,"another version of the row is taken and read as nothing");
      var head:Rectangle=profile.profileItems.slotHead.getBounds(stage), mine:Rectangle=picker.getBounds(stage);
      var back:Rectangle=profile.profileItems.slotBack.getBounds(stage), ammo:Object=profile.ammoContainer;
      require(mine.right<=head.left && mine.top==head.top && mine.bottom<=back.top+1,"beside the helmet's slot, above the back's: "+mine+" "+head);
      require(profile.getChildIndex(picker)<profile.getChildIndex(ammo),"under the ammo window");
      require(answer("F1;0;0;1") && !picker.visible,"one face: hidden");
      require(answer("F1;0;2;11") && picker.visible,"eleven faces: shown");
      expect("3/11","srp's face");
      trace("FACE shot english");wait=90;break;
     case 1:
      press(1);expect("4/11","a pick shows at once");
      press(1);press(1);expect("6/11","picks go on while the first is asked");
      answer("F1;1;3;11");expect("6/11","the first choice's answer keeps the last pick");
      answer("F1;2;3;11");expect("4/11","the next one refused: srp's face back");
      press(-1);press(-1);press(-1);press(-1);expect("11/11","left from the first wraps round");
      answer("F1;1;10;11");expect("11/11","chosen");
      press(-1);answer("F1;0;0;4");expect("1/4","a pick past fewer faces: srp's");
      answer("F1;0;9;4");expect("1/4","a face past the count is no answer");
      answer("F1;0;3;4");expect("4/4","the state settles a face with no pick");
      // Every language's word fits over the number, inside the picker.
      for each(var lang:String in HarnessText.LANGUAGES) {
       Loc.setData(texts(lang));answer("F1;0;3;4");
       var t:TextField=label();
       require(t.text==HarnessText.DATA[lang]["st_srp_face"],lang+": the word is the dictionary's: "+t.text);
       require(t.x>=0 && t.x+t.width<=picker.width+0.5,lang+": "+t.text+" fits ("+t.width+" in "+picker.width+")");
      }
      Loc.setData(texts("hungarian"));answer("F1;0;9;11");
      // The chevrons inside the picker, clear of the widest number.
      var arrows:Rectangle=picker.getChildAt(1).getBounds(picker as DisplayObject), digits:Rectangle=picker.getChildAt(3).getBounds(picker as DisplayObject);
      require(arrows.left>=0 && arrows.right<=picker.width+0.5 && arrows.left<digits.left-4 && arrows.right>digits.right+4,"chevrons "+arrows+" clear of "+number()+" "+digits);
      trace("FACE shot hungarian");wait=90;break;
     case 2:
      finish("pass");break;
    }
   } catch(err:Error){finish("fail: "+err.message);}
  }
  private function finish(what:String):void {removeEventListener(Event.ENTER_FRAME,tick);trace("FACE RESULT "+what);}
 }
}
