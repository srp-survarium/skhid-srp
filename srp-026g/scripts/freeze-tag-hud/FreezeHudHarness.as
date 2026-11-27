package
{
   import flash.display.*;
   import flash.events.*;
   import flash.net.URLRequest;
   import flash.system.*;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.utils.getQualifiedClassName;
   import flash.utils.getTimer;

   // Freeze Tag's frozen teammates in the hud movie (docs/notes/freeze-tag.md, "The frozen name"), its
   // own root fed as the game feeds it: a match of mode 12 (the map, the local player's team, the
   // players' dots, the Tab list's rows) and patch 160's root.freeze_tag_rows(mask). Players: the local
   // one (0), teammates 1 (plain), 2 (plain), 3 (his squad), 4 (his squad, carrying), an enemy 10
   // (spotted). Frozen: 1, 3, 4 and 10, and the local player himself. Checks:
   // - on the minimap and the Tab map a frozen teammate's glyph is ice blue (a squad mate's too, its
   //   carry mark kept), the others keep their own (white, the squad's green), the enemy's dot and the
   //   local player's are as they are; the Tab map's names (Alt) of the frozen teammates ice blue;
   // - the Tab list's names of every frozen place ice blue (both teams, as before);
   // - a thaw gives each its own colour back; a dot made while its place is frozen (a class change, a
   //   new player) is blue from the start; a map held (the Tab list closed) draws it when it shows;
   // - the round's restart (mask 0), another mode and a new match leave nothing blue;
   // - the enemy team's last one fighting (11, far; then near): listed, his icon draws nothing until
   //   root.freeze_tag_revealed names him; then a red mark on the Tab map (on the screen, red there) and
   //   on the corner minimap by HUD.REVEALED_ON_MINIMAP; a teammate's bit and a spotted enemy unmarked;
   //   marked when a held Tab map shows; none at mask 0, in another mode or a new match;
   // - in every language: Freeze Tag's mode notice under the timer and root.freeze_tag_reveal's
   //   popups (the game's short message): one when a side starts counting, its seconds then rounded
   //   (a late joiner's real number), no countdown after it, one when he is revealed; both sides at
   //   once both lines;
   // - a timeline at the exe's cadence (patch 160's on_hud every 40 ms: score, update, rows, revealed,
   //   reveal): the hunters' popup, then the revealed one (the popup's second showing), each fully
   //   shown for 4 s between its fades, the revealed one told again within it refreshed (no dip, its
   //   hold from then), each gone after its fade; and beside it the shared short message
   //   (set_parameterized_message) as Free for All calls it: its welcome run to its end, then its
   //   end's verdict, the clip's second message, fully shown its 6 s too (the owner, 2026-11-27: both
   //   "disappear almost immediately" in the game).
   // "FREEZE SHOT maps" for check.sh's screenshot; ends with "FREEZE RESULT pass" or "... fail: ...".
   public class FreezeHudHarness extends Sprite
   {
      private var ICE:uint;
      private static const SQUAD:uint = 0x66CC00;
      private var loader:Loader = new Loader();
      private var hud:*;
      private var failures:Array = [];
      private var phase:int = 0;
      private var frames:int = 0;
      // The local player's squad mates (their glyph green).
      private var squad:Array = [3,4];
      // The enemy team's last one fighting (FreezeTag::last_standing): listed by the exe unspotted, far from
      // the local player (beyond the corner minimap's circle).
      private static const LAST:int = 11;
      // The timeline (phase 4): its start, the exe's next frame (every 40 ms), the samples' lows.
      private var timelineStart:int;
      private var timelineNext:int;
      private var timelineTexts:Object = {};
      private var timelineLow:Object = {};
      private var timelineSeen:Object = {};

      public function FreezeHudHarness()
      {
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
            function(e:UncaughtErrorEvent):void { e.preventDefault(); failures.push(String(e.error)); trace("FREEZE uncaught " + e.error); });
         loader.contentLoaderInfo.addEventListener(Event.INIT,function(e:Event):void { stage.addChild(loader.content); });
         loader.load(new URLRequest("hud.swf"),new LoaderContext(false,new ApplicationDomain()));
         addEventListener(Event.ENTER_FRAME,tick);
      }

      private function check(ok:Boolean, message:String) : void
      {
         trace("FREEZE " + (ok ? "ok " : "FAIL ") + message);
         if(!ok) failures.push(message);
      }

      private static function player(id:int, team:uint, x:Number, y:Number, squad:Boolean = false, carry:Boolean = false) : Object
      {
         return {player_id:id,team:team,type:1,player_pos_x:x,player_pos_y:y,player_rotation_in_rad:0,
            is_local_player:id == 0,is_carrying_item:carry,is_squad_player:squad};
      }

      private static function players() : Array
      {
         return [player(0,0,-20,0),player(1,0,-38,18),player(2,0,-8,22),player(3,0,-30,-22,true),
            player(4,0,2,-18,true,true),player(10,1,30,4),player(LAST,1,95,85)];
      }

      private static function row(id:int, name:String, ally:Boolean) : Object
      {
         return {id:id,name:name,team:ally ? 1 : 2,kills:3,deaths:2,rank:30,score:30,ping:40};
      }

      private static function frozen(places:Array) : uint
      {
         var bits:uint = 0;
         for each(var place:int in places) bits |= 1 << place;
         return bits;
      }

      private function matchStart() : void
      {
         hud.reset();
         hud.set_game_type(12);
         hud.set_minimap_props({map:"level_01"});
         hud.set_local_player("Hunter");
         hud.update_local_player(-20,0,0.6,0);
         hud.update_players(players());
         hud.list_set_players([row(0,"Hunter",true),row(1,"Wolf",true),row(2,"Lynx",true),row(3,"Bear",true),
            row(4,"Elk",true),row(10,"Raven",false),row(11,"Fox",false)]);
         hud.show_player_list(true);
         hud.show_map_names(true);
      }

      // A map's dot of a place: its glyph's colour (the upper part named renderer), its name's colour,
      // whether it is the class's own (null: none).
      private function dot(minimap:*, place:int) : Object
      {
         var players:DisplayObjectContainer = minimap.minimap._players;
         for(var i:int = 0; i < players.numChildren; i++)
         {
            var icon:* = players.getChildAt(i);
            if(icon.index != place) continue;
            var glyph:DisplayObject = null;
            var label:TextField = null;
            var marks:int = 0;
            var iced:Boolean = false;
            for(var j:int = 0; j < icon.numChildren; j++)
            {
               var part:DisplayObject = icon.getChildAt(j);
               iced = iced || part.transform.colorTransform.redMultiplier == 0 && part.transform.colorTransform.color == ICE;
               if(part.name == "renderer") glyph = part;
               else if(part is TextField && part.visible) label = TextField(part);
               else marks++;
            }
            return {glyph:glyph ? glyph.transform.colorTransform.color : -1,
               tinted:glyph != null && glyph.transform.colorTransform.redMultiplier == 0,
               name:label ? label.textColor : -1,marks:marks,iced:iced};
         }
         return null;
      }

      // The Tab list's names' colours by place.
      private function rowColours() : Object
      {
         var colours:Object = {};
         for each(var list:* in [hud.player_list.players.team1list,hud.player_list.players.team2list])
         {
            list.validateNow();
            for(var i:int = 0; i < 20; i++)
            {
               var r:* = list.getRendererAt(i);
               if(r != null && r.data) colours[r.data.id] = r.ui_name.textField.textColor;
            }
         }
         return colours;
      }

      // What a map draws of a place's dot, its name left out, on the stage: null without a dot, an
      // empty rectangle for a dot that draws nothing.
      private function drawn(minimap:*, place:int) : Rectangle
      {
         var players:DisplayObjectContainer = minimap.minimap._players;
         for(var i:int = 0; i < players.numChildren; i++)
         {
            var icon:* = players.getChildAt(i);
            if(icon.index != place) continue;
            var shown:Rectangle = new Rectangle();
            for(var j:int = 0; j < icon.numChildren; j++)
            {
               var part:DisplayObject = icon.getChildAt(j);
               if(!(part is TextField) && part.visible) shown = shown.union(part.getBounds(stage));
            }
            return shown;
         }
         return null;
      }

      private static function empty(shown:Rectangle) : Boolean
      {
         return shown != null && (shown.width < 1 || shown.height < 1);
      }

      // The screen's colour at the middle of `shown` (the stage drawn as Ruffle shows it).
      private function colourAt(shown:Rectangle) : uint
      {
         var screen:BitmapData = new BitmapData(stage.stageWidth,stage.stageHeight,false,0);
         screen.draw(stage);
         return screen.getPixel(int(shown.x + shown.width / 2),int(shown.y + shown.height / 2));
      }

      private static function red(colour:uint) : Boolean
      {
         return (colour >> 16 & 0xFF) >= 180 && (colour >> 8 & 0xFF) <= 90 && (colour & 0xFF) <= 90;
      }

      // The revealed last fighter: a red mark at least 8 px on the Tab map, on the screen and red
      // there; on the corner minimap too while HUD.REVEALED_ON_MINIMAP (the one switch), red on the
      // screen near the local player (beyond its circle the minimap clips him, as every dot).
      private function revealedShown(what:String, near:Boolean) : void
      {
         var tab:* = hud.player_list.minimap;
         var mark:Rectangle = drawn(tab,LAST);
         var screen:Rectangle = new Rectangle(0,0,stage.stageWidth,stage.stageHeight);
         check(mark != null && mark.width >= 8 && mark.height >= 8 && screen.containsRect(mark),
            what + ": the revealed enemy marked on the Tab map, on the screen (" + mark + ")");
         var colour:uint = mark == null ? 0 : colourAt(mark);
         check(red(colour),what + ": the mark red on the screen (0x" + colour.toString(16) + ")");
         var corner:Rectangle = drawn(hud.minimap,LAST);
         if(!Object(hud).constructor.REVEALED_ON_MINIMAP)
         {
            check(empty(corner),what + ": the corner minimap switched off, nothing drawn for him");
            return;
         }
         check(corner != null && corner.width >= 8,what + ": marked on the corner minimap (" + corner + ")");
         if(near)
         {
            colour = colourAt(corner);
            check(red(colour),what + ": the corner minimap's mark red on the screen (0x" + colour.toString(16) + ")");
         }
      }

      // The hud's own child of a class (its private notices).
      private function childOf(name:String) : *
      {
         for(var i:int = 0; i < hud.numChildren; i++)
         {
            if(getQualifiedClassName(hud.getChildAt(i)) == name) return hud.getChildAt(i);
         }
         return null;
      }

      private function setLanguage(language:String) : void
      {
         loader.contentLoaderInfo.applicationDomain.getDefinition("survarium.data.LocalizationData")
            .setData(HarnessText.DATA[language]);
      }

      // The game's short message (set_parameterized_message), the reveal's popup: its text, and
      // whether it shows.
      private function popup() : String
      {
         return String(popupField().text).split("\r").join("\n");
      }

      private function popupField() : TextField
      {
         return hud.revealPopup.clip.mc_text.contextText;
      }

      private function popupShown() : Boolean
      {
         return hud.revealPopup.text != null && hud.revealPopup.clip.visible && hud.revealPopup.clip.parent != null;
      }

      // Freeze Tag's mode notice (set_game_type 12) and the reveal's popups for everyone
      // (freeze_tag_reveal, told each frame), in every language: a side that starts counting gives
      // one popup with its seconds left then, rounded; the frames after it change nothing (no
      // countdown); a side revealed gives "... revealed on the map".
      private function notices() : void
      {
         var mode:* = childOf("survarium.modes::ModeNotice");
         var screen:Rectangle = new Rectangle(0,0,stage.stageWidth,stage.stageHeight);
         for each(var language:String in HarnessText.LANGUAGES)
         {
            setLanguage(language);
            var data:Object = HarnessText.DATA[language];
            var text:Function = function(key:String, seconds:String = null):String {
               return seconds == null ? data[key] : String(data[key]).split("%1$s").join(seconds);
            };
            // As the game: reset, then patch 214's word (here: shown), then the mode and the time.
            var match:Function = function(word:* = true):void {
               hud.reset();
               if(word != null) hud.mode_notice(word);
               hud.set_game_type(12);
               hud.force_interface_hidden(false);
               hud.set_match_time("09:59");
            };
            match();
            check(mode != null && mode.visible && mode.label.text.split("\r").join("\n") == data["st_srp_hud_freeze_start"],
               language + ": Freeze Tag's notice under the timer, Blackout's unforce leaves it");
            check(mode.label.textHeight < 200 && screen.containsRect(mode.getBounds(stage)),language + ": the notice fits");
            // The word says no (the account's fourth match of the mode, a replay): no notice, the
            // time ticking on; a later word changes nothing until the next reset.
            match(false);
            hud.set_match_time("09:58");
            check(!mode.visible,language + ": told no, Freeze Tag's notice hidden");
            hud.mode_notice(true);
            hud.set_match_time("09:57");
            check(!mode.visible,language + ": the match's first word stays");
            // No word yet: the notice waits for it, then shows.
            match(null);
            check(!mode.visible,language + ": no word yet, no notice");
            hud.mode_notice(true);
            hud.set_match_time("09:58");
            check(mode.visible,language + ": the word comes, the notice shows");
            // Blackout's the same: told no, hidden; told yes, its own text.
            hud.reset();
            hud.mode_notice(false);
            hud.force_interface_hidden(true);
            hud.set_match_time("09:59");
            check(!mode.visible,language + ": told no, Blackout's notice hidden");
            hud.reset();
            hud.mode_notice(true);
            hud.force_interface_hidden(true);
            hud.set_match_time("09:59");
            check(mode.visible && mode.label.text.split("\r").join("\n") == data["st_srp_hud_blackout_start"],
               language + ": told yes, Blackout's notice");
            hud.force_interface_hidden(false);
            match();
            // The hunters: one popup, its seconds then; the next frames' times change nothing.
            hud.freeze_tag_reveal(0,0,3,14980);
            check(popupShown() && popup() == text("st_srp_hud_freeze_reveal_enemy","15"),
               language + ": the hunters' popup: " + popup());
            var shown:Rectangle = popupField().getBounds(stage);
            check(screen.containsRect(shown) && !shown.intersects(mode.label.getBounds(stage)),
               language + ": the popup on the screen, clear of the mode notice");
            hud.freeze_tag_reveal(0,0,3,13900);
            hud.freeze_tag_reveal(0,0,3,9000);
            check(popup() == text("st_srp_hud_freeze_reveal_enemy","15"),language + ": no countdown: " + popup());
            // A late joiner (or a resynced game): his real seconds, rounded.
            match();
            hud.freeze_tag_reveal(1,7400,0,0);
            check(popupShown() && popup() == text("st_srp_hud_freeze_reveal_self","7"),language + ": the last one's own popup, a late join: " + popup());
            match();
            hud.freeze_tag_reveal(2,14600,0,0);
            check(popup() == text("st_srp_hud_freeze_reveal_teammate","15"),language + ": his frozen teammates' popup");
            // Both sides down to one in the same frame: both lines, fitting.
            match();
            hud.freeze_tag_reveal(2,500,3,15000);
            check(popup() == text("st_srp_hud_freeze_reveal_teammate","1") + "\n" + text("st_srp_hud_freeze_reveal_enemy","15") &&
               popupField().numLines == 2 && screen.containsRect(popupField().getBounds(stage)),
               language + ": both sides at once: both lines, on the screen");
            // Revealed: one popup for the side revealed; the side still counting tells nothing new.
            hud.freeze_tag_reveal(2,0,3,14000);
            check(popupShown() && popup() == text("st_srp_hud_freeze_revealed_teammate"),language + ": your teammate is revealed");
            hud.freeze_tag_reveal(0,0,3,0);
            check(popup() == text("st_srp_hud_freeze_revealed_enemy"),language + ": the last enemy revealed");
            hud.freeze_tag_reveal(0,0,0,0);
            check(popup() == text("st_srp_hud_freeze_revealed_enemy"),language + ": nothing new after it");
            match();
            hud.freeze_tag_reveal(1,0,0,0);
            check(popup() == text("st_srp_hud_freeze_revealed_self"),language + ": you are revealed");
         }
         setLanguage("english");
         var english:Object = HarnessText.DATA["english"];
         // Another mode and a new match forget the sides: the next count is a popup again.
         hud.set_game_type(12);
         hud.freeze_tag_reveal(0,0,3,12000);
         hud.set_game_type(1);
         hud.set_game_type(12);
         hud.freeze_tag_reveal(0,0,3,9000);
         check(popup() == String(english.st_srp_hud_freeze_reveal_enemy).split("%1$s").join("9"),"another mode: the sides forgotten");
         hud.reset();
         hud.set_game_type(12);
         hud.freeze_tag_reveal(0,0,3,8000);
         check(popup() == String(english.st_srp_hud_freeze_reveal_enemy).split("%1$s").join("8"),"a new match: the sides forgotten");
         // The timeline (phase 4).
         hud.reset();
         hud.set_game_type(12);
         timelineTexts = {counting:String(english.st_srp_hud_freeze_reveal_enemy).split("%1$s").join("6"),
            revealed:english.st_srp_hud_freeze_revealed_enemy,welcome:"Free for all: first to 30 kills",
            verdict:"You won the match"};
         timelineStart = timelineNext = getTimer();
      }

      // The exe's frame at `t` ms of the timeline (patch 160's on_hud): the enemy team down to its
      // last one at 0, revealed at 6 s, told so for 3 s (REVEALED_NOTICE_MS) but for one frame at 8 s
      // that tells nobody (a thaw, then down to one and revealed again: the same popup told again);
      // the shared message's welcome at 0 and the end's verdict at 7.5 s (Free for All's, 6000 ms).
      private function exeFrame(t:int) : void
      {
         hud.freeze_tag_score(1,2,5);
         hud.freeze_tag_update(0,0,0,0,4000,0,0);
         hud.freeze_tag_rows(0);
         var revealed:Boolean = t >= 6000 && t < 9000 && !(t >= 8000 && t < 8040);
         hud.freeze_tag_revealed(revealed ? 1 << LAST : 0);
         hud.freeze_tag_reveal(0,0,t < 6000 ? 3 : revealed ? 3 : 0,t < 6000 ? 6000 - t : 0);
         if(t == 0) hud.set_parameterized_message(timelineTexts.welcome,33,20,6000);
         if(t == 7520) hud.set_parameterized_message(timelineTexts.verdict,33,20,6000);
      }

      // A message's alpha at `t` within [from, to): its lowest kept under `name` (and its text right).
      private function sample(name:String, t:int, from:int, to:int, clip:*, text:String) : void
      {
         if(t < from || t >= to) return;
         var shown:Boolean = clip.visible && clip.parent != null && String(clip.mc_text.contextText.text) == text;
         var alpha:Number = shown ? clip.alpha * clip.mc_text.alpha / 0.84 : 0;
         if(!(name in timelineLow) || alpha < timelineLow[name]) timelineLow[name] = alpha;
         if(clip.currentFrame != 15) timelineLow[name] = -clip.currentFrame;
      }

      // Whether a message was seen in [from, to) (its gone checks).
      private function seen(name:String, t:int, from:int, to:int, shown:Boolean) : void
      {
         if(t >= from && t < to && shown) timelineSeen[name] = true;
      }

      private function timeline() : void
      {
         var now:int = getTimer();
         while(int(now - timelineNext) >= 0)
         {
            exeFrame(timelineNext - timelineStart);
            timelineNext += 40;
         }
         var t:int = now - timelineStart;
         var reveal:* = hud.revealPopup.clip;
         var shared:* = hud.contextInfo3;
         // Fully shown between the fades: 0.5 s in, the hold, 0.5 s out (a frame's margin each side).
         sample("counting",t,600,3900,reveal,timelineTexts.counting);
         sample("revealed",t,6600,11900,reveal,timelineTexts.revealed);
         sample("welcome",t,600,5900,shared,timelineTexts.welcome);
         sample("verdict",t,8200,13400,shared,timelineTexts.verdict);
         seen("counting gone",t,4600,6000,hud.revealPopup.text != null || reveal.visible);
         seen("revealed gone",t,12600,14500,hud.revealPopup.text != null || reveal.visible);
         seen("welcome gone",t,6600,7520,shared.visible);
         seen("verdict gone",t,14100,14500,shared.visible);
         seen("shared off the display list",t,0,14500,shared.parent == null);
         if(t < 14500) return;
         for each(var name:String in ["counting","revealed","welcome","verdict"])
         {
            check(timelineLow[name] >= 0.95,"the timeline: " + name + " fully shown between its fades (lowest " +
               (timelineLow[name] < 0 ? "at frame " + -timelineLow[name] : timelineLow[name]) + ")");
         }
         for each(name in ["counting gone","revealed gone","welcome gone","verdict gone"])
         {
            check(!timelineSeen[name],"the timeline: " + name + " after its fade");
         }
         check(!timelineSeen["shared off the display list"],"the timeline: the shared message stays on the display list");
         trace("FREEZE RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
         phase = 3;
      }

      private function maps() : Array
      {
         return [{name:"the minimap",map:hud.minimap},{name:"the Tab map",map:hud.player_list.minimap}];
      }

      // Every map: each listed place's glyph frozen or as its own.
      private function dots(frozen:Array, what:String) : void
      {
         for each(var m:Object in maps())
         {
            var wrong:Array = [];
            for each(var place:int in [1,2,3,4])
            {
               var d:Object = dot(m.map,place);
               var own:uint = squad.indexOf(place) >= 0 ? SQUAD : 0;
               var want:uint = frozen.indexOf(place) >= 0 ? ICE : own;
               if(d == null || d.glyph != want || (want != 0 && !d.tinted)) wrong.push(place + "=" + (d ? d.glyph.toString(16) : "none"));
            }
            check(wrong.length == 0,what + " on " + m.name + (wrong.length ? ": " + wrong.join() : ""));
         }
      }

      private function tick(e:Event) : void
      {
         try
         {
            frames++;
            var m:Object;
            if(phase == 0)
            {
               if(loader.content == null || Object(loader.content).progress_indicator == null) return;
               hud = loader.content;
               ICE = (loader.contentLoaderInfo.applicationDomain.getDefinition("survarium.modes.FrozenPlaces") as Class).COLOR;
               trace("FREEZE hud colour 0x" + ICE.toString(16).toUpperCase());
               setLanguage("english");
               matchStart();
               dots([],"nobody frozen: every teammate's own colour");
               hud.freeze_tag_rows(frozen([0,1,3,4,10]));
               dots([1,3,4],"frozen teammates ice blue (a squad mate's green too), the others their own");
               for each(m in maps())
               {
                  var enemy:Object = dot(m.map,10);
                  check(enemy != null && !enemy.iced,"the frozen enemy's dot as it is on " + m.name);
                  check(dot(m.map,4).marks == dot(m.map,2).marks + 1,"the frozen carrier keeps his mark on " + m.name);
                  check(dot(m.map,0) == null || dot(m.map,0).glyph == -1,"the local player's own icon as it is on " + m.name);
               }
               var tab:* = hud.player_list.minimap;
               check(dot(tab,1).name == ICE && dot(tab,3).name == ICE && dot(tab,2).name == 0xFFFFFF,
                  "the Tab map's names: the frozen teammates' ice blue, the others white");
               check(dot(tab,10).name != ICE,"the Tab map's name of the frozen enemy as it is");
               // The enemy team's last one fighting (freeze-tag.md: on the other team's map at all
               // times): the exe lists him unspotted, and an enemy's icon draws nothing.
               check(empty(drawn(tab,LAST)) && empty(drawn(hud.minimap,LAST)),
                  "before he is told revealed: his dot listed and drawing nothing (no red mark)");
               var teammate:Rectangle = drawn(tab,1);
               // Revealed: his place, and a teammate's (the rule's mask is both teams'; never marked).
               hud.freeze_tag_revealed(1 << LAST | 1 << 1);
               revealedShown("revealed, far from the local player",false);
               check(drawn(tab,1).equals(teammate),"a teammate's place in the mask: his dot as it was");
               check(empty(drawn(tab,10)) && empty(drawn(hud.minimap,10)),
                  "an enemy listed and not revealed (spotted) as before: nothing drawn");
               check(dot(tab,LAST).name == 0xFF0000,"his name with Alt red, an enemy's");
               var label:Rectangle = null;
               var icon:DisplayObjectContainer = DisplayObjectContainer(tab.minimap._players);
               for(var k:int = 0; k < icon.numChildren; k++)
               {
                  var it:* = icon.getChildAt(k);
                  for(var n:int = 0; it.index == LAST && n < it.numChildren; n++)
                     if(it.getChildAt(n) is TextField) label = it.getChildAt(n).getBounds(stage);
               }
               check(label != null && label.bottom <= drawn(tab,LAST).top + 3,"his name over the mark");
               // Near the local player: marked the same.
               var near:Array = players();
               near[6] = player(LAST,1,-10,6);
               hud.update_players(near);
               revealedShown("revealed, near the local player",true);
               hud.update_players(players());
               // The screenshot: the mode notice and the hunters' popup.
               hud.set_match_time("09:45");
               hud.freeze_tag_reveal(0,0,3,12000);
               phase = 1;
               frames = 0;
               return;
            }
            if(phase == 1)
            {
               if(frames < 30) return;
               var rows:Object = rowColours();
               check(rows[1] == ICE && rows[3] == ICE && rows[4] == ICE && rows[10] == ICE,"the Tab list's frozen names ice blue");
               check(rows[2] != ICE && rows[11] != ICE,"the others' names their own");
               trace("FREEZE SHOT maps");
               phase = 2;
               frames = 0;
               return;
            }
            if(phase == 2)
            {
               if(frames < 150) return;
               // A thaw: 1 and 3; the squad mate 3 green again.
               hud.freeze_tag_rows(frozen([0,4,10]));
               dots([4],"thawed: 1 white and 3 green again, 4 still frozen");
               check(dot(hud.player_list.minimap,1).name == 0xFFFFFF,"thawed: the Tab map's name white again");
               // 2 joins the squad while frozen (its class changes: a new dot), and 5 enters frozen.
               var later:Array = players();
               later[2] = player(2,0,-8,22,true);
               squad = [2,3,4];
               later.push(player(5,0,10,30));
               hud.freeze_tag_rows(frozen([0,2,4,5,10]));
               hud.update_players(later);
               for each(m in maps())
               {
                  check(dot(m.map,2).glyph == ICE && dot(m.map,5).glyph == ICE,"a dot made while frozen: ice blue on " + m.name);
               }
               // The Tab list closed: its map held; a thaw while held is drawn when it shows again.
               hud.show_player_list(false);
               hud.freeze_tag_rows(frozen([0,5,10]));
               hud.update_players(later);
               check(dot(hud.minimap,2).glyph == SQUAD && dot(hud.minimap,4).glyph == SQUAD,"thawed on the minimap at once");
               hud.show_player_list(true);
               check(dot(hud.player_list.minimap,2).glyph == SQUAD && dot(hud.player_list.minimap,5).glyph == ICE,
                  "the Tab map shown again: drawn as the minimap");
               // A teammate of his thawed (or the round over): revealed nobody, his dot no more
               // listed; a round later he is revealed while the Tab list is closed: marked when it
               // shows again.
               hud.freeze_tag_revealed(0);
               var unlisted:Array = later.filter(function(p:Object, ...rest):Boolean { return p.player_id != LAST; });
               hud.update_players(unlisted);
               check(drawn(hud.player_list.minimap,LAST) == null,"not revealed (a teammate thawed, a team of two): no dot");
               hud.update_players(later);
               check(empty(drawn(hud.player_list.minimap,LAST)),"listed while not revealed (spotted): nothing drawn");
               hud.show_player_list(false);
               hud.freeze_tag_revealed(1 << LAST);
               hud.update_players(later);
               hud.show_player_list(true);
               revealedShown("revealed while the Tab list was closed, shown again",false);
               // The round's restart: nobody frozen.
               hud.freeze_tag_rows(0);
               dots([],"the round's restart: every own colour back");
               hud.freeze_tag_rows(frozen([1,3]));
               // Another mode: nothing frozen, the next calls of a match not of mode 12 none.
               hud.set_game_type(1);
               dots([],"another mode: every own colour back");
               check(empty(drawn(hud.player_list.minimap,LAST)),"another mode: nobody marked");
               rows = rowColours();
               check(rows[1] != ICE && rows[3] != ICE,"another mode: the Tab list's names their own");
               // A new match after a frozen one: no dot of the last match drawn again.
               hud.set_game_type(12);
               hud.freeze_tag_rows(frozen([1,3]));
               hud.freeze_tag_revealed(1 << LAST);
               hud.reset();
               squad = [3,4];
               for each(m in maps())
               {
                  check(dot(m.map,1) == null,"a new match: the last match's dots gone from " + m.name);
               }
               hud.show_player_list(true);
               hud.update_players(players());
               dots([],"a new match: nobody frozen");
               check(empty(drawn(hud.player_list.minimap,LAST)),"a new match: nobody marked");
               // A round's restart, Artifact Hunt's: its 3 s show the round's line; the fade goes
               // black; the countdown shows the kits and its time at once; an artefact offer still
               // waits under the kits.
               hud.freeze_tag_update(0,0,5,2200,4000,0,3000);
               check(hud.contextInfo.visible && hud.contextInfo.contextText.text.length > 0,
                  "the 3 s after a round show its line (the losing side's)");
               hud.freeze_tag_update(0,0,4,3000,4000,0,3000);
               check(hud.contextInfo.visible && hud.contextInfo.contextText.text.length > 0 && !hud.roundFade.visible,
                  "the 3 s after a round show its line (the winning side's), no fade");
               hud.freeze_tag_update(0,0,7,500,4000,0,1000);
               check(hud.roundFade.visible,"the fade goes black");
               hud.show_profiles_selector(true);
               hud.freeze_tag_update(0,0,8,4200,4000,0,5000);
               check(hud.contextInfo.visible && hud.contextInfo.contextText.text.length > 0,
                  "the countdown's time shown with the kits");
               hud.freeze_tag_update(0,0,0,0,4000,0);
               check(!hud.roundFade.visible,"the round in play: no fade");
               hud.show_artefact_offer("st_artefact_take_teammate");
               check(!hud.contextInfo.visible,"an artefact offer waits under the kits");
               hud.show_artefact_offer("");
               hud.show_profiles_selector(false);
               notices();
               phase = 4;
               return;
            }
            if(phase == 4)
            {
               timeline();
            }
         }
         catch(error:Error)
         {
            trace("FREEZE RESULT fail: " + error + " " + error.getStackTrace());
            phase = 3;
         }
      }
   }
}
