package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.URLRequest;
 import flash.system.*;
 import flash.text.TextField;
 // The hud's talking list (VoiceSpeakers, docs/notes/voice-chat.md) as exe patch 66 calls it: each
 // speaker's name and side. A teammate's speaker stays 0.69's blue, an opponent's pale red; a
 // speaker whose side changes while it talks is redrawn (a free-for-all: everyone a mate, blue);
 // one no longer sent fades. SHOT: the list with one teammate and one opponent, held for the
 // screenshot.
 public class VoiceSpeakersHarness extends Sprite {
  private static const BLUE:uint = 0x73B9FF;
  private static const RED:uint = 0xFF8080;
  private var loader:Loader = new Loader();
  private var failures:Array = [];
  private var list:Object;
  private var frame:int = 0;
  public function VoiceSpeakersHarness() {
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void { e.preventDefault(); });
   loader.load(new URLRequest("hud.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function loaded(e:Event):void {
   try {
    var Speakers:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("survarium.controls.VoiceSpeakers") as Class;
    list = new Speakers();
    list.x = 1240; list.y = 410;
    addChild(DisplayObject(list));
    list.update(["Sokol","mate","Hunter_77","opponent"]);
    expect("Hunter_77",RED);
    expect("Sokol",BLUE);
    trace("VOICE SHOT talking");
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(err:Error) { finish("setup: " + err); }
  }
  private function tick(e:Event):void {
   frame++;
   try {
    if(frame == 90) {
     // Both mates (a free-for-all): the opponent's speaker redrawn blue, no new row.
     list.update(["Sokol","mate","Hunter_77","mate"]);
     expect("Hunter_77",BLUE);
     expect("Sokol",BLUE);
     list.update(["Sokol","mate","Hunter_77","opponent"]);
     expect("Hunter_77",RED);
     // The opponent no longer sent: faded out and gone after 1.5 s.
     list.update(["Sokol","mate"]);
    }
    if(frame == 90 + 120) {
     if(row("Hunter_77") != null) throw new Error("Hunter_77 not gone after its fade");
     expect("Sokol",BLUE);
     finish(null);
    }
   } catch(err:Error) { finish(err.message); }
  }
  // The row of a name (a sprite of its name and speaker).
  private function row(name:String):Sprite {
   for(var i:int = 0; i < list.numChildren; i++) {
    var sprite:Sprite = list.getChildAt(i) as Sprite;
    if(sprite && sprite.numChildren == 2 && TextField(sprite.getChildAt(0)).text == name) return sprite;
   }
   return null;
  }
  // The name's row: one, the speaker's box (drawRect 2,9,5,8) of that colour, the name white.
  private function expect(name:String, colour:uint):void {
   var sprite:Sprite = row(name);
   if(sprite == null) throw new Error(name + ": no row");
   var count:int = 0;
   for(var i:int = 0; i < list.numChildren; i++) if(Sprite(list.getChildAt(i)).getChildAt(0) is TextField && TextField(Sprite(list.getChildAt(i)).getChildAt(0)).text == name) count++;
   if(count != 1) throw new Error(name + ": " + count + " rows");
   var mark:DisplayObject = sprite.getChildAt(1);
   var shot:BitmapData = new BitmapData(4,4,false,0);
   shot.draw(mark,new Matrix(1,0,0,1,-3,-11));
   var got:uint = shot.getPixel(1,1);
   if(got != colour) throw new Error(name + ": speaker 0x" + got.toString(16) + ", not 0x" + colour.toString(16));
   if(TextField(sprite.getChildAt(0)).textColor != 0xFFFFFF) throw new Error(name + ": the name not white");
   trace("VOICE " + name + " speaker 0x" + got.toString(16).toUpperCase());
  }
  private function finish(error:String):void {
   removeEventListener(Event.ENTER_FRAME,tick);
   if(error) failures.push(error);
   trace("VOICE RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
   fscommand("quit");
  }
 }
}
