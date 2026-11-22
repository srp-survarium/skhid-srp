package
{
   import flash.display.*;
   import flash.events.*;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.net.URLRequest;
   import flash.system.*;
   import flash.utils.getQualifiedClassName;
   import flash.utils.getTimer;

   // Artifact Hunt's hud (docs/notes/artefact_hunt.md "HUD calls") with the hud movie's own root,
   // fed as the game feeds it: a match's start (set_game_type 13, the map, the players, the Tab
   // list), then root.artefact_hunt_update with made-up states, each one as the whole state. Checks
   // what each state shows (the bars, the row under them, the round's texts, the fade, the minimaps'
   // bases, artefact and carrier, the Tab list's carrier row), that a late join draws a state as a
   // game that saw everything before it does, that a call outside the contract changes nothing, and
   // that another mode after it shows none of it. Each state is held while check.sh takes its
   // screenshot ("HUNT SHOT <name>"). Ends with "HUNT RESULT pass" or "HUNT RESULT fail: ...".
   public class HuntHudHarness extends Sprite
   {
      private var loader:Loader = new Loader();
      private var hud:*;
      private var domain:ApplicationDomain;
      private var failures:Array = [];
      private var steps:Array = [];
      // getTimer() until which the state is held (re-sent each frame), 0 none.
      private var holding:int = 0;
      private var hold:Function = null;
      private var started:Boolean = false;

      // A shot's hold: ms the state stays (re-sent each frame, as the exe's time stood still), long
      // enough for check.sh's screenshot on a busy machine.
      private static const HOLD:int = 2500;

      // ms a state is drawn before its checks (the lists' and the layout's own invalidation).
      private static const SETTLE:int = 800;

      // The crosshair's spread the harness sets (frame 500 x 3 / 25.4 of its 500: its lines 48 px
      // from the centre, the screen's pixels).
      private static const STANDING_SPREAD:Number = 3;

      // How far the crosshair's top line reaches up at its widest (its last frame), in the screen's
      // pixels at every resolution (the crosshair is drawn 1:1).
      private var _widestReach:Number = 0;

      // The lowest bottom of the row under the bars over every state, in the hud's own units.
      private var _rowBottom:Number = 0;

      // level_01's frame on the minimaps: the bases, the artefact, the players (x, -z).
      private static const BASE_0:Array = [-60,20];
      private static const BASE_1:Array = [80,-60];
      private static const ARTEFACT:Array = [25,-5];

      public function HuntHudHarness()
      {
         loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
            function(e:UncaughtErrorEvent):void
            {
               e.preventDefault();
               trace("HUNT uncaught " + e.error + (e.error is Error ? " " + Error(e.error).getStackTrace() : ""));
            });
         // the hud's root lays itself out on the stage (its effects and kept layers beside it): put
         // on the stage itself, where the game has it
         loader.contentLoaderInfo.addEventListener(Event.INIT,function(e:Event):void { stage.addChild(loader.content); });
         loader.load(new URLRequest(loaderInfo.parameters.movie || "hud.swf"),new LoaderContext(false,new ApplicationDomain()));
         addEventListener(Event.ENTER_FRAME,tick);
      }

      private function check(ok:Boolean, what:String) : void
      {
         trace("HUNT " + (ok ? "ok " : "FAIL ") + what);
         if(!ok) failures.push(what);
      }

      // The 30 numbers of a state: TDM-like defaults, `o` overriding by name. A hunt out of sudden
      // death has a wave 9 s away unless `o` says otherwise, as the exe sends.
      private static function values(o:Object) : Array
      {
         var d:Object = {own:0,s0:1,s1:2,goal:5,phase:2,left:0,whole:0,after:0,clock:512000,clockWhole:900000,
            sudden:0,ownAlive:0,ownTotal:5,enemyAlive:0,enemyTotal:5,art:0,x:0,y:0,carrier:255,carrierTeam:2,me:0};
         for(var k:String in o) d[k] = o[k];
         var hunt:Boolean = d.phase >= 2 && d.phase <= 5 && d.sudden != 2;
         if(!("wave" in o)) d.wave = hunt ? 8600 : 0;
         if(!("waveWhole" in o)) d.waveWhole = d.wave > 0 ? 15000 : 0;
         return [d.own,d.s0,d.s1,d.goal,d.phase,d.left,d.whole,d.after,d.clock,d.clockWhole,d.sudden,
            d.ownAlive,d.ownTotal,d.enemyAlive,d.enemyTotal,d.art,d.x,d.y,d.carrier,d.carrierTeam,d.me,
            d.wave,d.waveWhole,2,0,BASE_0[0],BASE_0[1],1,BASE_1[0],BASE_1[1]];
      }

      private function send(o:Object) : void
      {
         hud.artefact_hunt_update.apply(null,values(o));
      }

      private static function player(id:int, team:uint, x:Number, y:Number, local:Boolean = false) : Object
      {
         return {player_id:id,team:team,type:1,player_pos_x:x,player_pos_y:y,player_rotation_in_rad:0,
            is_local_player:local,is_carrying_item:false,is_squad_player:false};
      }

      private static function row(id:int, name:String, ally:Boolean, kills:int) : Object
      {
         return {id:id,name:name,team:ally ? 1 : 2,kills:kills,deaths:2,rank:kills * 10,score:kills * 10,ping:40};
      }

      private function matchStart() : void
      {
         hud.set_localization_data(HuntText.DATA.english);
         hud.reset();
         hud.set_game_type(13);
         hud.set_minimap_props({map:"level_01"});
         hud.show_game_info(true);
         hud.show_crosshair(true);
         // the exe sets the aiming lines' spread every frame (update_current_player_weapon_stats); a
         // standing rifle's here, or the lines run through their 500 frames on their own
         hud.set_crosshair_size(STANDING_SPREAD);
         hud.set_local_player("Hunter");
         hud.update_local_player(-20,0,0.6,0);
         hud.update_players([player(0,0,-20,0,true),player(1,0,-35,12),player(2,0,5,-25),
            player(10,1,40,-30),player(11,1,55,-10),player(12,1,30,-55)]);
         hud.list_set_players([row(0,"Hunter",true,4),row(1,"[WLF] Wolf",true,6),row(2,"Lynx",true,1),
            row(10,"Raven",false,5),row(11,"[BOT] Marauder",false,3),row(12,"Fox",false,2)]);
         hud.set_match_time("08:32");
      }

      // The minimap's mode objects and the carrier's ring, as class names.
      private function minimapState(minimap:*) : Object
      {
         var objects:Array = [];
         var rings:Array = [];
         var clip:DisplayObjectContainer = minimap.minimap;
         var playerClass:Class = domain.getDefinition("survarium.controls.MinimapObjectPlayer") as Class;
         for(var i:int = 0; i < clip.numChildren; i++)
         {
            var name:String = getQualifiedClassName(clip.getChildAt(i));
            if(name == "Base" || name == "BaseHighlighted" || name == "BaseEnemy" || name.indexOf("ArtefactMinimapPoint") >= 0)
               objects.push(name.replace(/.*::/,""));
         }
         var players:DisplayObjectContainer = minimap.minimap._players;
         for(i = 0; i < players.numChildren; i++)
         {
            var icon:* = players.getChildAt(i);
            if(icon is playerClass && icon.carrying) rings.push(icon.index);
         }
         objects.sort();
         return {objects:objects.join(),rings:rings.join(),local:hasRing(minimap.minimap.player)};
      }

      private function hasRing(clip:DisplayObjectContainer) : Boolean
      {
         for(var i:int = 0; i < clip.numChildren; i++)
            if(getQualifiedClassName(clip.getChildAt(i)).indexOf("CarryRing") >= 0) return true;
         return false;
      }

      // The Tab list's marked rows' names.
      private function markedRows() : String
      {
         var names:Array = [];
         for each(var list:* in [hud.player_list.players.team1list,hud.player_list.players.team2list])
         {
            list.validateNow();
            for(var i:int = 0; i < 20; i++)
            {
               var r:* = list.getRendererAt(i);
               if(r != null && r.carrierMarked && r.data) names.push(r.data.name);
            }
         }
         return names.join();
      }

      private function rowTexts() : Object
      {
         return hud.huntRow.texts;
      }

      private function bars() : String
      {
         var p:* = hud.progress_indicator;
         return (p.team_1_score.visible ? p.team_1_score.value : "-") + ":" + (p.team_2_score.visible ? p.team_2_score.value : "-")
            + (p.team1LastPlayers.visible ? " heads " + p.team1LastPlayers.value + "/" + p.team1LastPlayers.maximum
               + " " + p.team2LastPlayers.value + "/" + p.team2LastPlayers.maximum : "");
      }

      // The fade on the stage.
      private function fade() : Number
      {
         var f:* = hud.roundFade;
         return f.visible ? f.alpha : 0;
      }

      // A state sent and held for a shot, its checks after a frame.
      private function shot(name:String, state:Object, checks:Function, also:Function = null) : void
      {
         steps.push(function():void
         {
            send(state);
            if(also != null) also();
            hold = function():void { send(state); };
            holding = getTimer() + SETTLE;
            steps.unshift(function():void
            {
               checks();
               clearOfCrosshair(name);
               trace("HUNT SHOT " + name);
               holding = getTimer() + HOLD;
            });
         });
      }

      private function plan() : void
      {
         var english:Object = HuntText.DATA.english;
         shot("waiting",{phase:2,wave:11600},function():void
         {
            check(bars() == "1:2","own team (0) first in the bars, its points 1 against 2: " + bars());
            check(rowTexts().caption == "" && rowTexts().banner == "" && hud.huntRow.visible,"waiting: the row's grey orb, no text");
            check(rowTexts().wave == "12","waiting: the next wave 12 s away beside the match time: " + rowTexts().wave);
            checkWaveBesideClock("waiting");
            var m:Object = minimapState(hud.minimap);
            check(m.objects == "Base,BaseEnemy" && m.rings == "","waiting: both bases on the minimap, nothing else: " + m.objects);
            check(fade() == 0,"no fade in play");
         });
         shot("lying",{phase:3,left:84000,whole:120000,art:1,x:ARTEFACT[0],y:ARTEFACT[1]},function():void
         {
            check(rowTexts().caption == "1:24","lying: its time left under the orb: " + rowTexts().caption);
            check(minimapState(hud.minimap).objects == "ArtefactMinimapPoint,Base,BaseEnemy","lying: the artefact on the minimap");
            check(minimapState(hud.player_list.minimap).objects == "ArtefactMinimapPoint,Base,BaseEnemy","lying: and on the Tab map");
         });
         shot("tab-ally-carrier",{phase:4,art:2,carrier:1,carrierTeam:0},function():void
         {
            check(rowTexts().caption == "[WLF] Wolf","carried by a teammate: his name under the orb: " + rowTexts().caption);
            check(minimapState(hud.minimap).objects == "Base,BaseEnemy","carried: no artefact on the map");
            check(minimapState(hud.minimap).rings == "1" && minimapState(hud.player_list.minimap).rings == "1","the carrier's dot ringed on both maps");
            check(markedRows() == "[WLF] Wolf","his row marked in the Tab list: " + markedRows());
         },function():void { hud.show_player_list(true); });
         shot("enemy-carrier",{phase:4,art:2,carrier:11,carrierTeam:1},function():void
         {
            check(rowTexts().caption == "[BOT] Marauder","carried by an enemy: his name: " + rowTexts().caption);
            check(minimapState(hud.minimap).rings == "11","the enemy carrier's dot ringed");
            check(markedRows() == "[BOT] Marauder","his row marked, the teammate's no more: " + markedRows());
         },function():void { hud.show_player_list(false); });
         shot("you-carry",{phase:4,art:2,carrier:0,carrierTeam:0,me:1},function():void
         {
            check(rowTexts().caption == english.st_srp_hud_artefact_hunt_carrying && hud.huntRow.carryScale > 1,
               "the viewer carries: \"" + rowTexts().caption + "\", the orb larger");
            var m:Object = minimapState(hud.minimap);
            check(m.objects == "BaseEnemy,BaseHighlighted","own base highlighted while he carries: " + m.objects);
            check(m.local,"his own arrow ringed");
            check(hud.contextInfo.visible && hud.contextInfo.contextText.text == english.st_put_item,
               "the put prompt: " + hud.contextInfo.contextText.text);
         },function():void { hud.show_artefact_offer("st_put_item"); });
         shot("restart",{phase:6,left:2400,whole:3000,after:2,s0:2},function():void
         {
            check(rowTexts().banner == english.st_round_restart_in + " 00:03","round restarts: " + rowTexts().banner);
            check(rowTexts().wave == "","no wave in a round's restart: " + rowTexts().wave);
            check(bars() == "2:2","the delivery's point: " + bars());
            check(minimapState(hud.minimap).objects == "Base,BaseEnemy" && minimapState(hud.minimap).local == false,
               "nothing carried: the own base plain again");
         },function():void
         {
            hud.show_artefact_offer("");
            hud.set_parameterized_message(english.st_artefact_hunt_team_delivered,33,20,4000);
         });
         shot("fade",{phase:7,left:400,whole:1000,s0:2},function():void
         {
            check(Math.abs(fade() - 0.648) < 0.01,"the fade 600 ms in: " + fade());
            check(rowTexts().banner == "","no banner in the fade");
         });
         shot("countdown-start",{phase:8,left:4800,whole:5000,s0:2},function():void
         {
            check(fade() > 0.6 && fade() < 0.9,"the countdown's first 200 ms: coming back from black: " + fade());
         });
         shot("countdown",{phase:8,left:4200,whole:5000,s0:2},function():void
         {
            check(fade() == 0,"back from black after 500 ms");
            check(rowTexts().banner == english.st_final_countdown + " 00:05","the round's countdown: " + rowTexts().banner);
         });
         shot("sudden-coming",{phase:6,left:2000,whole:3000,after:2,sudden:1,s0:2,s1:2,clock:180000,clockWhole:180000},function():void
         {
            check(rowTexts().sudden == english.st_srp_hud_artefact_hunt_sudden_death
               && rowTexts().banner == english.st_round_restart_in + " 00:02","sudden death with the coming round: " + rowTexts().sudden + " / " + rowTexts().banner);
            check(bars() == "2:2","its bars until it starts: " + bars());
         },function():void { hud.set_match_time("03:00"); });
         shot("sudden",{phase:3,left:100000,whole:120000,art:1,x:ARTEFACT[0],y:ARTEFACT[1],sudden:2,s0:2,s1:2,
            clock:143000,clockWhole:180000,ownAlive:3,ownTotal:5,enemyAlive:2,enemyTotal:5},function():void
         {
            check(bars() == "-:- heads 3/5 2/5","sudden death: the players still in, in the bars' place: " + bars());
            check(rowTexts().sudden == english.st_srp_hud_artefact_hunt_sudden_death,"\"Sudden death\" under the orb");
            check(rowTexts().wave == "","no wave in sudden death: " + rowTexts().wave);
         },function():void
         {
            hud.set_match_time("02:23");
            hud.set_respawn_time("");
         });
         shot("respawn",{phase:3,left:70000,whole:120000,art:1,x:ARTEFACT[0],y:ARTEFACT[1],wave:2400},function():void
         {
            var orb:Rectangle = hud.huntRow.getBounds(hud);
            check(bars() == "1:2","out of sudden death: the bars again: " + bars());
            check(rowTexts().wave == "3","dead: the wave 3 s away beside the match time, his own line the game's: " + rowTexts().wave);
            check(true,"the respawn countdown under the row: " + orb.bottom);
         },function():void
         {
            hud.set_match_time("06:40");
            hud.set_respawn_time("Respawn in 00:03");
         });
         steps.push(lateJoinAndRefusals);
         shot("other-languages",{phase:6,left:2000,whole:3000,after:2,sudden:1,s0:2,s1:2},function():void
         {
            for each(var language:String in HuntText.LANGUAGES)
            {
               hud.set_localization_data(HuntText.DATA[language]);
               hud.huntRow.tick(getTimer());
               var width:Number = hud.huntRow.getBounds(hud).width;
               check(width < 560,language + ": the round's texts fit (" + Math.round(width) + " px): " + rowTexts().banner);
            }
            hud.set_localization_data(HuntText.DATA.russian);
         });
         steps.push(crosshairReach);
         steps.push(otherMode);
      }

      // A game that joins late (a new hud, the match's start, then one update) draws the same as
      // the one that saw everything; a call outside the contract changes nothing.
      private function lateJoinAndRefusals() : void
      {
         hud.set_respawn_time("");
         var state:Object = {phase:4,art:2,carrier:11,carrierTeam:1};
         send(state);
         var seen:String = describe();
         matchStart();
         check(describe() != seen,"a new match's start shows none of it");
         send(state);
         check(describe() == seen,"a late join draws the state as a game that saw it all: " + describe());
         var before:String = describe();
         var bad:Array = values({phase:9});
         hud.artefact_hunt_update.apply(null,bad);
         hud.artefact_hunt_update.apply(null,values({}).slice(0,23));
         hud.artefact_hunt_update.apply(null,values({wave:16000,waveWhole:15000}));
         hud.artefact_hunt_update.apply(null,values({wave:3000,waveWhole:0}));
         hud.artefact_hunt_update.apply(null,values({art:2,carrier:255}));
         hud.artefact_hunt_update.apply(null,values({phase:4,art:2,carrier:3,carrierTeam:0,me:1,s0:6}));
         check(describe() == before,"calls outside the contract refused, nothing changed");
      }

      private function describe() : String
      {
         var m:Object = minimapState(hud.minimap);
         var t:Object = rowTexts();
         return [bars(),hud.huntRow.visible,t.wave,t.caption,t.sudden,t.banner,m.objects,m.rings,markedRows(),fade()].join("|");
      }

      // The wave right after the match time, on its line, clear of both bars.
      private function checkWaveBesideClock(name:String) : void
      {
         var p:* = hud.progress_indicator;
         var clock:Rectangle = p.match_time_2_tdm.textField.getBounds(stage);
         clock.inflate(-(clock.width - p.match_time_2_tdm.textField.textWidth * clock.width / p.match_time_2_tdm.textField.width) / 2 + 2,0);
         // the glyphs, without the text field's 2 px gutters (the hud's scale)
         var wave:Rectangle = hud.huntRow.wave.shownBounds(stage);
         wave.inflate(-2 * hud.scaleX,0);
         var right:Rectangle = p.team_2_score.getBounds(stage);
         var left:Rectangle = p.team_1_score.getBounds(stage);
         check(wave.left >= clock.right - 2 && wave.right <= right.left && clock.left >= left.right - 1
            && wave.top >= clock.top && wave.top < clock.bottom,
            name + ": the wave after the match time, between the bars (clock " + clock + ", wave " + wave
            + ", bars to " + left.right + " and from " + right.left + ")");
         var bars:Number = (left.left + right.right) / 2;
         var group:Number = (clock.left + wave.right) / 2;
         check(Math.abs(bars - group) <= 3,name + ": the time and the wave centred between the bars: " + group + " / " + bars);
      }

      // The crosshair's top line is on the row's centre line: as Research's units and the native
      // countdowns there, the row must stay clear of it at the spread the game draws.
      private function clearOfCrosshair(name:String) : void
      {
         var row:* = hud.huntRow;
         if(!row.visible) return;
         var bottom:Number = row.localToGlobal(new Point(0,row.contentBottom)).y;
         _rowBottom = Math.max(_rowBottom,bottom / hud.scaleY);
         var line:Number = hud.crosshair.crosshair_mc.getBounds(stage).top;
         check(line > bottom,name + ": the row (to " + Math.round(bottom) + " px) clear of the crosshair's line (from " + Math.round(line) + " px)");
      }

      // Where the crosshair's line meets the row at its widest, at 1280x720 and 1920x1080.
      private function crosshairReach() : void
      {
         var mc:* = hud.crosshair.crosshair_mc;
         var frame:int = mc.currentFrame;
         var reach:Array = [];
         for(var f:int = 1; f <= mc.totalFrames; f++)
         {
            mc.gotoAndStop(f);
            reach.push(-mc.getBounds(hud.crosshair).top);
         }
         mc.gotoAndStop(frame);
         _widestReach = reach[reach.length - 1];
         for each(var height:int in [720,1080])
         {
            var bottom:Number = _rowBottom * height / 1080;
            var from:int = 0;
            for(f = 0; f < reach.length && from == 0; f++)
               if(height / 2 - reach[f] <= bottom) from = f + 1;
            trace("HUNT note: at " + (height * 16 / 9) + "x" + height + " the row ends at " + Math.round(bottom)
               + " px; the crosshair's line reaches " + (from ? "it from frame " + from + " of " + reach.length
               + " (" + Math.round(100 * from / reach.length) + " % of its widest)" : "it at no spread (its widest: "
               + Math.round(height / 2 - _widestReach) + " px)"));
         }
      }

      private function otherMode() : void
      {
         trace("HUNT another mode: reset");
         hud.reset();
         trace("HUNT another mode: Team Deathmatch");
         hud.set_game_type(4);
         hud.set_minimap_props({map:"level_01"});
         hud.update_local_player(-20,0,0.6,0);
         hud.update_players([player(0,0,-20,0,true),player(11,1,55,-10)]);
         trace("HUNT another mode: checks");
         var m:Object = minimapState(hud.minimap);
         check(!hud.huntRow.visible && m.objects == "" && m.rings == "" && fade() == 0 && markedRows() == "",
            "Team Deathmatch after it: nothing of the mode (" + describe() + ")");
         hud.set_tickets_required(150);
         hud.set_game_progress(150,120);
         check(hud.progress_indicator.team_1_score.value == 150,"Team Deathmatch's own bars as before");
         trace("HUNT RESULT " + (failures.length ? "fail: " + failures.join("; ") : "pass"));
         steps = [];
      }

      private function tick(e:Event) : void
      {
         try
         {
            if(!started)
            {
               if(loader.content == null || Object(loader.content).progress_indicator == null) return;
               started = true;
               hud = loader.content;
               domain = loader.contentLoaderInfo.applicationDomain;
               trace("HUNT pending texts (the dictionaries lack them yet): " + HuntText.PENDING.join(", "));
               matchStart();
               plan();
               return;
            }
            if(getTimer() < holding)
            {
               if(hold != null) hold();
               return;
            }
            if(steps.length) (steps.shift() as Function)();
         }
         catch(error:Error)
         {
            trace("HUNT RESULT fail: " + error + " " + error.getStackTrace());
            removeEventListener(Event.ENTER_FRAME,tick);
         }
      }
   }
}
