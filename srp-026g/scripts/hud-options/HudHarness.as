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

    // The match hud's "Hide hit markers" and "Hide player scores" (exe patch 83,
    // docs/notes/hide-hit-markers-scores.md) with the hud movie's own classes,
    // as HUD.srp_hud_options sets them (the hud's own root cannot run outside
    // the game): the crosshair's hitMarkersHidden and the Tab list's
    // (SrvPlayerList.players) namesOnly.
    // - the hit marker (root.crosshair_enemy_hit: Crosshair.setDamage) shown
    //   while the option is off; hidden on, a shown one (a kill's too) cleared;
    //   shown again off;
    // - the Tab list off: each team by score as the game sorts it, every
    //   column shown; on: each team (the board's two lists, as it groups them)
    //   by clan tag, then name (the clanless after the clans, the bots' "BOT" last), case-
    //   insensitive, equal names by id, its score, kills, deaths and rank not shown,
    //   kept so through a player's update and the local player's name; off
    //   again: as before.
    // Ends showing the list with scores, then names only, for check.sh's
    // screenshots. Prints "HUD ..." lines and "HUD RESULT pass" or "HUD RESULT
    // fail: ...". No game.
    public class HudHarness extends Sprite
    {
        private var loader:Loader = new Loader();
        private var frames:int = 0;
        private var crosshair:MovieClip;
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
                if (frames == 12) scores();
                if (frames == 40) trace("HUD SHOT scores");
                if (frames == 100) namesOnly();
                if (frames == 130)
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

        private function hitMarker():void
        {
            require(!crosshair.hitMarkersHidden && !markerShown(), "off by default, nothing shown yet");
            crosshair.setDamage(0.8, 1000, false, false);
            require(markerShown(), "off: a hit on an enemy shows the hit marker");
            crosshair.hitMarkersHidden = true;
            require(!markerShown(), "on: the marker shown is cleared");
            crosshair.setDamage(0.8, 1000, false, false);
            require(!markerShown(), "on: a hit shows no marker");
            crosshair.setDamage(1, 1000, true, false);
            require(!markerShown(), "on: a kill shows no marker");
            crosshair.hitMarkersHidden = false;
            crosshair.setDamage(1, 1000, true, false);
            require(markerShown(), "off again: a kill shows the marker");
            crosshair.hitMarkersHidden = true;
            require(!markerShown(), "on: a kill's marker is cleared too");
            crosshair.setDamage(0.5, 1000, false, false);
            require(!markerShown(), "on after a kill's marker: a hit shows nothing");
            crosshair.hitMarkersHidden = false;
            crosshair.setDamage(0.5, 1000, false, false);
            require(markerShown(), "off: hits shown again");
            trace("HUD hit marker: shown off, hidden on (a shown one cleared, a kill's too)");
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
