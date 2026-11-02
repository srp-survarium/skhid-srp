package {
 import flash.display.*;
 import flash.events.*;
 import flash.net.URLRequest;
 import flash.system.*;
 import flash.text.TextField;

 // Exercise the shipped HUD methods. Its native-only initialization may
 // stop outside the game; the prompt and localization have no native calls.
 public class CarryHarness extends Sprite {
  private var loader:Loader = new Loader();
  public function CarryHarness() {
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void { e.preventDefault(); });
   loader.contentLoaderInfo.addEventListener(IOErrorEvent.IO_ERROR,function(e:Event):void { finish(String(e)); });
   addChild(loader);
   loader.load(new URLRequest("hud.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function loaded(e:Event):void {
   try {
    var hud:Object = loader.content;
    hud.set_localization_data({st_artefact_take_teammate:"Take artifact (E)",st_weapon_pick_up:"Pick up %1$s (E)"});
    // These native HUD methods set their state before touching game-only
    // view objects, which need not exist in this headless fixture.
    try { hud.show_profiles_selector(false); } catch(nativeProfile:Error) {}
    if(!hud.contextInfo) {
     var Widget:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("SrvContextInfo") as Class;
     hud.contextInfo = new Widget();
     hud.addChild(hud.contextInfo);
    }
    hud.set_context("Native collection",37);
    hud.show_artefact_offer("st_artefact_take_teammate");
    var prompt:Object = hud.contextInfo;
    if(!prompt.visible || prompt.contextText.text != "Take artifact (E)" || prompt.currentFrame != 1) throw new Error("native collection widget did not show localized take action");
    hud.hide_context();
    hud.show_context(false);
    if(!prompt.visible || prompt.contextText.text != "Take artifact (E)") throw new Error("native hide erased take action");
    hud.set_context("Native collection",37);
    if(prompt.contextText.text != "Take artifact (E)" || prompt.currentFrame != 1) throw new Error("native collection overwrote take action");
    hud.show_artefact_offer("");
    if(!prompt.visible || prompt.contextText.text != "Native collection" || prompt.currentFrame != 2) throw new Error("native collection was not restored");
    hud.hide_context();
    hud.show_context(true);
    if(!prompt.visible || prompt.contextText.text != "Native collection") throw new Error("native show did not retain text");
    hud.hide_context();
    // Patch 85: the dropped gun's prompt names the gun; the next offer has no name.
    hud.show_artefact_offer("st_weapon_pick_up","SVD");
    if(!prompt.visible || prompt.contextText.text != "Pick up SVD (E)" || prompt.currentFrame != 1) throw new Error("weapon pickup prompt wrong: " + prompt.contextText.text);
    hud.show_artefact_offer("st_artefact_take_teammate");
    if(!prompt.visible || prompt.contextText.text != "Take artifact (E)") throw new Error("take message wrong");
    hud.show_artefact_offer("");
    if(prompt.visible) throw new Error("lost target left prompt visible");
    hud.show_artefact_offer("st_artefact_take_teammate");
    try { hud.focus_chat(true); } catch(nativeChat:Error) {}
    hud.show_artefact_offer("st_artefact_take_teammate");
    if(prompt.visible) throw new Error("chat did not hide prompt");
    try { hud.focus_chat(false); } catch(nativeChatEnd:Error) {}
    hud.show_artefact_offer("st_artefact_take_teammate");
    if(!prompt.visible) throw new Error("chat close did not restore prompt");
    try { hud.show_profiles_selector(true); } catch(nativeKits:Error) {}
    hud.show_artefact_offer("st_artefact_take_teammate");
    if(prompt.visible) throw new Error("kit selector did not hide prompt");
    finish(null);
   } catch(err:Error) { finish(String(err)); }
  }
  private function finish(error:String):void {
   trace("CARRY RESULT " + (error ? "fail: " + error : "pass"));
   fscommand("quit");
  }
 }
}
