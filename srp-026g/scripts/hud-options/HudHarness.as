package
{
    import flash.display.Loader;
    import flash.display.MovieClip;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.UncaughtErrorEvent;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;

    // The match hud's hit marker list and "Hide player scores" (exe patch 83,
    // docs/notes/hide-hit-markers-scores.md) with the hud movie's own classes,
    // through HUD.srp_hud_options with real controls supplied to the root
    // (its game-only initialization cannot finish here): hitMarkers and the Tab list's
    // (SrvPlayerList.players) namesOnly.
    // - the hit marker (root.crosshair_enemy_hit: Crosshair.setDamage) shown
    //   with All; Hidden: none, a shown one (a kill's too) cleared; All again:
    //   shown; Kill marker only: a hit shows nothing, a kill its mark (its last
    //   bar part on its kill frame), a hit's marker shown cleared at the pick;
    //   Hidden crosshair preserves hit/kill feedback and survives game updates;
    // - the Tab list off: each team by score as the game sorts it, every
    //   column shown; on: each team (the board's two lists, as it groups them)
    //   by clan tag, then name (the clanless after the clans, the bots' "BOT" last), case-
    //   insensitive, equal names by id, its score, kills, deaths and rank not shown,
    //   kept so through a player's update and the local player's name; off
    //   again: as before.
    // Shows a kill's mark with Kill marker only, then the list with scores,
    // then names only, for check.sh's
    // screenshots. Prints "HUD ..." lines and "HUD RESULT pass" or "HUD RESULT
    // fail: ...". No game.
    public class HudHarness extends Sprite
    {
        private var loader:Loader = new Loader();
        private var frames:int = 0;
        private var crosshair:MovieClip;
        private var hud:MovieClip;
        private var board:MovieClip;
        private var list:*;

        // Two teams, clan tags and names in mixed case. Names only, the allies: the clans by tag
        // (Axe, then wolf/Wolf as one), then those without a clan (Bob before bob: by id), then the bots
        // ([BOT]), each by name; case-sensitive would differ. By score: Dave, Abe, alice, carol, amy,
        // bob, Bob, zed.
        private static const PLAYERS:Array = [
            {id: 1, name: "[wolf] carol", team: 1, kills: 3, deaths: 2, score: 30, rank: 12, squad_id: 0, squad_state: 0},
            {id: 2, name: "Bob", team: 1, kills: 1, deaths: 5, score: 10, rank: 3, squad_id: 0, squad_state: 0},
            {id: 3, name: "[BOT] Dave", team: 1, kills: 9, deaths: 1, score: 90, rank: 20, squad_id: 0, squad_state: 0},
            {id: 4, name: "[Axe] alice", team: 1, kills: 5, deaths: 3, score: 50, rank: 7, squad_id: 0, squad_state: 0},
            {id: 8, name: "[Wolf] Abe", team: 1, kills: 6, deaths: 2, score: 60, rank: 8, squad_id: 0, squad_state: 0},
            {id: 9, name: "zed", team: 1, kills: 0, deaths: 6, score: 5, rank: 2, squad_id: 0, squad_state: 0},
            {id: 10, name: "[BOT] amy", team: 1, kills: 2, deaths: 3, score: 15, rank: 4, squad_id: 0, squad_state: 0},
            {id: 11, name: "bob", team: 1, kills: 1, deaths: 1, score: 12, rank: 5, squad_id: 0, squad_state: 0},
            {id: 5, name: "Yuri", team: 2, kills: 2, deaths: 4, score: 20, rank: 9, squad_id: 0, squad_state: 0},
            {id: 6, name: "xena", team: 2, kills: 7, deaths: 2, score: 70, rank: 15, squad_id: 0, squad_state: 0},
            {id: 7, name: "[cat] Zoe", team: 2, kills: 4, deaths: 4, score: 40, rank: 11, squad_id: 0, squad_state: 0}];

        public function HudHarness()
        {
            loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
            loader.contentLoaderInfo.addEventListener("ioError", function(e:Event):void {
                trace("HUD RESULT fail: " + e);
            });
            // The hud's own root fails outside the game (no exe): only its classes are used.
            loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
                function(e:UncaughtErrorEvent):void { e.preventDefault(); });
            loader.load(new URLRequest("hud.swf"), new LoaderContext(false, new ApplicationDomain()));
        }

        private function loaded(e:Event):void
        {
            var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
            // a dark scene behind the hud, as a match has
            graphics.beginFill(0x2F3431);
            graphics.drawRect(0, 0, 1280, 720);
            graphics.endFill();
            crosshair = new (domain.getDefinition("SrvCrosshair") as Class)() as MovieClip;
            crosshair.x = 1100;
            crosshair.y = 600;
            addChild(crosshair);
            // the Tab list, scaled to the window
            board = new (domain.getDefinition("SrvPlayerList") as Class)() as MovieClip;
            board.scaleX = board.scaleY = 0.9;
            board.x = 20 - 101 * 0.9;
            board.y = 20 - 264 * 0.9;
            addChild(board);
            list = board.players;
            // The loaded root's game-only initialization is incomplete in Ruffle.
            // Supply its real controls so the public HUD methods are exercised too.
            hud = loader.content as MovieClip;
            addChild(loader);
            loader.visible = false;
            hud.crosshair = crosshair;
            hud.player_list = board;
            addEventListener(Event.ENTER_FRAME, tick);
        }

        private function require(value:Boolean, message:String):void
        {
            if (!value) throw new Error(message);
        }

        private function tick(e:Event):void
        {
            ++frames;
            try
            {
                if (frames == 10) hitMarker();
                if (frames == 40) mapNames();
                if (frames == 41) scores();
                if (frames == 70) trace("HUD SHOT scores");
                if (frames == 130) namesOnly();
                if (frames == 160)
                {
                    trace("HUD SHOT names");
                    trace("HUD RESULT pass");
                    removeEventListener(Event.ENTER_FRAME, tick);
                }
            }
            catch (error:Error)
            {
                trace("HUD RESULT fail: " + error.message);
                removeEventListener(Event.ENTER_FRAME, tick);
            }
        }

        // The hit marker's four bars: shown when past their first (empty) frame.
        private function markerShown():Boolean
        {
            var bars:* = crosshair.bars.barsContainer;
            return bars.damage1.currentFrame > 1 && bars.damage2.currentFrame > 1
                && bars.damage3.currentFrame > 1 && bars.damage4.currentFrame > 1;
        }

        // A kill's mark: each bar's last part shown on its kill frame (Crosshair.setDamage).
        private function killShown():Boolean
        {
            var bars:* = crosshair.bars.barsContainer;
            if (!markerShown()) return false;
            var last:String = "part_" + String(bars.damage1.currentFrame - 1);
            return bars.damage1[last].currentFrame == 2 && bars.damage2[last].currentFrame == 2
                && bars.damage3[last].currentFrame == 2 && bars.damage4[last].currentFrame == 2;
        }

        // The kill's white hit parts (each bar's parts under its red last one): all `shown` or all not;
        // false without such parts.
        private function hitParts(shown:Boolean):Boolean
        {
            var bars:* = crosshair.bars.barsContainer;
            var last:int = bars.damage1.currentFrame - 1;
            for each (var bar:* in [bars.damage1, bars.damage2, bars.damage3, bars.damage4])
            {
                for (var k:int = 1; k < last; ++k)
                {
                    if (bar["part_" + k].visible != shown) return false;
                }
            }
            return last > 1;
        }

        private function hitMarker():void
        {
            var ALL:uint = 0, KILL_ONLY:uint = 1, HIDDEN:uint = 2;
            require(crosshair.hitMarkers == ALL && !markerShown(), "All by default, nothing shown yet");
            crosshair.setDamage(0.8, 1000, false, false);
            require(markerShown() && !killShown(), "All: a hit on an enemy shows the hit marker");
            crosshair.hitMarkers = HIDDEN;
            require(!markerShown(), "Hidden: the marker shown is cleared");
            crosshair.setDamage(0.8, 1000, false, false);
            require(!markerShown(), "Hidden: a hit shows no marker");
            crosshair.setDamage(1, 1000, true, false);
            require(!markerShown(), "Hidden: a kill shows no marker");
            crosshair.hitMarkers = ALL;
            crosshair.setDamage(1, 1000, true, false);
            require(killShown() && hitParts(true), "All again: a kill shows its mark over the hit's white parts");
            crosshair.hitMarkers = HIDDEN;
            require(!markerShown(), "Hidden: a kill's marker is cleared too");
            crosshair.setDamage(0.5, 1000, false, false);
            require(!markerShown(), "Hidden after a kill's marker: a hit shows nothing");
            crosshair.hitMarkers = ALL;
            crosshair.setDamage(0.5, 1000, false, false);
            require(markerShown(), "All: hits shown again");
            crosshair.hitMarkers = KILL_ONLY;
            require(!markerShown(), "Kill marker only: the hit's marker shown is cleared");
            crosshair.setDamage(0.8, 1000, false, false);
            require(!markerShown(), "Kill marker only: a hit shows nothing");
            crosshair.setDamage(1, 1000, true, false);
            require(killShown() && hitParts(false), "Kill marker only: a kill shows its red mark alone");
            crosshair.setDamage(0.3, 1000, false, false);
            require(killShown(), "Kill marker only: a hit after it leaves the kill's mark");
            crosshair.hitMarkers = 7;
            require(crosshair.hitMarkers == ALL && !markerShown(), "another number: All, the mark shown cleared");
            crosshair.hitMarkers = ALL;
            crosshair.setDamage(1, 1000, true, false);
            require(killShown() && hitParts(true), "All after Kill marker only: a kill's white parts shown again");
            crosshair.hitMarkers = HIDDEN;
            crosshair.hitMarkers = ALL;
            // Hidden crosshair affects only aiming lines. Weapon updates cannot restore
            // them, and the separate hit-marker choice works in either crosshair mode.
            hud.show_crosshair(true);
            require(crosshair.crosshair_mc.visible, "normal crosshair shows aiming lines");
            hud.srp_hud_options(ALL, 0, 1, 0);
            hud.crosshair_enemy_hit(0.5);
            require(!crosshair.crosshair_mc.visible && crosshair.visible && crosshair.bars.visible && markerShown(),
                "hidden crosshair keeps hit feedback visible");
            hud.show_crosshair(true);
            hud.set_crosshair_size(15);
            require(!crosshair.crosshair_mc.visible, "weapon and spread updates keep aiming lines hidden");
            hud.crosshair_enemy_hit(1, 1000, true);
            require(markerShown() && crosshair.visible && !crosshair.crosshair_mc.visible,
                "hidden crosshair keeps kill feedback visible");
            hud.srp_hud_options(HIDDEN, 0, 1, 0);
            require(!markerShown(), "separate hit-marker option clears feedback with a hidden crosshair");
            crosshair.setDamage(0.5, 1000, false, false);
            require(!markerShown(), "both hidden options suppress both visuals");
            hud.srp_hud_options(ALL, 0, 1, 0);
            crosshair.setDamage(0.5, 1000, false, false);
            require(markerShown() && !crosshair.crosshair_mc.visible, "hit feedback can return independently");
            hud.show_crosshair(false);
            hud.srp_hud_options(ALL, 0, 0, 0);
            require(!crosshair.crosshair_mc.visible, "unhiding in ADS or on death respects the game's hidden state");
            hud.show_crosshair(true);
            require(crosshair.crosshair_mc.visible && markerShown(), "leaving ADS restores normal crosshair and feedback");
            hud.show_crosshair(false);
            hud.srp_hud_options(ALL, 0, 1, 0);
            hud.show_crosshair(true);
            require(!crosshair.crosshair_mc.visible, "respawn cannot override saved Hidden");
            hud.srp_hud_options(ALL, 0, 0, 0);
            require(crosshair.crosshair_mc.visible, "turning Hidden off restores the latest game request");
            trace("HUD hit marker: independent of Hidden crosshair, including kills, weapon updates, aim/death and respawn");
            // check.sh's kill-marker.png: a kill with Kill marker only, its mark kept a while
            hud.srp_hud_options(KILL_ONLY, 0, 0, 0);
            hud.crosshair_enemy_hit(0.6, 3000, true);
            require(killShown(), "srp_hud_options: Kill marker only, a kill shows its mark");
            trace("HUD SHOT kill");
        }

        private function names(team:*):Array
        {
            var out:Array = [];
            for (var i:int = 0; i < team.dataProvider.length; ++i) out.push(team.dataProvider[i].name);
            return out;
        }

        // A list or row drawn now. A plain row's background takes a null colorTransform
        // (PlayerListRenderer.updateData), which Scaleform takes and Ruffle refuses (#2007) after
        // the row's texts and columns are set: that refusal is not the row's.
        private function draw(control:*):void
        {
            try
            {
                control.validateNow();
            }
            catch (error:Error)
            {
                if (error.errorID != 2007) throw error;
            }
        }

        // Each drawn row of a team: [name, the stats shown].
        private function rows(team:*):Array
        {
            draw(team);
            var out:Array = [];
            for (var i:int = 0; i < team.dataProvider.length; ++i)
            {
                var row:* = team.getRendererAt(i);
                require(row != null, "a row for each player of " + team.name);
                draw(row);
                var stats:Boolean = row.ui_kills.visible && row.ui_deaths.visible && row.ui_rank.visible
                    && (row.ui_score == null || row.ui_score.visible);
                var none:Boolean = !row.ui_kills.visible && !row.ui_deaths.visible && !row.ui_rank.visible
                    && (row.ui_score == null || !row.ui_score.visible);
                require(stats || none, "a row's columns shown or hidden together");
                out.push([row.ui_name.text, stats]);
            }
            return out;
        }

        // The columns' headers (the icons over each list): the board's children but its team names and lists.
        private function headersShown():Array
        {
            var out:Array = [];
            for (var i:int = 0; i < list.numChildren; ++i)
            {
                var child:* = list.getChildAt(i);
                if (child != list.team1 && child != list.team2 && child != list.team1list && child != list.team2list)
                    out.push(child.visible);
            }
            return out;
        }

        private function expectTeams(allies:Array, enemies:Array, stats:Boolean, what:String):void
        {
            var headers:Array = headersShown();
            require(headers.length == 2, what + ": the two lists' headers, " + headers.length);
            require(headers[0] == stats && headers[1] == stats, what + ": the headers " + (stats ? "shown" : "hidden"));
            require(names(list.team1list).join() == allies.join(), what + ": the allies " + names(list.team1list));
            require(names(list.team2list).join() == enemies.join(), what + ": the enemies " + names(list.team2list));
            for each (var team:* in [list.team1list, list.team2list])
            {
                var drawn:Array = rows(team);
                var order:Array = team == list.team1list ? allies : enemies;
                for (var i:int = 0; i < drawn.length; ++i)
                {
                    require(drawn[i][0] == order[i], what + ": row " + i + " of " + team.name + " is " + drawn[i][0]);
                    require(drawn[i][1] == stats, what + ": " + drawn[i][0] + "'s score, kills, deaths and rank "
                        + (stats ? "shown" : "hidden"));
                }
            }
            trace("HUD list " + what + ": " + allies.join(", ") + " | " + enemies.join(", ") + (stats ? ", stats shown" : ", names only"));
        }

        private function copy():Array
        {
            var out:Array = [];
            for each (var player:Object in PLAYERS)
            {
                var row:Object = {};
                for (var key:String in player) row[key] = player[key];
                out.push(row);
            }
            return out;
        }

        // The Tab map's names: Alt shows them; with "Names on map" they show and Alt hides them.
        private function mapNames():void
        {
            var map:* = board.minimap;
            require(!map.namesShown, "map names: hidden by default");
            hud.show_map_names(true);
            require(map.namesShown, "map names: Alt shows them");
            hud.show_map_names(false);
            require(!map.namesShown, "map names: Alt let go hides them");
            hud.srp_hud_options(0, 0, 0, 1);
            require(map.namesShown, "Names on map: shown without Alt");
            hud.show_map_names(true);
            require(!map.namesShown, "Names on map: Alt hides them");
            hud.show_map_names(false);
            require(map.namesShown, "Names on map: Alt let go shows them again");
            hud.srp_hud_options(0, 0, 0, 0);
            require(!map.namesShown, "Names on map off: hidden again");
            trace("HUD map names: Alt shows them; Names on map reverses Alt");
        }

        private function scores():void
        {
            require(!list.namesOnly, "the list shows scores by default");
            list.addPlayersList(copy());
            expectTeams(["[BOT] Dave", "[Wolf] Abe", "[Axe] alice", "[wolf] carol", "[BOT] amy", "bob", "Bob", "zed"], ["xena", "[cat] Zoe", "Yuri"], true, "off (the game's order)");
        }

        private function namesOnly():void
        {
            list.namesOnly = true;
            expectTeams(["[Axe] alice", "[Wolf] Abe", "[wolf] carol", "Bob", "bob", "zed", "[BOT] amy", "[BOT] Dave"], ["[cat] Zoe", "xena", "Yuri"], false, "on");
            // Bob tops the scores: the order stays by name
            list.updatePlayer({id: 2, name: "Bob", team: 1, kills: 20, deaths: 0, score: 200, rank: 3});
            expectTeams(["[Axe] alice", "[Wolf] Abe", "[wolf] carol", "Bob", "bob", "zed", "[BOT] amy", "[BOT] Dave"], ["[cat] Zoe", "xena", "Yuri"], false, "on, a player's update");
            list.set_local_name("[wolf] carol");
            expectTeams(["[Axe] alice", "[Wolf] Abe", "[wolf] carol", "Bob", "bob", "zed", "[BOT] amy", "[BOT] Dave"], ["[cat] Zoe", "xena", "Yuri"], false, "on, the local player's name");
            list.addPlayersList(copy());
            expectTeams(["[Axe] alice", "[Wolf] Abe", "[wolf] carol", "Bob", "bob", "zed", "[BOT] amy", "[BOT] Dave"], ["[cat] Zoe", "xena", "Yuri"], false, "on, a new list");
            list.namesOnly = false;
            expectTeams(["[BOT] Dave", "[Wolf] Abe", "[Axe] alice", "[wolf] carol", "[BOT] amy", "bob", "Bob", "zed"], ["xena", "[cat] Zoe", "Yuri"], true, "off again");
            list.namesOnly = true;
            expectTeams(["[Axe] alice", "[Wolf] Abe", "[wolf] carol", "Bob", "bob", "zed", "[BOT] amy", "[BOT] Dave"], ["[cat] Zoe", "xena", "Yuri"], false, "on again");
        }
    }
}
