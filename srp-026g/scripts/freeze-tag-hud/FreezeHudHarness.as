package
{
   import flash.display.*;
   import flash.events.*;
   import flash.net.URLRequest;
   import flash.system.*;
   import flash.text.TextField;

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
   // - the round's restart (mask 0), another mode and a new match leave nothing blue.
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
            player(4,0,2,-18,true,true),player(10,1,30,4)];
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
               // The round's restart: nobody frozen.
               hud.freeze_tag_rows(0);
               dots([],"the round's restart: every own colour back");
               hud.freeze_tag_rows(frozen([1,3]));
               // Another mode: nothing frozen, the next calls of a match not of mode 12 none.
               hud.set_game_type(1);
               dots([],"another mode: every own colour back");
               rows = rowColours();
               check(rows[1] != ICE && rows[3] != ICE,"another mode: the Tab list's names their own");
               // A new match after a frozen one: no dot of the last match drawn again.
               hud.set_game_type(12);
               hud.freeze_tag_rows(frozen([1,3]));
               hud.reset();
               squad = [3,4];
               for each(m in maps())
               {
                  check(dot(m.map,1) == null,"a new match: the last match's dots gone from " + m.name);
               }
               hud.show_player_list(true);
               hud.update_players(players());
               dots([],"a new match: nobody frozen");
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
               trace("FREEZE RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
               phase = 3;
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
