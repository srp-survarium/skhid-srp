package
{
    import flash.display.DisplayObjectContainer;
    import flash.display.Loader;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.UncaughtErrorEvent;
    import flash.geom.Rectangle;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;

    // Free for all's match results (docs/notes/ffa.md) with the lobby movie's own results screen
    // (SrvVictoryScreenReskin), told what the exe hands Flash (results.md §1): the server sends the
    // ranking's upper half (the extra one of an odd count) as team 0 and the rest as team 1, and the
    // exe gives each row `team` 1 when it is on the reader's own team, else 2, the reader's own row
    // the colour 0xF1E545; game_mode 8. For 1, 3, 4 and 10 players, the reader in the upper and in
    // the lower half: one ranking over the two columns, its upper half on the left whichever half the
    // reader is in. Plain names, ten rows left before overflow right; no team names
    // (label_teamA/B) and no team score. The order is the server's: kills, then the score, then
    // fewer deaths, then the place in the match (the order its rows come in). Then Team Deathmatch,
    // 10 a side: each side in its column, the names and the score back, no standings.
    // Player vs Bots too (the match settings panel, SrvTopMatchSelection, told the server's status
    // rows): free for all a mode of its menu after the protective device and Team Deathmatch, named
    // by srp's text; chosen, the map menu lists the levels the server offers a free for all on.
    // Ends showing the 10 player free for all for check.sh's screenshot, after a Team Deathmatch
    // one. Prints "RESULTS ..." lines and "RESULTS RESULT pass" or "RESULTS RESULT fail: ...".
    public class FfaResultsHarness extends Sprite
    {
        private static const FREE_FOR_ALL:int = 8;
        private static const TEAM_DEATHMATCH:int = 4;

        private var loader:Loader = new Loader();
        private var domain:ApplicationDomain;
        private var results:*;
        private var settings:*;
        private var frames:int = 0;

        public function FfaResultsHarness()
        {
            loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
            loader.contentLoaderInfo.addEventListener("ioError", function(e:Event):void {
                trace("RESULTS RESULT fail: " + e);
            });
            // the lobby's own root fails outside the game (no exe): only its classes are used
            loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
                function(e:UncaughtErrorEvent):void { e.preventDefault(); });
            loader.load(new URLRequest("lobby_menu.swf"), new LoaderContext(false, new ApplicationDomain()));
        }

        private function make(name:String):*
        {
            return new (domain.getDefinition(name) as Class)();
        }

        private function loaded(e:Event):void
        {
            try
            {
                domain = loader.contentLoaderInfo.applicationDomain;
                domain.getDefinition("MainScene").layout = make("scaleform.clik.layout.Layout");
                domain.getDefinition("survarium.controls.PlayerInfo").nickname = "Me";
                graphics.beginFill(0x1C2023);
                graphics.drawRect(0, 0, 1280, 720);
                graphics.endFill();
                // The match settings panel, off the screen: it sets itself up at its first render,
                // and takes its texts only after that (as in the game, where they come later).
                settings = make("SrvTopMatchSelection");
                settings.x = 1400;
                addChild(settings);
                addEventListener(Event.ENTER_FRAME, tick);
            }
            catch (err:Error)
            {
                trace("RESULTS RESULT fail: the lobby: " + err.getStackTrace());
            }
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
                if (frames == 5) stage.invalidate();
                if (frames == 15)
                {
                    domain.getDefinition("survarium.data.LocalizationData").setData(HarnessText.DATA.english);
                    results = make("SrvVictoryScreenReskin");
                    results.scaleX = results.scaleY = 0.9;
                    addChild(results);
                }
                if (frames == 25)
                {
                    bots();
                    sizes();
                    order();
                    updates();
                    teams();
                }
                if (frames == 55) trace("RESULTS SHOT tdm");
                if (frames == 75) show(ffa(3, 2, 1), FREE_FOR_ALL, false);
                if (frames == 105) trace("RESULTS SHOT ffa");
                if (frames == 125) show(ffa(11, 10, -1), FREE_FOR_ALL, false);
                if (frames == 155) trace("RESULTS SHOT overflow");
                if (frames == 175)
                {
                    trace("RESULTS RESULT pass");
                    removeEventListener(Event.ENTER_FRAME, tick);
                }
            }
            catch (error:Error)
            {
                trace("RESULTS RESULT fail: " + error.message);
                removeEventListener(Event.ENTER_FRAME, tick);
            }
        }

        // The Player vs Bots menus of the match settings panel (its size, mode and map lists, in
        // the order it adds them), each row "<label>=<id>".
        private function botMenus():Array
        {
            var box:DisplayObjectContainer = settings.getChildByName("pvbGame") as DisplayObjectContainer;
            require(box != null, "the match settings panel has its Player vs Bots section");
            var Menu:Class = domain.getDefinition("scaleform.clik.controls.DropdownMenu") as Class;
            var out:Array = [];
            for (var i:int = 0; i < box.numChildren; ++i)
            {
                var menu:* = box.getChildAt(i);
                if (!(menu is Menu)) continue;
                var rows:Array = [];
                for (var r:int = 0; r < menu.dataProvider.length; ++r)
                    rows.push(menu.dataProvider[r].label + "=" + menu.dataProvider[r].id);
                out.push({menu: menu, rows: rows.join(", ")});
            }
            require(out.length == 3, "the Player vs Bots section's three menus, " + out.length);
            return out;
        }

        // The server's status (B1: allowed, sizes 3 to 5) and its maps "<map>;<level>;<mode>;<free>":
        // batteries and Team Deathmatch on Rudnya and School, then the day free for all of each.
        private function bots():void
        {
            var rows:Array = [];
            var sent:Array = ["B1;0;3;5;3.4.5;4;61;0;5", "61;1;2;1", "67;2;2;1", "117;1;4;1", "120;2;4;1",
                "12;1;" + FREE_FOR_ALL + ";1", "15;2;" + FREE_FOR_ALL + ";1"];
            for (var i:int = 0; i < sent.length; ++i)
                rows.push({nickname: sent[i], place: 2050000000 + i, ranking: 0, ranking_diff: 0});
            require(domain.getDefinition("survarium.gsearch.MatchMakingSettings").pvbTake(rows), "the status taken");
            var menus:Array = botMenus();
            var modes:Array = String(menus[1].rows).split(", ");
            require(modes.length == 3 && modes[0].split("=")[1] == "2" && modes[1].split("=")[1] == "4"
                && modes[2] == "Free for all=" + FREE_FOR_ALL, "the bot modes: " + menus[1].rows);
            require(menus[2].rows == "Rudnya=61, School=67", "the bot maps of the first mode: " + menus[2].rows);
            // free for all chosen, as the open list's click tells it (closed, the menu tells nothing)
            var ListEvent:Class = domain.getDefinition("scaleform.clik.events.ListEvent") as Class;
            menus[1].menu.selectedIndex = 2;
            menus[1].menu.dispatchEvent(new ListEvent(ListEvent["INDEX_CHANGE"], true, false, 2));
            menus = botMenus();
            require(menus[2].rows == "Rudnya=12, School=15", "the bot maps of free for all: " + menus[2].rows);
            trace("RESULTS Player vs Bots: modes " + menus[1].rows + "; free for all's maps " + menus[2].rows);
        }

        // A free for all's rows: `count` players, A, B, C... by standing (A the most kills), the
        // reader "Me" at standing `me` (0-based) instead, 10 a kill; `tie` (a 0-based standing) has
        // the kills of the one above it. The server's halves, then the exe's sides relative to the
        // reader's row; sent last first (never the ranking's order).
        private function ffa(count:int, me:int, tie:int):Array
        {
            var out:Array = [];
            var kills:int = 3 * count;
            var upper:int = Math.ceil(count / 2);
            var mine:int = me < upper ? 0 : 1;
            for (var i:int = 0; i < count; ++i)
            {
                if (i != tie) kills -= 2;
                var server:int = i < upper ? 0 : 1;
                var row:Object = {id: 100 + i, name: i == me ? "Me" : String.fromCharCode(65 + i), score: 10 * kills,
                    kills: kills, deaths: i, rank: 10 + i, team: server == mine ? 1 : 2};
                if (i == me) row.color = 0xF1E545;
                out.unshift(row);
            }
            return out;
        }

        // A screen's data as the exe fills it (results.md §1), the rows given.
        private function show(players:Array, mode:int, victory:Boolean):void
        {
            var component:Object = {name: "st_statistics_score_kills", profile_name: "Me", profile_play_time: "900",
                amount: 1, money: 10, experience: 10, reputation: 1};
            var data:Object = {
                match_id: 4242, match_result: victory ? 5 : 0, game_mode: mode,
                team1_score: mode == FREE_FOR_ALL ? 0 : 88, team2_score: mode == FREE_FOR_ALL ? 0 : 61,
                team1_param: 0, team2_param: 0, team_1_players_count: 0, team_2_players_count: 0,
                score_components: [component, component, component],
                statistics_rating: {}, statistics_exp: {match_exp: 33}, statistics_money: {},
                statistics_reputation: {faction_id: 1, quest_rep: 0},
                title_0_player: "A", title_0_value_0: 280, allow_drop_card: false, players: players};
            try
            {
                results.setData(data);
            }
            catch (err:Error)
            {
                // its last steps (the drop card) need the lobby; the rows are in by then
                trace("RESULTS (setData stopped: " + err.message + ")");
            }
        }

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

        private function texts(column:*):Array
        {
            draw(column);
            var out:Array = [];
            for (var i:int = 0; i < column.dataProvider.length; ++i)
            {
                var row:* = column.getRendererAt(i);
                require(row != null, "a row for each player of " + column.name);
                draw(row);
                out.push(row.ui_name.text);
            }
            return out;
        }

        private function expect(left:Array, right:Array, what:String):void
        {
            var list:* = results.players_list;
            var l:Array = texts(list.team1list);
            var r:Array = texts(list.team2list);
            require(l.join() == left.join(), what + ": left " + l.join(", ") + ", wanted " + left.join(", "));
            require(r.join() == right.join(), what + ": right " + r.join(", ") + ", wanted " + right.join(", "));
            trace("RESULTS " + what + ": " + l.join(", ") + " | " + r.join(", "));
        }

        private function teamWords(shown:Boolean, what:String):void
        {
            require(results.txAlly.visible == shown && results.txEnemy.visible == shown,
                what + ": the team names " + (shown ? "shown" : "hidden"));
            var score:String = results.victoryPlate.textScore.text;
            require(shown ? score == "88:61" : score == "", what + ": the team score " + score);
        }

        private function sizes():void
        {
            show(ffa(1, 0, -1), FREE_FOR_ALL, true);
            teamWords(false, "free for all");
            require(results.victoryPlate.currentFrame == 1, "a winner's plate: victory");
            expect(["Me"], [], "1 player");
            show(ffa(3, 0, -1), FREE_FOR_ALL, true);
            expect(["Me", "B", "C"], [], "3 players, me first");
            show(ffa(3, 2, -1), FREE_FOR_ALL, false);
            require(results.victoryPlate.currentFrame == 2, "a loser's plate: defeat");
            expect(["A", "B", "Me"], [], "3 players, me last");
            show(ffa(4, 1, 2), FREE_FOR_ALL, true);
            expect(["A", "Me", "C", "D"], [], "4 players, equal kills and plain names");
            show(ffa(10, 7, -1), FREE_FOR_ALL, false);
            expect(["A", "B", "C", "D", "E", "F", "G", "Me", "I", "J"], [], "10 players fill left");
            show(ffa(11, 10, -1), FREE_FOR_ALL, false);
            expect(["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"], ["Me"], "11 players overflow");
            show(ffa(20, 17, -1), FREE_FOR_ALL, false);
            expect(["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"],
                ["K", "L", "M", "N", "O", "P", "Q", "Me", "S", "T"], "20 players fill both");
            teamWords(false, "free for all, 20");
        }

        private function updates():void
        {
            show(ffa(11, 10, -1), FREE_FOR_ALL, false);
            var list:* = results.players_list;
            var me:Object = list.team2list.dataProvider[0];
            list.updatePlayer({id: me.id, kills: 100, deaths: me.deaths, rank: me.rank,
                score: 0, ping: me.ping, artifacts: me.artifacts});
            expect(["Me", "A", "B", "C", "D", "E", "F", "G", "H", "I"], ["J"], "kills move right to left");
            require(list.team1list.selectedIndex == 0 && list.team2list.selectedIndex == -1,
                "local selection follows to left");
            list.updatePlayer({id: me.id, kills: 0, deaths: me.deaths, rank: me.rank,
                score: 999, ping: me.ping, artifacts: me.artifacts});
            expect(["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"], ["Me"], "kills before score, left to right");
            require(list.team2list.selectedIndex == 0 && list.team1list.selectedIndex == -1,
                "local selection follows to right");
            list.set_online_player("Me", false);
            require(list.team2list.dataProvider[0].online == false, "offline update follows actual column");
        }

        // Kills decide: B has the most score (assists) and fewer kills than A; of the four with 4
        // kills, the score, then fewer deaths order them, and D and the reader, alike in all three,
        // stay as the server sent them (its order is the place in the match); the four share a
        // standing. The rows as the server sends them: ranked, its upper half team 0.
        private function order():void
        {
            var rows:Array = [];
            var sent:Array = [["A", 5, 50, 9], ["B", 4, 95, 9], ["C", 4, 40, 0], ["D", 4, 40, 3], ["Me", 4, 40, 3]];
            for (var i:int = 0; i < sent.length; ++i)
            {
                // the reader is in the lower half (team 1 of the server's): that half is his side
                var row:Object = {id: 300 - i, name: sent[i][0], kills: sent[i][1], score: sent[i][2], deaths: sent[i][3],
                    rank: 10, team: i < 3 ? 2 : 1};
                if (row.name == "Me") row.color = 0xF1E545;
                rows.push(row);
            }
            rows = [rows[3], rows[2], rows[0], rows[4], rows[1]];
            show(rows, FREE_FOR_ALL, false);
            expect(["A", "B", "C", "D", "Me"], [], "kills, then score, fewer deaths, stable order");
            results.players_list.updatePlayer({id: 298, kills: 4, deaths: 0, score: 200, rank: 10});
            expect(["A", "C", "B", "D", "Me"], [], "score-only update reorders a kill tie");
        }

        // Team Deathmatch, 10 a side, the reader's side first: as before free for all.
        private function teams():void
        {
            var rows:Array = [];
            for (var i:int = 0; i < 20; ++i)
                rows.push({id: 200 + i, name: i == 0 ? "Me" : "P" + i, score: 300 - 10 * i, kills: 20 - i, deaths: 3,
                    rank: 12, team: i % 2 == 0 ? 1 : 2});
            show(rows, TEAM_DEATHMATCH, true);
            teamWords(true, "Team Deathmatch");
            expect(["Me", "P2", "P4", "P6", "P8", "P10", "P12", "P14", "P16", "P18"],
                ["P1", "P3", "P5", "P7", "P9", "P11", "P13", "P15", "P17", "P19"], "Team Deathmatch 10 v 10");
            var list:* = results.players_list;
            var column:Rectangle = list.team1list.getBounds(this);
            trace("RESULTS a column of 10 rows: " + list.team1list.rowHeight + " px rows, y " + Math.round(column.y)
                + " to " + Math.round(column.bottom) + " (the screen " + Math.round(results.getBounds(this).bottom) + ")");
        }
    }
}
