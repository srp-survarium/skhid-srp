package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.*;
 import flash.system.*;
 import flash.text.*;
 import flash.utils.*;
 // The player panel's Face button and its chooser (docs/notes/face-choice.md), made by the lobby movie's
 // own panel (top_fla.topPanelLeftContainer_56) and fed srp's answers as PlayerRanking.fillPlayers gives
 // them. Traces FACE lines, `FACE shot <name>` when a capture is due, and `FACE RESULT pass|fail: ...`.
 public class FacePickerHarness extends Sprite {
  private static const TILE:Number=88;
  private var loader:Loader=new Loader();
  private var domain:ApplicationDomain;
  private var Choice:Object, Loc:Object, Transient:Object, ButtonEvent:Class, Chooser:Object, Panel:Class;
  private var panel:Object, button:Object, choice:Object;
  private var backdrop:Sprite=new Sprite();
  private var step:int=0, wait:int=0;
  public function FacePickerHarness() {
   stage.align=StageAlign.TOP_LEFT;stage.scaleMode=StageScaleMode.NO_SCALE;
   backdrop.graphics.beginFill(0x1B201E);backdrop.graphics.drawRect(0,0,760,400);backdrop.graphics.endFill();
   addChild(backdrop);
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void{e.preventDefault();});
   loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function require(ok:Boolean,why:String):void {if(!ok)throw new Error(why);}
  private function texts(language:String):Object {var t:Object={};for(var k:String in HarnessText.DATA[language])t[k]=HarnessText.DATA[language][k];for(k in HarnessText.STOCK[language])t[k]=HarnessText.STOCK[language][k];return t;}
  private function answer(row:String):Boolean {return Choice.take([{place:2097000000,nickname:row,rating:0,change:0}]);}
  private function chooser():Object {return button.chooser;}
  private function tile(face:int):DisplayObjectContainer {return chooser().getChildByName("face"+face) as DisplayObjectContainer;}
  private function shown():int {return choice.shown;}
  private function click():void {button.dispatchEvent(new ButtonEvent("buttonClick"));}
  // A press on a face's tile, as the game gives it (to the tile, bubbling to the chooser).
  private function press(face:int):void {var t:DisplayObject=tile(face);require(t!=null,"no tile of face "+face);t.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN,true,false,TILE/2,TILE/2));}
  private function expect(want:int,why:String):void {require(shown()==want,why+": shows face "+shown()+", want "+want);}
  // Framed gold: the tile's top left pixel is the gold.
  private function gold(face:int):Boolean {var b:BitmapData=new BitmapData(TILE,TILE,true,0);b.draw(tile(face));var c:uint=b.getPixel(1,1);b.dispose();return c==0xE0A64E;}
  private function numbered(face:int):Boolean {var f:TextField=tile(face).getChildAt(0) as TextField;return f!=null && f.text==String(face+1);}
  private function loaded(e:Event):void {
   try {
    domain=loader.contentLoaderInfo.applicationDomain;
    var Scene:Object=domain.getDefinition("MainScene");Scene.stage=this;
    Loc=domain.getDefinition("survarium.data.LocalizationData");Loc.setData(texts("english"));
    Choice=domain.getDefinition("survarium.face.FaceChoice");
    Chooser=domain.getDefinition("survarium.face.FaceChooser");
    // The lobby's transient layer is in its root, not on this stage: none here (nothing moves).
    Transient=domain.getDefinition("survarium.controls.Transient");Transient.layer=null;
    ButtonEvent=domain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
    Panel=domain.getDefinition("top_fla.topPanelLeftContainer_56") as Class;
    require(!domain.hasDefinition("survarium.face.FacePicker"),"the Character panel's stepper is gone");
    panel=new Panel();panel.x=10;panel.y=10;
    addChild(panel as DisplayObject);
    button=panel.btn_face;choice=Choice.shared();
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error){finish("setup: "+err+" "+err.getStackTrace());}
  }
  private function tick(e:Event):void {
   if(wait>0){wait--;return;}
   try {
    switch(step++) {
     case 0:
      // The button: the lobby button's size and look, 8 px left of it, on its row.
      var lobby:Object=panel.btn_lobby;
      require(button!=null && button==panel.getChildByName("btn_face"),"the panel makes the face button");
      require(button is (domain.getDefinition("survarium.controls.PanelButton") as Class) && getQualifiedClassName(lobby)=="survarium.controls::PanelButton","the face button is a panel button, as the lobby's");
      require(button.boxWidth==lobby.boxWidth && button.boxHeight==lobby.boxHeight && button.y==lobby.y,"as big as the lobby button, on its row");
      require(Math.abs(lobby.x-(button.x+button.boxWidth)-8)<0.01,"8 px left of the lobby button: "+button.x+" "+lobby.x);
      require(button.shown==lobby.shown,"the same back piece as the lobby button: "+button.shown);
      require(!button.visible,"hidden until srp says");
      require(!Choice.take([{place:2090000000,nickname:"F1;0;1;3"}]),"rows of another range pass by");
      require(answer("F2;0;0;3") && !button.visible,"another version of the row is taken and read as nothing");
      require(answer("F1;0;0;1") && !button.visible,"one face: hidden");
      click();require(!button.isOpen,"hidden: no chooser");
      require(answer("F1;0;2;11") && button.visible,"eleven faces: shown");
      expect(2,"srp's face");
      // Every language's word, upper case, inside the button and centred, as the lobby button's.
      for each(var lang:String in HarnessText.LANGUAGES) {
       Loc.setData(texts(lang));
       var b:Object=Object(new Panel()).btn_face;var l:TextField=b.getChildByName("panelLabel") as TextField;
       var want:String=String(HarnessText.DATA[lang]["st_srp_face"]).toUpperCase();
       var dx:Number=l.x+l.width/2-b.boxWidth/2;
       trace("FACE label "+lang+": '"+l.text+"' "+int(l.textWidth)+" in "+int(b.boxWidth)+", off centre "+dx.toFixed(1));
       require(l.text==want && l.width<=b.boxWidth-8 && Math.abs(dx)<=1,lang+": the label '"+l.text+"' fits, centred");
      }
      Loc.setData(texts("english"));
      trace("FACE shot button");wait=180;break;
     case 1:
      // Open: under the button, the faces in two rows of six and five, srp's face gold.
      click();
      require(button.isOpen && Transient.isOpen(chooser()),"a click opens the chooser, a transient window");
      var cy:Number=chooser().y-(button.y+button.boxHeight);
      require(chooser().parent==button.parent && chooser().x==button.x && cy>=0 && cy<=6,"under the button, in the panel: "+chooser().x+","+chooser().y);
      require(Chooser.columnsOf(11)==6 && Chooser.columnsOf(7)==4 && Chooser.columnsOf(4)==4,"rows even");
      for(var i:int=0;i<11;i++) require(tile(i)!=null,"a tile a face: "+i);
      require(tile(11)==null,"no tile past the faces");
      require(tile(5).y==tile(0).y && tile(6).y>tile(0).y && tile(6).x==tile(0).x,"six a row");
      require(gold(2) && !gold(3),"srp's face framed gold");
      require(numbered(4),"no atlas cell: the face's number");
      trace("FACE shot chooser");wait=180;break;
     case 2:
      press(3);expect(3,"a press chooses at once");require(gold(3) && !gold(2),"the pick framed at once");
      require(button.isOpen,"open after a choice: another can be tried");
      press(4);press(5);expect(5,"picks go on while the first is asked");
      answer("F1;1;3;11");expect(5,"the first choice's answer keeps the last pick");
      answer("F1;2;3;11");expect(3,"the next one refused: srp's face back");
      require(gold(3),"the refusal framed");
      press(10);answer("F1;1;10;11");expect(10,"chosen");
      // Esc closes it; the button's click again; a press outside.
      require(Transient.escape() && !button.isOpen,"Esc closes it");
      click();require(button.isOpen,"opened again");click();require(!button.isOpen,"the button closes it");
      click();backdrop.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN,true,false,700,380));
      require(!button.isOpen,"a press outside closes it");
      click();press(9);require(button.isOpen,"a press on a tile keeps it open");
      // Fewer faces: the tiles made again; one face: the button hidden and the chooser closed.
      answer("F1;0;0;4");expect(0,"a pick past fewer faces: srp's");require(tile(3)!=null && tile(4)==null,"four tiles");
      answer("F1;0;9;4");expect(0,"a face past the count is no answer");
      answer("F1;0;0;1");require(!button.visible && !button.isOpen,"one face: hidden, the chooser closed");
      // The faces pack's pictures: the equipment atlas's cells of column 16 (a stand-in atlas
      // here: 16 columns of 8 rows, a head drawn in each cell of the last); faces past its rows
      // keep their number.
      var rows:int=8, atlas:BitmapData=new BitmapData(171*16,91*rows,true,0);
      var head:Shape=new Shape();
      for(var r:int=0;r<rows;r++) {
       var g:Graphics=head.graphics;
       var cx:Number=171*15+85, cy:Number=91*r+41;
       g.beginFill(0x6A5446+r*0x090807);g.drawEllipse(cx-22,cy-30,44,58);g.endFill();
       g.beginFill(0x2A2420);g.drawEllipse(cx-23,cy-33,46,26);g.endFill();
       g.beginFill(0x3A4248);g.drawRect(cx-30,cy+28,60,13);g.endFill();
      }
      atlas.draw(head);
      var Icons:Class=domain.getDefinition("survarium.data.IconContainer") as Class;
      new Icons("equipment",171,83,atlas,true,0,0,8,false);
      answer("F1;0;6;11");click();
      require(!numbered(0) && !numbered(7),"faces 0 and 7: their atlas cells");
      require(numbered(8),"face 8: past the atlas's rows, its number");
      var pic:DisplayObjectContainer=tile(0).getChildAt(0) as DisplayObjectContainer;var m:DisplayObject=pic.getChildAt(0).mask;
      require(m!=null && m.width==TILE && m.height==TILE && m.x==0 && m.y==0,"the cell masked to the tile");
      trace("FACE shot icons");wait=180;break;
     case 3:
      finish("pass");break;
    }
   } catch(err:Error){finish("fail: "+err.message);}
  }
  private function finish(what:String):void {removeEventListener(Event.ENTER_FRAME,tick);trace("FACE RESULT "+what);}
 }
}
