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

    // Free for all's Tab list and top bar (docs/notes/ffa.md) with the hud movie's own classes, as
    // HUD.set_game_type(8) sets them (the hud's own root cannot run outside the game): the Tab list's
    // (SrvPlayerList.players) ranked, and FfaProgress on the match progress indicator. The rows as
    // the exe sends them: the local player the ally (team 1), every other an enemy (team 2), the
    // game's score in `rank`, no `score`, the place in the match in `id`, in roster order.
    // Check plain names in one kill-sorted list: ten rows on the left, overflow on the right,
    // including live moves across that boundary, ties, selection, disconnects and names-only mode.
    // FFA's top HUD keeps the match timer while all team counters stay hidden after callbacks;
    // leaving FFA lets the next mode own those controls again.
    // FFA's chat (SrvGameChat.teamChannel off, HUD.set_game_type's other word) has no team tab: it
    // opens on "All", sends on the match's channel and the tab key never reaches the team's; the
    // next match of two teams has it back, first as before.
    public class FfaTabHarness extends Sprite
    {
        private var loader:Loader = new Loader();
        private var frames:int = 0;
        private var board:MovieClip;
        private var list:*;
        private var domain:ApplicationDomain;
        private var bar:*;
        private var kills:*;
        private var talk:*;
        private var sent:Array = [];

        public function FfaTabHarness()
        {
            loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
            loader.contentLoaderInfo.addEventListener("ioError", function(e:Event):void {
                trace("TAB RESULT fail: " + e);
            });
            // The hud's own root fails outside the game (no exe): only its classes are used.
            loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
                function(e:UncaughtErrorEvent):void { e.preventDefault(); });
            loader.load(new URLRequest("hud.swf"), new LoaderContext(false, new ApplicationDomain()));
        }

        private function loaded(e:Event):void
        {
            domain = loader.contentLoaderInfo.applicationDomain;
            graphics.beginFill(0x2F3431);
            graphics.drawRect(0, 0, 1280, 720);
            graphics.endFill();
            board = new (domain.getDefinition("SrvPlayerList") as Class)() as MovieClip;
            board.scaleX = board.scaleY = 0.9;
            board.x = 20 - 101 * 0.9;
            board.y = 20 - 264 * 0.9;
            addChild(board);
            list = board.players;
            list.set_local_name("Me");
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
                if (frames == 10)
                {
                    trace("TAB rows a column shows: " + Math.floor(list.team1list.height / list.team1list.rowHeight)
                        + " (" + list.team1list.rowHeight + " px rows)");
                    sizes();
                    order();
                    update();
                    offline();
                    namesOnly();
                    teams();
                    chat();
                    board.visible = false;
                    topBarShown();
                }
                if (frames == 25) topBarDrawn();
                if (frames == 35) trace("TAB SHOT topbar");
                if (frames == 55)
                {
                    topBarLeft();
                    board.visible = true;
                    list.ranked = true;
                    list.addPlayersList(players(3, 1));
                }
                if (frames == 85) trace("TAB SHOT ffa");
                if (frames == 95) list.addPlayersList(players(11, 10));
                if (frames == 115) trace("TAB SHOT overflow");
                if (frames == 135)
                {
                    trace("TAB RESULT pass");
                    removeEventListener(Event.ENTER_FRAME, tick);
                }
            }
            catch (error:Error)
            {
                trace("TAB RESULT fail: " + error.message);
                removeEventListener(Event.ENTER_FRAME, tick);
            }
        }

        // `count` players, names A, B, C... by standing (A the most kills), the local player "Me" at
        // standing `me` (0-based) instead; ranked score (`rank`) 10 a kill; sent in roster order
        // (reversed, so never the ranking's). Equal `tie` (a 0-based standing) shares the kills of
        // the one above it.
        private function players(count:int, me:int, tie:int = -1):Array
        {
            var out:Array = [];
            var kills:int = 3 * count;
            for (var i:int = 0; i < count; ++i)
            {
                if (i != tie) kills -= 2;
                var name:String = i == me ? "Me" : String.fromCharCode(65 + i);
                out.unshift({id: i + 1, name: name, team: i == me ? 1 : 2, kills: kills, deaths: i, rank: 10 * kills,
                    ping: 66, artifacts: 0, squad_id: 0, squad_state: 0});
            }
            return out;
        }

        private function draw(control:*):void
        {
            try
            {
                control.validateNow();
            }
            catch (error:Error)
            {
                // Ruffle refuses what Scaleform takes (#2007), after the row is drawn
                if (error.errorID != 2007) throw error;
            }
        }

        // Each drawn row's name text of a column.
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
            var l:Array = texts(list.team1list);
            var r:Array = texts(list.team2list);
            require(l.join() == left.join(), what + ": left " + l.join(", ") + ", wanted " + left.join(", "));
            require(r.join() == right.join(), what + ": right " + r.join(", ") + ", wanted " + right.join(", "));
            trace("TAB " + what + ": " + l.join(", ") + " | " + r.join(", "));
        }

        private function chosen():String
        {
            if (list.team1list.selectedIndex >= 0) return "left " + list.team1list.selectedIndex;
            if (list.team2list.selectedIndex >= 0) return "right " + list.team2list.selectedIndex;
            return "none";
        }

        private function sizes():void
        {
            list.ranked = true;
            require(!list.team1.visible && !list.team2.visible, "ranked: no team names");
            list.addPlayersList(players(1, 0));
            expect(["Me"], [], "one player");
            list.addPlayersList(players(3, 2));
            expect(["A", "B", "Me"], [], "three players stay left");
            require(chosen() == "left 2", "my row chosen on the left");
            list.addPlayersList(players(4, 1, 2));
            expect(["A", "Me", "C", "D"], [], "equal kills: fewer deaths first, plain names");
            list.addPlayersList(players(10, 7));
            expect(["A", "B", "C", "D", "E", "F", "G", "Me", "I", "J"], [], "ten fill the left");
            list.addPlayersList(players(11, 10));
            expect(["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"], ["Me"], "eleventh spills right");
            require(chosen() == "right 0", "my row chosen in the overflow");
            list.addPlayersList(players(20, 17));
            expect(["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"],
                   ["K", "L", "M", "N", "O", "P", "Q", "Me", "S", "T"], "twenty fill both columns");
            require(chosen() == "right 7", "my row chosen at the end of the ranking");
        }

        // Kills decide: B has the most score (assists) and fewer kills than A; of the four with 4
        // kills, the score, then fewer deaths, then the place (D's 2 before E's 5) order them, and
        // the sorted rows keep their plain names. Sent in roster order.
        private function order():void
        {
            var rows:Array = [];
            for each (var r:Array in [["C", 1, 4, 40, 0], ["D", 2, 4, 40, 3], ["A", 3, 5, 50, 9], ["B", 4, 4, 95, 9],
                ["E", 5, 4, 40, 3]])
                rows.push({id: r[1], name: r[0], team: 2, kills: r[2], deaths: r[4], rank: r[3], ping: 66, artifacts: 0,
                    squad_id: 0, squad_state: 0});
            list.addPlayersList(rows);
            expect(["A", "B", "C", "D", "E"], [], "kills, then score, fewer deaths, place");
        }

        // The eleventh player overtakes the leader: they move right-to-left and J spills right.
        // J then overtakes everyone, pushing I from the left column to the right.
        private function update():void
        {
            list.addPlayersList(players(11, 10));
            list.updatePlayer({id: 11, kills: 32, deaths: 10, rank: 320, ping: 66, artifacts: 0});
            expect(["Me", "A", "B", "C", "D", "E", "F", "G", "H", "I"], ["J"], "right-to-left after a kill");
            require(chosen() == "left 0", "selection follows the player across columns");
            list.updatePlayer({id: 10, kills: 33, deaths: 9, rank: 330, ping: 66, artifacts: 0});
            expect(["J", "Me", "A", "B", "C", "D", "E", "F", "G", "H"], ["I"], "next update spills I right");
            require(chosen() == "left 1", "selection follows the reordered player");
        }

        private function offline():void
        {
            list.addPlayersList(players(20, 7));
            list.set_online_player("B", false);
            list.set_online_player("N", false);
            draw(list.team1list);
            draw(list.team2list);
            var b:* = list.team1list.getRendererAt(1);
            var n:* = list.team2list.getRendererAt(3);
            draw(b);
            draw(n);
            require(b.ui_name.text == "B" && b.ui_name.textField.textColor == 0x999999, "B gone: grey, no prefix");
            require(n.ui_name.text == "N" && n.ui_name.textField.textColor == 0x999999, "N gone: grey, no prefix");
        }

        private function namesOnly():void
        {
            var mixed:Array = players(5, 3);
            for each (var p:Object in mixed)
                p.name = {C: "[Axe] bob", E: "[BOT] zed"}[p.name] || p.name;
            list.addPlayersList(mixed);
            list.namesOnly = true;
            expect(["[Axe] bob", "A", "B", "Me", "[BOT] zed"], [], "names only");
            list.namesOnly = false;
            expect(["A", "B", "[Axe] bob", "Me", "[BOT] zed"], [], "names only off");
        }

        private function teams():void
        {
            list.ranked = false;
            require(list.team1.visible && list.team2.visible, "teams: the team names shown");
            list.addPlayersList(players(4, 2));
            expect(["Me"], ["A", "B", "D"], "Team Deathmatch after it");
        }

        // A match's start as the hud's root has it: HUD.reset (the chat cleared), set_game_type
        // (whether the mode has a team), then the tabs the exe gives (chat_handler::set_mode,
        // 0x209430): the team's (id 4, sent on 6 or 7), the match's (id 3, on 5) and, in a squad,
        // the squad's (id 0, on 8).
        private function matchStarts(teamChannel:Boolean, squad:Boolean):void
        {
            talk.clear();
            talk.teamChannel = teamChannel;
            tabs(squad);
        }

        private function tabs(squad:Boolean):void
        {
            talk.addTab({id: 4, name: "Team", channels: [6, 5, 8, 2], channel_to_send: 6, closable: false,
                save_history: false});
            talk.addTab({id: 3, name: "All", channels: [6, 5, 8, 2], channel_to_send: 5, closable: false,
                save_history: false});
            if (squad)
                talk.addTab({id: 0, name: "Squad", channels: [8, 5, 6], channel_to_send: 8, closable: false,
                    save_history: true});
        }

        // The tab the chat opens on, by the name before its input, and the channel a line typed
        // there goes out on.
        private function typedOn(name:String, channel:uint, what:String):void
        {
            talk.startEnterMessage();
            require(talk.textFieldName.text == name + ":", what + ": typed into " + talk.textFieldName.text
                + ", wanted " + name);
            talk.text_inpput.text = "gl";
            talk.submitText();
            require(sent.length == 1 && sent[0] == channel, what + ": sent on " + sent + ", wanted " + channel);
            sent = [];
            talk.stopEnterMessage();
            trace("TAB chat, " + what + ": " + name + ", channel " + channel);
        }

        // The tabs the tab key goes through (next_tab), from the one open, once round.
        private function round(count:int):String
        {
            var names:Array = [];
            for (var i:int = 0; i < count; ++i)
            {
                talk.nextTab();
                names.push(talk.textFieldName.text);
            }
            return names.join(" ");
        }

        private function chat():void
        {
            talk = new (domain.getDefinition("SrvGameChat") as Class)();
            talk.x = 2000;                  // off the window
            addChild(talk);
            draw(talk);
            talk.send_function = function(input:Object, channel:uint):Boolean { sent.push(channel); return true; };

            // Team Deathmatch, as it always was: the team's tab first.
            matchStarts(true, true);
            typedOn("Team", 6, "two teams");
            require(round(3) == "All: Squad: Team:", "two teams: the tab key goes All, Squad, Team");

            // A free for all after it, the team's tab left open by the last match: "All".
            matchStarts(false, true);
            typedOn("All", 5, "free for all");
            require(round(2) == "Squad: All:", "free for all: the tab key goes Squad, All, never Team");
            talk.prevTab();
            require(talk.textFieldName.text == "Squad:", "free for all: back to Squad, not Team");
            talk.prevTab();
            typedOn("All", 5, "free for all, round the tabs");
            talk.got_message({type: 5, name: "A", text: "gl"});
            talk.got_message({type: 2, text: "a system line"});
            matchStarts(false, false);
            typedOn("All", 5, "free for all, no squad");
            require(round(2) == "All: All:", "free for all, no squad: the tab key has nowhere to go");
            require(talk.hasTab(4), "free for all: the team's tab given, not offered");
            // The exe takes its two tabs away and gives them again (set_mode).
            talk.removeTab(4);
            talk.removeTab(3);
            tabs(false);
            typedOn("All", 5, "free for all, the tabs given again");

            // The mode told after the tabs: the team's tab open goes, "All" takes its place.
            talk.clear();
            talk.teamChannel = true;
            tabs(true);
            talk.teamChannel = false;
            typedOn("All", 5, "free for all told after the tabs");
            talk.nextTab();
            talk.teamChannel = true;
            typedOn("Squad", 8, "two teams told after the tabs: the tab open kept");
            require(round(3) == "Team: All: Squad:", "two teams told after the tabs: Team is back");

            // The next match of two teams: the team's tab is back, first.
            matchStarts(true, false);
            typedOn("Team", 6, "two teams after a free for all");
            require(round(2) == "All: Team:", "two teams after a free for all: the tab key goes All, Team");
            removeChild(talk);
        }

        private function kill(own:int, top:int, target:int, what:String):void
        {
            for each (var name:String in ["team_1_score", "team_2_score", "team1Progress", "team2Progress",
                                         "team1LastPlayers", "team2LastPlayers"])
                require(!bar[name].visible, what + ": " + name + " hidden");
            require(bar.match_time_2_tdm.visible && bar.match_time_2_tdm.text == "12:34", what + ": match timer retained");
            trace("TAB top bar, " + what + ": timer only");
        }

        private function topBarShown():void
        {
            bar = new (domain.getDefinition("SrvMatchProgress") as Class)();
            bar.x = 640;
            bar.y = 80;
            addChild(bar);
            trace("TAB the top bar: " + bar.totalFrames + " frames");
            kills = new (domain.getDefinition("survarium.modes.FfaProgress") as Class)();
            bar.gotoAndStop(3);
            kills.tell(12, 18, 30);
            kills.draw();
            require(bar.team_1_score == null && bar.currentFrame == 3, "told before the mode: the battery frame as it was");
            bar.gotoAndStop(5);
            for each (var part:String in ["team_1_score", "team_2_score", "team1Progress", "team2Progress",
                "team1LastPlayers", "team2LastPlayers", "match_time_2_tdm"])
                require(bar[part] != null, "the top bar's frame 5 has " + part);
            kills.show(bar);
            bar.match_time_2_tdm.text = "12:34";
            kill(12, 18, 30, "shown, as told before the mode");
        }

        // Frames later (the frame's own script has run): HUD.set_game_progress draws them again.
        private function topBarDrawn():void
        {
            trace("TAB top bar after the frame's script, before the next draw: bars "
                + (bar.team_1_score.visible ? "shown" : "hidden") + ", full at " + bar.team_1_score.maximum);
            kills.draw();
            kill(12, 18, 30, "drawn again");
            kills.tell(13, 18, 30);
            kill(13, 18, 30, "a kill");
        }

        // The end (the leader at the target), a number past it, then another mode on the same frame
        // (Team Deathmatch's tickets): nothing told later touches it.
        private function topBarLeft():void
        {
            kills.tell(13, 30, 30);
            kill(13, 30, 30, "the leader wins");
            kills.tell(31, 31, 30);
            kill(31, 31, 30, "past the target");
            kills.tell(0, 0, 0);
            kill(0, 0, 1, "nothing known");
            kills.hide();
            bar.team_1_score.visible = bar.team_2_score.visible = true;
            bar.team1Progress.visible = bar.team2Progress.visible = true;
            bar.team_1_score.maximum = bar.team_2_score.maximum = 150;
            bar.team_1_score.value = 88;
            bar.team_2_score.value = 61;
            bar.team1Progress.text = "88";
            bar.team2Progress.text = "61";
            kills.tell(5, 9, 30);
            kills.draw();
            require(bar.team_1_score.visible && bar.team_2_score.visible, "another mode: its bars remain shown");
            require(bar.team1Progress.text == "88" && bar.team2Progress.text == "61" && bar.team_1_score.maximum == 150
                && bar.team_1_score.value == 88 && bar.team_2_score.value == 61, "another mode: its own numbers kept");
            trace("TAB top bar, another mode after it: told 5 and 9, shows " + bar.team1Progress.text + " and "
                + bar.team2Progress.text);
            removeChild(bar);
        }
    }
}
