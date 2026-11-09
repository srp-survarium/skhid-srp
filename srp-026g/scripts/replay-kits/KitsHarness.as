package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.Rectangle;
 import flash.net.URLRequest;
 import flash.system.*;
 import flash.utils.getQualifiedClassName;

 // A spectator's kit chooser, in the hud movie's own component: what it shows of what it is
 // given, and where it sits against what each layout shows. Bounds are measured on the stage.
 public class KitsHarness extends HUD {
  private var playerLoader:Loader = new Loader();
  private var failures:Array = [];
  private var waits:int = 0;
  private var started:Boolean = false;
  public function KitsHarness() {
   super();
   // Layout reads `layoutData` off every child of the root, which any object answers in Scaleform and a
   // sealed class throws at in Flash and Ruffle: what is left of those (hud_root.py makes HUD's own
   // containers clips) is out of the movie for it. None is drawn in a spectator's layouts.
   for(var i:int = numChildren - 1; i >= 0; i--) {
    if(getChildAt(i) is Sprite && !(getChildAt(i) is MovieClip) && getQualifiedClassName(getChildAt(i)).indexOf("::Layout") < 0) { trace("KITS out of the layout: " + getQualifiedClassName(getChildAt(i))); removeChildAt(i); }
   }
   stage.addChild(playerLoader);
   playerLoader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void { e.preventDefault(); });
   playerLoader.load(new URLRequest("replay_player.swf"),new LoaderContext(false,new ApplicationDomain()));
   addEventListener(Event.ENTER_FRAME,frame);
  }
  private function frame(e:Event):void {
   if(started) return;
   var hud:* = this, player:* = playerLoader.content;
   waits++;
   if(player == null) {
    if(waits > 900) finish("the movies did not load");
    return;
   }
   // Give the movies some frames to make their children.
   if(waits < 60) return;
   started = true;
   try { run(hud,player); } catch(err:Error) { failures.push("error: " + err.message + " " + err.getStackTrace()); }
   finish(null);
  }
  private function expect(what:String, ok:Boolean):void { if(!ok) failures.push(what); }
  private function rect(what:DisplayObject):Rectangle { return what.getBounds(stage); }
  private function clear(a:Rectangle, b:Rectangle):Boolean { return !a.intersects(b); }
  private function notOver(layout:String, panel:Rectangle, name:String, object:*, always:Boolean = false):void {
   if(object == null) { trace("KITS " + layout + " " + name + " absent"); return; }
   var bounds:Rectangle = rect(object);
   trace("KITS " + layout + " " + name + " " + bounds + " visible=" + object.visible);
   if((object.visible || always) && bounds.width > 0) expect(layout + ": the chooser " + panel + " overlaps " + name + " " + bounds,clear(panel,bounds));
  }
  private function run(hud:*, player:*):void {
   var panel:* = hud.spectatorKitsPanel, selector:* = panel.profileSelector;
   var plates:Function = function():int { return selector.tabPage.dataProvider ? selector.tabPage.dataProvider.length : 0; };
   stage.dispatchEvent(new Event(Event.RESIZE));
   // A player's game, before any call of a spectator's: the game's own feed and its own show.
   hud.setup_player_profiles([{name:"Profile 1",state:0,level:1,items:[]},{name:"Profile 2",state:0,level:2,items:[]},{name:"Profile 3",state:1,level:0,items:[]}]);
   hud.show_profiles_selector(true);
   expect("a player: the chooser shown",panel.visible);
   expect("a player: its three plates (" + plates() + ")",plates() == 3);
   expect("a player: the chooser its own",selector.local == true);
   expect("a player: an incomplete kit not playable",selector.getProfileData(2).enabled == false && selector.getProfileData(0).enabled == true);
   hud.select_player_profile(1);
   expect("a player: the game's own select",selector.tabPage.selectedIndex == 1);
   var own:Rectangle = rect(selector);
   trace("KITS player " + stage.stageWidth + "x" + stage.stageHeight + " chooser " + own);
   expect("a player: the game's own place, bottom centre " + own,own.x > stage.stageWidth * 0.25 && own.right < stage.stageWidth * 0.75 && own.bottom > stage.stageHeight * 0.85);
   hud.show_profiles_selector(false);
   expect("a player: the game's own hide",!panel.visible);
   // Nothing given: nothing shown.
   hud.srp_spectator_kits_show(1);
   expect("no kits given, the chooser hidden",!panel.visible);
   // Live: kits 0 and 2 used so far, kit 1 not: only their plates.
   hud.srp_spectator_kits("0|0|7||77:0:1,78:1:3;");
   expect("a kit's items (" + selector.getProfileData(0).items.length + ")",selector.getProfileData(0).items.length == 2 && selector.getProfileData(0).items[1].dictId == 78 && selector.getProfileData(0).items[1].condition_or_stack == 3);
   hud.srp_spectator_kits("0|0|7|Alpha|;2|0|5|Bravo|;");   // no items: the item table the hover list reads is the exe's
   hud.srp_spectator_kits_show(1);
   expect("two kits given, shown",panel.visible);
   expect("two plates (" + plates() + ")",plates() == 2);
   expect("the second plate is kit 2's (" + selector.getProfileData(1).name + ")",selector.getProfileData(1).name == "Bravo");
   hud.srp_spectator_current_kit(2);
   expect("the highlight on kit 2's plate",selector.tabPage.selectedIndex == 1);
   expect("its glow shown",selector.selectedTabGlow.visible);
   hud.select_player_profile(0);
   expect("the game's own select does not move a spectator's highlight",selector.tabPage.selectedIndex == 1);
   selector.selectProfile(0);
   hud.srp_spectator_kits_show(1);
   expect("the highlight held on the next frame's show",selector.tabPage.selectedIndex == 1);
   hud.srp_spectator_current_kit(1);
   expect("a kit not given moves nothing",selector.tabPage.selectedIndex == 1);
   // Read-only: a pick tells the game nothing.
   expect("the chooser is not the local player's",selector.local == false);
   // The native show call does not bring it up or down.
   hud.show_profiles_selector(false);
   expect("the game's own hide does not hide a spectator's chooser",panel.visible);
   // The game's own place, live.
   trace("KITS scale " + hud.scaleX + " minimap " + rect(hud.minimap) + " ammo " + rect(hud.ammoInd) + " slots " + rect(hud.quick_slots) + " list " + rect(hud.player_list));
   var w:int = stage.stageWidth, h:int = stage.stageHeight;
   expect("the hud laid out: the ammo counter at the bottom right " + rect(hud.ammoInd),rect(hud.ammoInd).right > w - 100 && rect(hud.ammoInd).bottom > h - 100);
   hud.show_slots(false);                 // what the exe holds, every frame, for both
   expect("the quick slots hidden",!hud.quick_slots.visible);
   for each(var shown:String in ["minimap","ammoInd","chat"]) hud[shown].visible = true;
   for each(var mode:int in [1,2]) {
    hud.srp_spectator_kits_show(mode);
    hud.show_player_list(true);
    if(mode == 2) {
     expect("replay: the chooser hidden while the player list is up",!panel.visible);
     hud.show_player_list(false);
     expect("replay: and back when it goes",panel.visible);
    }
    var box:Rectangle = rect(selector);
    var layout:String = (mode == 1 ? "live" : "replay") + " " + w + "x" + h;
    trace("KITS " + layout + " chooser " + box + " panel.y=" + panel.y);
    expect(layout + ": the chooser on the stage " + box,box.x >= 0 && box.y >= 0 && box.right <= w && box.bottom <= h);
    for each(var n:String in ["minimap","ammoInd","quick_slots","player_list","chat"]) {
     notOver(layout,box,n,hud[n]);
    }
    if(mode == 2) {
     player.set_match_time(600000);
     player.start_viewer("Back","Moments","All","Mine","Followed","Cursor","Pause","Filter","Hide","Ctrl","Space");
     player.set_moments(0,0);
     player.set_cursor_shown(true);
     player.viewer.place();
     var v:* = player.viewer;
     notOver(layout,box,"the seek bar",player.bottomPanel);
     notOver(layout,box,"the key hint",v.hint,true);   // shown while the cursor is not
     notOver(layout,box,"the Moments switch",v.moments);
     notOver(layout,box,"Back",v.back);
    }
   }
   // Hover: over a plate its list shows and off it goes (the pointer, as the exe gives it).
   hud.srp_spectator_kits("0|0|7|Alpha|;1|0|5||;2|0|5||;");
   hud.srp_spectator_kits_show(2);
   var plate:Rectangle = rect(selector.tabPage);
   var viewer:* = player.viewer;
   try {
    hud.srp_spectator_pointer(uint(plate.x + plate.width * 5 / 6),uint(plate.y + 10));   // the last plate, the list's furthest right
    var hovered:Rectangle = rect(panel);
    trace("KITS hover " + w + "x" + h + " panel " + hovered);
    expect("the hover list on the stage " + hovered,hovered.x >= 0 && hovered.y >= 0 && hovered.right <= w && hovered.bottom <= h);
    for each(var over:String in ["minimap","ammoInd","player_list","chat"]) notOver("hover " + w + "x" + h,hovered,over,hud[over]);
    notOver("hover " + w + "x" + h,hovered,"the seek bar",player.bottomPanel);
    notOver("hover " + w + "x" + h,hovered,"the key hint",viewer.hint,true);
    notOver("hover " + w + "x" + h,hovered,"the Moments switch",viewer.moments);
    notOver("hover " + w + "x" + h,hovered,"Back",viewer.back);
   } catch(hover:Error) { failures.push("the hover list: " + hover.message + " " + hover.getStackTrace()); }
   hud.srp_spectator_pointer(uint(plate.x + 20),uint(plate.y - 200));
   // Mode 0 gives the movie back to a player's next match.
   hud.srp_spectator_kits_show(0);
   expect("mode 0 hides it",!panel.visible);
   expect("mode 0: the selector its own again",selector.local == true);
   hud.show_profiles_selector(true);
   expect("mode 0: the game's own show is honoured",panel.visible);
   // A player's match after a spectator's, the movie being the last match's.
   hud.srp_spectator_kits_show(2);
   try { hud.reset(); } catch(rest:Error) { trace("KITS reset outside the game: " + rest.message); }
   hud.show_profiles_selector(true);
   expect("reset: the game's own show honoured after a spectator's match",panel.visible);
   expect("reset: the selector its own again",selector.local == true);
  }
  private function finish(error:String):void {
   if(error) failures.push(error);
   for each(var f:String in failures) trace("KITS FAIL " + f);
   trace("KITS RESULT " + (failures.length ? "fail: " + failures.length : "pass"));
   fscommand("quit");
  }
 }
}
