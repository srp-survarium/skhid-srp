package survarium.rooms
{
    import flash.display.DisplayObject;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.text.TextField;
    import flash.utils.getTimer;
    import survarium.constants.RoomCode;
    import survarium.constants.RoomFit;
    import survarium.constants.RoomPhase;
    import survarium.constants.UiSound;
    import survarium.constants.GameMode;

    // The asker's room, after the design: what it is, the banner with the
    // countdown, the vote (always there) and the members, each place of the
    // room a slot; Leave, Join match, and Switch room to one of a lower max
    // level that takes us.
    public class RoomsRoom
    {
        // The vote's alert: the game's (UiSound.INVITE, its important
        // message's), not the match's own start (UiSound.MATCH_FOUND), the owner's; once a vote, when 10 s
        // or less are left of it (the vote runs until the match starts, the
        // owner's of 2026-10-19), also with the window hidden.
        private static const ALERT_SECS:int = 10;

        private var win:RoomsWindow;
        private var look:RoomsLook;
        private var _banner:TextField;
        private var _bannerLabel:TextField;
        private var _bannerHint:TextField;
        internal var _alerted:Boolean = false;
        // A right-click on a member: the game's right-click menu with "Show
        // player's information" (LobbyMenu.showPlayerInfoMenu, the
        // player-gear feature), naming the member by its place in our room's
        // state and its serial (as a kick does).
        internal var _roomHits:Array = [];

        public function RoomsRoom(win:RoomsWindow)
        {
            super();
            this.win = win;
            look = win.look;
        }

        // The asker's room, after the design: what it is, the banner with the
        // countdown, the vote (always there) and the members, each place of
        // the room a slot.
        internal function render():void
        {
            var room:Object = win._room;
            // A private room has no size of its own (the owner's of 2026-10-26).
            var info:String = win.say("level") + " " + String(room.level) + "  ·  " + win.say("max") + " "
                + win.levelText(room.max) + (room.pin ? "" : "  ·  " + room.size + " v " + room.size) + "  ·  " + win.say("pool") + " "
                + room.pool + "  ·  " + win.say("code") + " " + room.key + (room.pin ? "  ·  PIN" : "");
            look.text(look._content, info, 16, 50, look._w - 32, 15, RoomsLook.GREY);
            // The banner: the phase and its time.
            var band:Shape = new Shape();
            band.graphics.beginFill(room.phase == RoomPhase.WAITING ? 0x0A0E12 : 0x3C280C, 0.85);
            band.graphics.drawRect(1, 76, look._w - 2, 50);
            band.graphics.endFill();
            look._content.addChild(band);
            _bannerLabel = look.text(look._content, "", 16, 90, 300, 15, RoomsLook.GREY);
            _banner = look.text(look._content, "", 260, 82, 200, 30, room.phase == RoomPhase.WAITING ? RoomsLook.LIGHT : RoomsLook.GOLD);
            _bannerHint = look.text(look._content, "", 440, 91, look._w - 456, 15, RoomsLook.GREY);
            banner();
            var y:Number = 140;
            look.text(look._content, win.say("vote").toUpperCase(), 16, y, 400, 16, RoomsLook.LIGHT);
            look.text(look._content, win.say("voteHint"), 360, y + 1, look._w - 376, 14, RoomsLook.GREY);
            y += 28;
            var cw:int = int((look._w - 32 - 2 * 14) / 3);
            var ch:int = Math.min(190, int((look._h - y) * 0.48));
            for (var i:int = 0; i < room.cards.length; i++)
            {
                var card:Object = room.cards[i];
                var e:Object = win.entry(card.map);
                var cell:Sprite = new Sprite();
                cell.x = 16 + i * (cw + 14);
                cell.y = y;
                if (e != null)
                    cell.addChild(look.mapRender(e.level, e.weather, cw, ch));
                var shade:Shape = new Shape();
                shade.graphics.beginFill(0x000000, 0.01);
                shade.graphics.drawRect(0, 0, cw, ch);
                shade.graphics.endFill();
                shade.graphics.beginFill(0x000000, 0.62);
                shade.graphics.drawRect(0, ch - 58, cw, 58);
                shade.graphics.endFill();
                var share:Number = room.members.length > 0 ? card.votes / room.members.length : 0;
                shade.graphics.beginFill(0xFFFFFF, 0.14);
                shade.graphics.drawRect(12, ch - 10, cw - 24, 5);
                shade.graphics.endFill();
                shade.graphics.beginFill(RoomsLook.GOLD, 1);
                shade.graphics.drawRect(12, ch - 10, (cw - 24) * Math.min(1, share), 5);
                shade.graphics.endFill();
                var chosen:Boolean = room.phase == RoomPhase.COUNTDOWN && card.map == room.next;
                shade.graphics.lineStyle(card.mine || chosen ? 3 : 1, chosen ? 0xFFFFFF : card.mine ? RoomsLook.GOLD : 0x26323C);
                shade.graphics.drawRect(0, 0, cw, ch);
                cell.addChild(shade);
                // The vote is shut during the wait: its map marked, the rest dim.
                if (room.phase == RoomPhase.COUNTDOWN && !chosen)
                    cell.alpha = 0.45;
                if (chosen)
                    look.text(cell, win.say("nextMap").toUpperCase(), cw - 120, 13, 108, 13, 0xFFFFFF, "right");
                if (e != null)
                {
                    var tag:Shape = new Shape();
                    tag.graphics.lineStyle(1, e.mode == GameMode.TEAM_DEATHMATCH ? 0xB23A2A
                        : e.mode == GameMode.RESEARCH ? 0x6E9A3C : 0x5A8FB5);
                    tag.graphics.beginFill(0x000000, 0.7);
                    tag.graphics.drawRect(10, 10, cw * 0.55, 24);
                    tag.graphics.endFill();
                    cell.addChild(tag);
                    look.text(cell, win.modeName(e.mode).toUpperCase(), 16, 13, cw * 0.55 - 12, 13, 0xFFFFFF);
                    look.text(cell, win.levelName(e.level), 12, ch - 54, cw - 80, 21, 0xFFFFFF);
                    look.text(cell, win.weatherName(e.weather).toUpperCase(), 12, ch - 30, cw - 80, 13, 0xCDD8E0);
                }
                look.text(cell, String(card.votes), cw - 70, ch - 52, 58, 24, RoomsLook.GOLD, "right");
                cell.buttonMode = true;
                cell.mouseChildren = false;
                cell.name = String(i);
                cell.addEventListener(MouseEvent.CLICK, onVote, false, 0, true);
                look.route(cell, function(control:DisplayObject, e:MouseEvent):void
                {
                    RoomsLook.clickSound();
                    win.ask(RoomsWire.VOTE + Math.max(0, Math.min(RoomsWire.LAST_CARD, int(control.name))), 1);
                });
                look._content.addChild(cell);
            }
            y += ch + 16;
            // The players: one list, the sides drawn when the match starts.
            var panel:Shape = new Shape();
            panel.graphics.lineStyle(1, 0x26323C);
            panel.graphics.beginFill(0x080C10, 0.78);
            panel.graphics.drawRect(16, y, look._w - 32, look._h - RoomsLook.FOOT_H - y - 8);
            panel.graphics.endFill();
            look._content.addChild(panel);
            look.text(look._content, win.say("players").toUpperCase(), 28, y + 10, 140, 15, RoomsLook.LIGHT);
            // A private room: its owner's two teams (the owner's of
            // 2026-10-26); an open room: one list, the sides drawn at the start.
            var arranged:Boolean = room.pin && room.teams != null;
            var hint:String = !arranged ? win.say("teams") : room.owner ? win.say("teamsOwner") : win.say("teamsMember");
            look.text(look._content, room.members.length + " / " + room.size * 2 + "   ·   " + hint,
                160, y + 11, look._w - 200, 14, RoomsLook.GREY);
            look.line(look._content, 17, y + 38, look._w - 34);
            // Every place at once (20 at most): the rows share the panel's
            // height, 34 at most, no scrolling.
            var inner:Sprite = new Sprite();
            inner.x = 28;
            inner.y = y + 40;
            look._content.addChild(inner);
            var slots:int = room.size * 2;
            var colW:Number;
            var rowH:int;
            var m:int;
            if (arranged)
            {
                // Two columns, team 1 left, team 2 right, 10 rows each.
                colW = (look._w - 32 - 24) / 2;
                var perTeam:int = int(slots / 2);
                rowH = Math.min(30, int((look._h - RoomsLook.FOOT_H - y - 8 - 46 - 22) / perTeam));
                var used:Array = [0, 0];
                var moving:Boolean = room.owner && room.phase == RoomPhase.WAITING;
                for (var t:int = 0; t < 2; t++)
                {
                    var count:int = 0;
                    for (m = 0; m < room.members.length; m++)
                        if (teamOf(m) == t)
                            count++;
                    look.text(inner, win.say(t == 0 ? "team1" : "team2").toUpperCase() + "   " + count + " / " + perTeam,
                        t * colW + 34, 0, colW - 50, 13, RoomsLook.GOLD);
                }
                for (m = 0; m < room.members.length; m++)
                {
                    var side:int = teamOf(m);
                    // The owner's move: to the other team (the server refuses
                    // one onto a full team).
                    memberRow(inner, room, m, side * colW, 22 + used[side] * rowH, colW, rowH,
                        moving ? (side == 0 ? "»" : "«") : null);
                    used[side]++;
                }
                for (t = 0; t < 2; t++)
                    for (var free:int = used[t]; free < perTeam; free++)
                        memberRow(inner, room, -1, t * colW, 22 + free * rowH, colW, rowH, null);
            }
            else
            {
                colW = (look._w - 32 - 24) / 4;
                rowH = Math.min(34, int((look._h - RoomsLook.FOOT_H - y - 8 - 46) / Math.ceil(slots / 4)));
                for (m = 0; m < slots; m++)
                    memberRow(inner, room, m < room.members.length ? m : -1, (m % 4) * colW, int(m / 4) * rowH, colW, rowH, null);
            }
            // A room of a lower max level that takes us: one click there
            // (the server offers it only while we play no match).
            var buttons:Array = [[win.say("leave"), onLeave, null]];
            // A private room waits for its owner's Start (no countdown).
            if (startsByOwner())
                buttons.push([win.say("startMatch"), onStart, null]);
            if (room.joinIn >= 0)
                buttons.push([win.say("joinMatch"), onJoinMatch, null]);
            if (room.switchTo != null)
            {
                var to:Object = room.switchTo;
                buttons.push([win.say("switchTo"), onSwitch, to]);
                // The game's own button, untinted (the owner's of 2026-10-21).
                var made:Array = look.footer(buttons);
                // Two buttons leave room for what the switch is; three do not.
                if (buttons.length < 3)
                {
                // What the switch is, left of the buttons: the room's gear cap,
                // whose it is and how full.
                var room_w:Number = made[0].x - 28;
                var fy:Number = look._h - RoomsLook.FOOT_H;
                look.text(look._content, win.say("switchWhy").split("%L").join(String(to.max)), 16, fy + 5, room_w, 13, RoomsLook.GOLD);
                look.text(look._content, win.say("switchWho").split("%N").join(to.name).split("%P").join(to.players + " / " + to.size * 2),
                    16, fy + 24, room_w, 12, RoomsLook.GREY);
                }
            }
            else
                look.footer(buttons);
        }

        // A place of the room's list at (mx, my): the member at `m` of the
        // state (-1: a free place), its gear, red mark and name; for the
        // owner of a private room its kick and, with `move`, its move to the
        // other team.
        private function memberRow(inner:Sprite, room:Object, m:int, mx:Number, my:Number, colW:Number, rowH:int,
            move:String):void
        {
            var member:Object = m >= 0 ? room.members[m] : null;
            var mid:int = int((rowH - 20) / 2);
            var under:Shape = new Shape();
            under.graphics.beginFill(0x1A232B, 1);
            under.graphics.drawRect(mx, my + rowH - 1, colW - 10, 1);
            under.graphics.endFill();
            inner.addChild(under);
            if (member == null)
            {
                look.text(inner, win.say("open"), mx + 34, my + mid, colW - 50, 15, 0x4D5C67);
                return;
            }
            // A right-click on it: its equipment, by its place and the
            // state's serial, as a kick names it (`onContentClick`).
            var hitArea:Shape = new Shape();
            hitArea.graphics.beginFill(0x000000, 0.01);
            hitArea.graphics.drawRect(mx, my, colW - 10, rowH);
            hitArea.graphics.endFill();
            inner.addChild(hitArea);
            _roomHits.push({"control": hitArea, "name": member.name, "place": m,
                "how": LobbyMenu.PLAYER_INFO_BY_ROOM_PLACE,
                "player": m + RoomsWire.MEMBERS_MAX * (int(room.serial) % RoomsWire.KICK_CHECKS)});
            look.text(inner, String(member.gear), mx, my + mid + 1, 26, 14, member.fit == RoomFit.GEAR ? RoomsLook.RED : RoomsLook.DIM, "right");
            // A red mark: this kit cannot play the next match as it is
            // now; the match starts without it and takes it out of the room.
            var nameX:Number = mx + 34;
            if (member.fit != RoomFit.YES)
            {
                var mark:Shape = new Shape();
                mark.graphics.beginFill(RoomsLook.RED, 1);
                mark.graphics.drawCircle(mx + 40, my + mid + 10, 7);
                mark.graphics.endFill();
                inner.addChild(mark);
                look.text(inner, "!", mx + 33, my + mid + 1, 14, 12, 0xFFFFFF, "center");
                nameX = mx + 52;
            }
            // The buttons at the row's right end: the move, then the kick.
            var buttons:int = (move != null ? 1 : 0) + (room.owner && room.pin && !member.me ? 1 : 0);
            // A long name cut to its place, the owner's tag kept whole.
            // Away (its game gone for now): grey until it is back, or out
            // of the room after about 90 s (`memberColour`).
            RoomsLook.fitName(look.text(inner, "", nameX, my + mid, colW - 46 - 34 * Math.max(1, buttons) - (nameX - mx - 34), 16,
                RoomsLook.memberColour(member)), member.name, member.owner ? "  (" + win.say("ownerTag") + ")" : "");
            var bx:Number = mx + colW - 44;
            var by:Number = my + int((rowH - 28) / 2);
            // Only a private room (a PIN) turns a member out.
            if (room.owner && room.pin && !member.me)
            {
                look.button(inner, "×", bx, by, 30, onKick, m, 28);
                bx -= 34;
            }
            if (move != null)
                look.button(inner, move, bx, by, 30, onMove, m, 28);
        }

        // The team of the member at `m` of a private room's list (`A;<sides>`).
        internal function teamOf(m:int):int
        {
            var teams:String = win._room != null && win._room.teams != null ? win._room.teams : "";
            return teams.charAt(m) == "1" ? 1 : 0;
        }

        // Whether the asker may start its room's match now: it owns a private
        // room (a PIN) that is not playing one (the server checks it too).
        internal function startsByOwner():Boolean
        {
            var room:Object = win._room;
            return room != null && room.pin && room.owner && room.phase == RoomPhase.WAITING;
        }

        private function myFit():int
        {
            if (win._room != null)
                for (var i:int = 0; i < win._room.members.length; i++)
                    if (win._room.members[i].me)
                        return win._room.members[i].fit;
            return RoomFit.YES;
        }

        internal function alertNear():void
        {
            if (win._room == null || win._room.phase != RoomPhase.VOTE || _alerted)
                return;
            var left:int = win._room.secs - int((getTimer() - win._roomAt) / 1000);
            if (left <= ALERT_SECS)
            {
                _alerted = true;
                RoomsLook.gameSound(UiSound.INVITE);
            }
        }

        // The countdown, kept moving between the server's answers.
        internal function banner():void
        {
            if (win._room == null || _banner == null || _banner.parent != look._content)
                return;
            var passed:int = int((getTimer() - win._roomAt) / 1000);
            var fit:int = myFit();
            var unfit:String = fit == RoomFit.KIT ? win.say("unfitKit") : fit == RoomFit.GEAR ? win.say("unfitGear") : "";
            if (win._room.phase == RoomPhase.VOTE)
            {
                _bannerLabel.text = win.say("voting").toUpperCase();
                _banner.text = RoomsText.clock(win._room.secs - passed);
                _bannerHint.text = unfit != "" ? unfit : win.say("voteNow");
            }
            else if (win._room.phase == RoomPhase.COUNTDOWN)
            {
                _bannerLabel.text = win.say("starts").toUpperCase();
                _banner.text = RoomsText.clock(win._room.secs - passed);
                var e:Object = win.entry(win._room.next);
                _bannerHint.text = unfit != "" ? unfit : (e != null ? win.say("nextMap") + ": " + win.levelName(e.level) + " · "
                    + win.weatherName(e.weather) + " · " + win.modeName(e.mode) : "");
            }
            else if (win._room.phase == RoomPhase.IN_MATCH)
            {
                _bannerLabel.text = win.say("playing").toUpperCase();
                _banner.text = RoomsText.clock(win._room.secs + passed);
                // Waiting to go into it: the seconds left, counted here; or
                // why it cannot go in now; or its own kit or gear.
                var why:String = "joinWhy" + win._room.joinWhy;
                _bannerHint.text = win._room.joinIn >= 0
                    ? win.say("joinIn").split("%S").join(String(Math.max(0, win._room.joinIn - passed)))
                    : RoomsText.known(why) ? win.say(why)
                    : unfit != "" ? unfit : win.say("lateHint");
            }
            else if (win._room.pin)
            {
                // A private room: its owner's Start, nobody's count.
                _bannerLabel.text = win.say("waitingShort").toUpperCase();
                _banner.text = win._room.members.length + " / " + win._room.size * 2;
                _bannerHint.text = unfit != "" ? unfit : win._room.owner ? win.say("startHint") : win.say("waitingOwner");
            }
            else
            {
                _bannerLabel.text = win.say("waitingShort").toUpperCase();
                _banner.text = win._room.members.length + " / " + win._room.min;
                _bannerHint.text = win.say("waiting");
            }
        }

        // LEAVE in the room: the only way out of it (the cross only hides
        // the window).
        private function onLeave(e:Event):void
        {
            if (win._view == "room" && win._room != null)
                win.ask(RoomsWire.LEAVE, 1);
        }

        // Into the room's match now, without waiting for the countdown.
        private function onJoinMatch(e:Event):void
        {
            if (win._steps.length > 0)
                return;
            win.busy(e);
            win.ask(RoomsWire.JOIN_MATCH, 1);
            win._fastUntil = getTimer() + 5000;
        }

        // A private room's owner starts its match now, with whoever is in.
        private function onStart(e:Event):void
        {
            if (win._steps.length > 0 || !startsByOwner())
                return;
            win.busy(e);
            win.ask(RoomsWire.START, 1);
            win._fastUntil = getTimer() + 5000;
        }

        // The room offered of a lower max level: joining it leaves ours.
        private function onSwitch(e:Event):void
        {
            var to:Object = e.currentTarget.data;
            if (to == null || win._steps.length > 0)
                return;
            win.busy(e);
            win._code = RoomCode.NONE;
            win.ask(RoomsWire.JOIN + to.key, 1);
            win._fastUntil = getTimer() + 5000;
        }

        private function onVote(e:Event):void
        {
            RoomsLook.clickSound();
            win.ask(RoomsWire.VOTE + Math.max(0, Math.min(RoomsWire.LAST_CARD, int(e.currentTarget.name))), 1);
        }

        private function onKick(e:Event):void
        {
            // The place in the list this state gave, and that state's serial:
            // a kick from an older list does nothing.
            if (win._room != null)
                win.ask(RoomsWire.KICK + int(e.currentTarget.data) + RoomsWire.MEMBERS_MAX * (int(win._room.serial) % RoomsWire.KICK_CHECKS), 1);
        }

        // The owner moves the member at that place to the other team: the
        // place and the state's serial, as a kick names it.
        private function onMove(e:Event):void
        {
            if (win._room != null)
                win.ask(RoomsWire.MOVE + int(e.currentTarget.data) + RoomsWire.MEMBERS_MAX * (int(win._room.serial) % RoomsWire.KICK_CHECKS), 1);
        }

        // The whole name of the member at that place of our room (the state
        // cuts a long one at 32 bytes and ends it with "…"): from our room's
        // member rows (`S1;G`) when they have it, else the state's.
        internal function wholeName(place:int, cut:String):String
        {
            if (win._room == null || win._seen == null || win._seen.key != win._room.key || place >= win._seen.members.length)
                return cut;
            var whole:String = String(win._seen.members[place].name);
            var head:String = cut.charAt(cut.length - 1) == RoomsLook.DOTS ? cut.substr(0, cut.length - 1) : cut;
            return whole.substr(0, head.length) == head ? whole : cut;
        }
    }
}
