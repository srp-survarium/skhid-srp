package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.*;
 import flash.system.*;
 import flash.text.*;
 import flash.utils.*;
 // The player panel's Face button and the faces' window (docs/notes/face-choice.md), made by the lobby
 // movie's own panel (top_fla.topPanelLeftContainer_56) and its own window class (survarium.face.FaceWindow),
 // told srp's account as the exe tells it (LobbyMenu.srp_faces: FaceChoice.told) and fed srp's answers as
 // PlayerRanking.fillPlayers gives them. Traces FACE lines, `FACE shot <name>` when a capture is due, and
 // `FACE RESULT pass|fail: ...`.
 public class FacePickerHarness extends Sprite {
  private static const W:int=1640, H:int=540, CARD_W:int=140, CARD_H:int=180;
  private var loader:Loader=new Loader();
  private var domain:ApplicationDomain;
  private var Choice:Object, Loc:Object, Modals:Object, Window:Class, LobbyWindow:Class, Panel:Class;
  private var panel:Object, button:Object, choice:Object, window:Object;
  private var host:Sprite=new Sprite();
  private var step:int=0, wait:int=0;
  public function FacePickerHarness() {
   stage.align=StageAlign.TOP_LEFT;stage.scaleMode=StageScaleMode.NO_SCALE;
   graphics.beginFill(0x1B201E);graphics.drawRect(0,0,W,H);graphics.endFill();
   addChild(host);
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void{e.preventDefault();});
   loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function require(ok:Boolean,why:String):void {if(!ok)throw new Error(why);}
  private function texts(language:String):Object {var t:Object={};for(var k:String in HarnessText.DATA[language])t[k]=HarnessText.DATA[language][k];for(k in HarnessText.STOCK[language])t[k]=HarnessText.STOCK[language][k];return t;}
  private function answer(row:String):Boolean {return Choice.take([{place:2097000000,nickname:row,rating:0,change:0}]);}
  private function card(face:int):DisplayObjectContainer {return window.getChildByName(String(face)) as DisplayObjectContainer;}
  private function shown():int {return choice.shown;}
  private function open():void {if(!window.isOpen)window.toggle(host);require(window.isOpen,"the window opens");}
  // A press on a card as the game gives it: to the card (its click), or to the window over the card (routed).
  private function click(face:int):void {var c:DisplayObject=card(face);require(c!=null,"no card of face "+face);c.dispatchEvent(new MouseEvent(MouseEvent.CLICK,true,false,CARD_W/2,CARD_H/2));}
  private function routed(face:int):void {var c:DisplayObject=card(face);var p:Point=window.globalToLocal(c.localToGlobal(new Point(CARD_W/2,CARD_H/3)));window.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN,true,false,p.x,p.y));}
  private function expect(want:int,why:String):void {require(shown()==want,why+": shows face "+shown()+", want "+want);}
  // Framed gold: the card's left edge, half way down, is the gold.
  private function gold(face:int):Boolean {var b:BitmapData=new BitmapData(CARD_W,CARD_H,true,0);b.draw(card(face));var c:uint=b.getPixel(0,CARD_H/2);b.dispose();return (c>>16)>0xC0 && (c&0xFF)<0x80;}
  private function golds():Array {var g:Array=[];for(var i:int=0;i<window.count;i++)if(gold(i))g.push(i);return g;}
  private function picture(face:int):DisplayObject {return card(face).getChildAt(0);}
  private function numbered(face:int):Boolean {var p:DisplayObjectContainer=picture(face) as DisplayObjectContainer;var f:TextField=p?p.getChildAt(p.numChildren-1) as TextField:null;return f!=null && f.text==String(face+1);}
  // A card's picture: its own pixels (w x h), drawn at the card's size, smoothed.
  private function sharp(shown:DisplayObject,w:int,h:int,cardW:int,cardH:int):Boolean {var b:Bitmap=shown as Bitmap;return b!=null && b.bitmapData.width==w && b.bitmapData.height==h && Math.abs(b.width-cardW)<0.01 && Math.abs(b.height-cardH)<0.01 && b.smoothing;}
  private function label(face:int):TextField {var c:DisplayObjectContainer=card(face);return c.getChildAt(c.numChildren-1) as TextField;}
  private function loaded(e:Event):void {
   try {
    domain=loader.contentLoaderInfo.applicationDomain;
    var Scene:Object=domain.getDefinition("MainScene");Scene.stage=this;
    Loc=domain.getDefinition("survarium.data.LocalizationData");Loc.setData(texts("english"));
    Choice=domain.getDefinition("survarium.face.FaceChoice");
    Modals=domain.getDefinition("survarium.controls.Modals");
    Window=domain.getDefinition("survarium.face.FaceWindow") as Class;
    LobbyWindow=domain.getDefinition("survarium.lobbypick.LobbyWindow") as Class;
    Panel=domain.getDefinition("top_fla.topPanelLeftContainer_56") as Class;
    require(!domain.hasDefinition("survarium.face.FaceChooser"),"the drop-down chooser is gone");
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
      require(button.visible,"shown before srp's account says, as the lobby button");
      require(!Choice.take([{place:2090000000,nickname:"F1;0;1;3"}]),"rows of another range pass by");
      require(answer("F2;0;0;3") && choice.count==0,"another version of the row is taken and read as nothing");
      Choice.told(3,3);require(choice.count==0,"a face past the count is no word");
      Choice.told(0,1);require(!button.visible,"one face: hidden");
      Choice.told(2,11);require(button.visible,"eleven faces: shown");
      expect(2,"srp's face");
      // Every language's word, upper case, inside the button and centred, as the lobby button's.
      for each(var lang:String in HarnessText.LANGUAGES) {
       Loc.setData(texts(lang));
       var b:Object=Object(new Panel()).btn_face;var l:TextField=b.getChildByName("panelLabel") as TextField;
       var want:String=String(HarnessText.DATA[lang]["st_srp_face"]).toUpperCase();
       var dx:Number=l.x+l.width/2-b.boxWidth/2;
       trace("FACE label "+lang+": '"+l.text+"' "+int(l.textWidth)+" in "+int(b.boxWidth)+", off centre "+dx.toFixed(1));
       require(l.text==want && l.width<=b.boxWidth-8 && Math.abs(dx)<=1,lang+": the label '"+l.text+"' fits, centred");
       // and the window's title and every card's name inside its band
       var w:Object=new Window();
       var t:TextField=w.getChildAt(0) as TextField;
       require(t!=null && t.text==want,lang+": the window's title '"+(t?t.text:null)+"'");
       for(var f:int=0;f<11;f++) {
        var n:TextField=DisplayObjectContainer(w.getChildByName(String(f))).getChildAt(3) as TextField;
        var name:String=String(HarnessText.DATA[lang]["st_srp_face_card"]).split("%1$s").join(String(f+1));
        require(n.text==name && n.textWidth<=n.width-4,lang+": face "+f+"'s name '"+n.text+"' "+int(n.textWidth)+" in "+int(n.width));
       }
       trace("FACE window "+lang+": '"+t.text+"', '"+n.text+"' "+int(n.textWidth)+" in "+int(n.width));
      }
      Loc.setData(texts("english"));
      trace("FACE shot button");wait=120;break;
     case 1:
      // The window, as the lobby window: in the middle, a modal; the faces in rows of six and
      // five (the second centred), srp's face gold; the movie's portraits (SrpFace0..9) at their own
      // pixels, twice the card's (a 4K screen draws the card so big), the number without one.
      window=new Window();
      var Cards:Class=domain.getDefinition("survarium.rooms.CardWindow") as Class;
      require(window is Cards && new LobbyWindow() is Cards,"the lobby window's kind (CardWindow)");
      open();
      require(Modals.top()==window,"a modal: Esc closes it");
      trace("FACE window: "+int(window.width)+" x "+int(window.height)+" at "+int(window.x)+","+int(window.y));
      require(Math.abs(window.x+window.width/2-stage.stageWidth/2)<=1 && Math.abs(window.y+window.height/2-stage.stageHeight/2)<=1,"in the middle of "+stage.stageWidth+" x "+stage.stageHeight);
      require(Object(Window).columnsOf(11)==6 && Object(Window).columnsOf(10)==5 && Object(Window).columnsOf(7)==4 && Object(Window).columnsOf(4)==4,"rows even");
      for(var i:int=0;i<11;i++) require(card(i)!=null,"a card a face: "+i);
      require(card(11)==null && window.count==11,"no card past the faces");
      require(card(5).y==card(0).y && card(6).y>card(0).y,"six a row");
      require(Math.abs((card(6).x-card(0).x)-(CARD_W+14)/2)<=1,"the shorter row centred: "+card(0).x+" "+card(6).x);
      require(golds().join()=="2","srp's face framed gold: "+golds());
      for(i=0;i<10;i++) require(sharp(picture(i),CARD_W*2,CARD_H*2,CARD_W,CARD_H),"face "+i+": its portrait, 280 x 360 drawn at the card's size");
      require(numbered(10),"no portrait: the face's number");
      var lp:DisplayObject=DisplayObjectContainer(new LobbyWindow().getChildByName("0")).getChildAt(0);
      require(sharp(lp,560,380,280,190),"the lobby window's pictures too: 560 x 380 drawn at the card's size");
      require(label(4).text=="Face 5","the band's name: "+label(4).text);
      trace("FACE shot window");wait=120;break;
     case 2:
      // A press chooses at once and closes, as the lobby window's.
      click(3);expect(3,"a press chooses at once");require(!window.isOpen && Modals.top()!=window,"closed by a choice");
      open();require(golds().join()=="3","the pick framed");
      routed(4);expect(4,"a press given to the window goes to the card under it");require(!window.isOpen,"closed");
      open();click(5);expect(5,"picks go on while the first is asked");
      answer("F1;1;3;11");expect(5,"the first choice's answer keeps the last pick");
      answer("F1;2;3;11");expect(3,"the next one refused: srp's face back");
      open();require(golds().join()=="3","the refusal framed");
      click(3);expect(3,"its own face: nothing asked");require(!window.isOpen,"and closed");
      open();click(10);answer("F1;1;10;11");expect(10,"chosen");
      // Esc closes it, the button's toggle too.
      open();require(Modals.escape() && !window.isOpen,"Esc closes it");
      window.toggle(host);require(window.isOpen,"opened again");window.toggle(host);require(!window.isOpen,"the toggle closes it");
      // One card window at a time: the lobby window's toggle closes the faces' and the faces' the
      // lobby's; the one open is closed at a match's load (CardWindow.closeOpen).
      var other:Object=new LobbyWindow();other.state(1,1);
      open();other.toggle(host);
      require(other.isOpen && !window.isOpen && Modals.top()==other,"the lobby window opened: the faces' closed");
      window.toggle(host);
      require(window.isOpen && !other.isOpen && Modals.top()==window,"the faces' opened: the lobby window closed");
      Object(domain.getDefinition("survarium.rooms.CardWindow")).closeOpen();
      require(!window.isOpen && !other.isOpen && !Modals.any(),"none open after a match's load");
      window.toggle(host);require(window.isOpen,"and opened again after");window.toggle(host);
      // Fewer faces: the cards made again; one face: closed.
      open();Choice.told(0,4);require(card(3)!=null && card(4)==null && window.count==4,"four cards");
      require(golds().join()=="0","srp's face framed");
      answer("F1;0;9;4");expect(0,"a face past the count is no answer");
      Choice.told(0,1);require(!button.visible && !window.isOpen,"one face: the button hidden, the window closed");
      Choice.told(6,11);
      window=new Window();open();
      require(golds().join()=="6","srp's face framed");
      trace("FACE shot icons");wait=120;break;
     case 3:
      // Beside the lobby window, for the look (compare.png): both as their buttons open them.
      window.close();
      DisplayObject(panel).visible=false;
      var lw:Object=new LobbyWindow();lw.state(3,3);
      lw.x=16;lw.y=int((H-lw.height)/2);host.addChild(lw as DisplayObject);
      window.x=int(lw.x+lw.width+16);window.y=int((H-window.height)/2);host.addChild(window as DisplayObject);
      trace("FACE shot compare");wait=120;break;
     case 4:
      finish("pass");break;
    }
   } catch(err:Error){finish("fail: "+err.message);}
  }
  private function finish(what:String):void {removeEventListener(Event.ENTER_FRAME,tick);trace("FACE RESULT "+what);}
 }
}
