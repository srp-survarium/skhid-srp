package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.Point;
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
   // The ammo counter, placed by the hud's layout: nothing of a spectator's chooser moves it.
   hud.update_current_player_weapon_stats([{name:"5.45x39 7N6M",count:60,icon:0}],0,0,true,30,true,10);
   hud.ammoInd.validateNow();
   var ammoAt:Number = hud.playerInfo.x;
   trace("KITS ammo playerInfo.x=" + ammoAt + " w=" + hud.playerInfo.width);
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
   var shown:Function = function(at:int):Boolean {
    var button:* = selector.tabPage.getButtonAt(at);
    return button != null && button.selected && button.currentLabel != null && button.currentLabel.indexOf("selected_") == 0;
   };
   expect("kit 2's plate shows selected (" + selector.tabPage.getButtonAt(1).currentLabel + ")",shown(1));
   // A plate that lost its selected state, the bar's index still on it (what the bar's own setter would skip).
   selector.tabPage.getButtonAt(1).selected = false;
   hud.srp_spectator_kits_show(1);
   expect("the plate's selected state restored on the next frame's show (" + selector.tabPage.getButtonAt(1).currentLabel + ")",shown(1));
   selector.tabPage.getButtonAt(1).gotoAndPlay("up");
   hud.srp_spectator_kits_show(1);
   expect("the plate's selected frame restored on the next frame's show (" + selector.tabPage.getButtonAt(1).currentLabel + ")",shown(1));
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
   expect("the hud laid out: the ammo counter at the bottom right " + rect(hud.ammoInd),rect(hud.ammoInd).right > w - 100 * hud.scaleX && rect(hud.ammoInd).bottom > h - 100 * hud.scaleX);
   hud.show_slots(false);                 // what the exe holds, every frame, for both
   expect("the quick slots hidden",!hud.quick_slots.visible);
   for each(var drawn:String in ["minimap","ammoInd","chat"]) hud[drawn].visible = true;
   for each(var mode:int in [1,2]) {
    hud.srp_spectator_kits_show(mode);
    expect((mode == 1 ? "live" : "replay") + ": the chooser does not move the ammo counter (" + hud.playerInfo.x + ", was " + ammoAt + ")",hud.playerInfo.x == ammoAt);
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
   hud.srp_spectator_current_kit(0);
   var list:* = panel.getChildAt(panel.numChildren - 1);   // the hover list
   trace("KITS bar scaleX=" + selector.tabPage.scaleX + " width=" + selector.tabPage.width + " on the stage " + plate + " plate 1's middle local " + selector.tabPage.globalToLocal(new Point(plate.x + plate.width / 2,plate.y + 10)));
   for(var at:int = 0; at < 3; at++) {
    hud.srp_spectator_pointer(uint(plate.x + plate.width * (at + 0.5) / 3),uint(plate.y + 10));
    var wanted:Number = selector.x + at * 187;
    trace("KITS hover plate " + at + " list.x=" + list.x + " wanted=" + wanted);
    expect("hover: plate " + at + " shows the list",list.visible);
    if(at < 2) expect("hover: the list opens above plate " + at + " (x " + list.x + ", wanted " + wanted + ")",Math.abs(list.x - wanted) < 1);
    else expect("hover: the list reaches the last plate (x " + list.x + " to " + (list.x + 435) + ")",list.x > selector.x + 187 - 20 && list.x + 435 >= wanted + 187);
    var kept:Number = list.x;
    hud.srp_spectator_kits_show(2);
    expect("hover: the frame's show leaves the list on plate " + at,list.visible && list.x == kept);
    expect("hover: the highlight stays on the kit in use",selector.tabPage.selectedIndex == 0 && shown(0));
   }
   // The hud's own mouse (the game's handler for a player) does not take a spectator's list.
   var heldAt:Number = list.x;
   selector.tabPage.getButtonAt(0).dispatchEvent(new MouseEvent(MouseEvent.MOUSE_OVER,true));
   expect("hover: the game's own mouse-over does not move a spectator's list (x " + list.x + ")",list.x == heldAt);
   selector.tabPage.getButtonAt(0).dispatchEvent(new MouseEvent(MouseEvent.MOUSE_OUT,true));
   expect("hover: the game's own mouse-out does not hide a spectator's list",list.visible);
   hud.srp_spectator_pointer(uint(plate.x + 20),uint(plate.y - 200));
   expect("hover: off the plates the list goes",!list.visible);
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
   hud.show_profiles_selector(false);
   everySize(hud,player,panel,selector);
  }
  // Every size: where each layout's pieces are, on the worst kit (every row of the list, six quick slots)
  // and the fullest player list (20 rows), by bounds on the stage, as a table.
  private function box(r:Rectangle):String { return int(r.x) + "," + int(r.y) + " " + int(r.width) + "x" + int(r.height); }
  private function middle(r:Rectangle):String { return int(r.x + r.width / 2) + "," + int(r.y + r.height / 2); }
  private function onStage(what:String, r:Rectangle):void {
   expect(what + " on the stage " + r,r.x >= 0 && r.y >= 0 && r.right <= stage.stageWidth && r.bottom <= stage.stageHeight);
  }
  private function everySize(hud:*, player:*, panel:*, selector:*):void {
   var w:int = stage.stageWidth, h:int = stage.stageHeight, size:String = w + "x" + h;
   var wanted:String = root.loaderInfo.parameters.size;
   expect("the stage is the size asked for (" + wanted + ", " + size + ")",wanted == size);
   var dict:Array = [], worst:Array = [];
   for(var i:int = 0; i < 19; i++) {
    dict.push({dictId:1000 + i,name:"Chemical protection boots, reinforced",is_stack:i >= 13,category:0,icon:0,level:1});
    worst.push((1000 + i) + ":" + i + ":3");
   }
   hud.itemDescriptor.setItemsDictionary(dict);
   var kit:String = worst.join(",");
   var rows:Array = [];
   for(var n:int = 0; n < 20; n++) rows.push(n + ":" + (n % 2) + ":Player " + n);
   player.set_players_text(rows.join(";"));
   hud.show_player_list(false);
   player.set_cursor_shown(false);
   stage.dispatchEvent(new Event(Event.RESIZE));
   player.viewer.place();
   var list:* = panel.getChildAt(panel.numChildren - 1);
   var v:* = player.viewer, doll:* = list.profileItems;
   var around:Array = [["the seek bar",player.bottomPanel],["the key hint",v.hint],["the Moments switch",v.moments],["Back",v.back],
    ["the minimap",hud.minimap],["the ammo counter",hud.ammoInd],["the chat",hud.chat],["the player list",player.playersBar],["free view",player.freeViewButton]];
   var clear:Function = function(what:String, r:Rectangle):void { for each(var a:Array in around) expect(size + ": " + what + " " + r + " over " + a[0] + " " + rect(a[1]),!r.intersects(rect(a[1]))); };
   for each(var a:Array in around) if(a[0] != "the chat" && a[0] != "the minimap" && a[0] != "the ammo counter") onStage(size + ": " + a[0],rect(a[1]));
   // The viewer's own controls.
   var mine:Array = around.slice(0,4);
   for(var x:int = 0; x < mine.length; x++) for(var y:int = x + 1; y < mine.length; y++) expect(size + ": " + mine[x][0] + " over " + mine[y][0],!rect(mine[x][1]).intersects(rect(mine[y][1])));
   // A replay's viewer.
   hud.srp_spectator_kits("0|0|7|Alpha|" + kit + ";1|0|5|Bravo|" + kit + ";2|0|4|Charlie|" + kit + ";");
   hud.srp_spectator_current_kit(0);
   hud.srp_spectator_kits_show(2);
   expect(size + ": a replay's chooser shows",panel.visible);
   var line:String = "KITS TABLE " + size + " scale=" + hud.scaleX.toFixed(3) + " replay=" + player.scaleX.toFixed(3) + " chooser " + box(rect(selector));
   clear("the chooser",rect(selector));
   onStage(size + ": the chooser",rect(selector));
   line += " plates " + box(rect(selector.tabPage));
   var coords:String = "KITS COORDS " + size + " plates";
   var plates:Array = [];
   for(var p:int = 0; p < 3; p++) {
    plates.push(rect(selector.tabPage.getButtonAt(p)));
    onStage(size + ": plate " + p,plates[p]);
    coords += " " + middle(plates[p]);
   }
   for(p = 0; p < 3; p++) {
    hud.srp_spectator_pointer(uint(plates[p].x + plates[p].width / 2),uint(plates[p].y + plates[p].height / 2));
    doll.validateNow();
    expect(size + ": hover " + p + " shows",list.visible);
    expect(size + ": hover " + p + " has the kit's six quick rows (" + doll.itemsBack.height + ")",doll.itemsBack.visible && doll.itemsBack.height == 10 + 27 * 6 + 9);
    var whole:Rectangle = rect(doll), side:Rectangle = rect(doll.itemsBack);
    var items:Rectangle = whole.clone(); items.right = side.x;
    onStage(size + ": hover " + p + " list",whole); onStage(size + ": hover " + p + " side panel",side);
    clear("hover " + p + " list",items); clear("hover " + p + " side panel",side);
    expect(size + ": hover " + p + " list ends above the plates (" + whole.bottom + ", plates at " + plates[p].y + ")",whole.bottom <= plates[p].y + 1);
    line += " | hover" + p + " list " + box(items) + " side " + box(side);
    // kept by the frame's show
    var keptX:Number = list.x; hud.srp_spectator_kits_show(2);
    expect(size + ": hover " + p + " kept by the frame's show",list.visible && list.x == keptX);
   }
   hud.srp_spectator_pointer(uint(0),uint(0));
   expect(size + ": hover goes off the plates",!list.visible);
   trace(line);
   // The player list as a two-player match shows it (for the real client's check).
   player.set_players_text("0:0:ReplayTester;1:1:SecondTester");
   stage.dispatchEvent(new Event(Event.RESIZE));
   player.playersBar.validateNow();
   coords += " rows";
   for(var row:int = 0; row < 2; row++) coords += " " + middle(rect(player.playersBar.getButtonAt(row)));
   coords += " freeview " + middle(rect(player.freeViewButton));
   trace(coords);
   // A live spectator, the game's own place, one to three plates (no cursor there: its hover measured all the same).
   for(var count:int = 1; count <= 3; count++) {
    var live:Array = [];
    for(var q:int = 0; q < count; q++) live.push(q + "|0|5|Kit|" + kit);
    hud.srp_spectator_kits(live.join(";") + ";");
    hud.srp_spectator_kits_show(1);
    var chooser:Rectangle = rect(selector.tabPage);
    onStage(size + ": live, " + count + " plates",chooser);
    var liveLine:String = "KITS LIVE " + size + " plates=" + count + " " + box(chooser);
    for(var t:int = 0; t < count; t++) {
     var one:Rectangle = rect(selector.tabPage.getButtonAt(t));
     hud.srp_spectator_pointer(uint(one.x + one.width / 2),uint(one.y + one.height / 2));
     doll.validateNow();
     var hovered:Rectangle = rect(doll);
     onStage(size + ": live, " + count + " plates, hover " + t,hovered);
     for each(var fixed:* in [hud.minimap,hud.ammoInd]) expect(size + ": live hover over the hud's corner " + hovered,!hovered.intersects(rect(fixed)));
     liveLine += " hover" + t + " " + box(hovered);
    }
    hud.srp_spectator_pointer(uint(0),uint(0));
    trace(liveLine);
   }
   // A player's own chooser: the game's own feed and place.
   hud.srp_spectator_kits_show(0);
   hud.setup_player_profiles([{name:"A",state:0,level:1,items:[]},{name:"B",state:0,level:1,items:[]},{name:"C",state:0,level:1,items:[]}]);
   hud.show_profiles_selector(true);
   var ownBox:Rectangle = rect(selector);
   expect(size + ": a player's chooser centred on the stage " + ownBox,Math.abs(ownBox.x + ownBox.width / 2 - w / 2) < 40 * hud.scaleX);
   expect(size + ": a player's chooser at the bottom " + ownBox,ownBox.bottom > h * 0.85 && ownBox.bottom <= h);
   trace("KITS PLAYER " + size + " chooser " + box(ownBox));
   hud.show_profiles_selector(false);
  }
  private function finish(error:String):void {
   if(error) failures.push(error);
   for each(var f:String in failures) trace("KITS FAIL " + f);
   trace("KITS RESULT " + (failures.length ? "fail: " + failures.length : "pass"));
   fscommand("quit");
  }
 }
}
