package survarium.rooms
{
    import flash.display.DisplayObject;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.text.TextField;
    import flash.utils.getTimer;

    // The asker's room, after the design: what it is, the banner with the
    // countdown, the vote (always there) and the members, each place of the
    // room a slot; Leave, Join match, and Switch room to one of a lower max
    // level that takes us.
    public class RoomsRoom
    {
        // The vote's alert: the game's (6, its important message's), not
        // the match's own start (15), the owner's; once a vote, when 10 s
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
        // The footer's Switch room while the room view shows it.
        internal var _switchBtn:* = null;

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
            var info:String = win.say("level") + " " + String(room.level) + "  ·  " + win.say("max") + " "
                + win.levelText(room.max) + "  ·  " + room.size + " v " + room.size + "  ·  " + win.say("pool") + " "
                + room.pool + "  ·  " + win.say("code") + " " + room.key + (room.pin ? "  ·  PIN" : "");
            look.text(look._content, info, 16, 50, look._w - 32, 15, RoomsLook.GREY);
            // The banner: the phase and its time.
            var band:Shape = new Shape();
            band.graphics.beginFill(room.phase == 0 ? 0x0A0E12 : 0x3C280C, 0.85);
            band.graphics.drawRect(1, 76, look._w - 2, 50);
            band.graphics.endFill();
            look._content.addChild(band);
            _bannerLabel = look.text(look._content, "", 16, 90, 300, 15, RoomsLook.GREY);
            _banner = look.text(look._content, "", 260, 82, 200, 30, room.phase == 0 ? RoomsLook.LIGHT : RoomsLook.GOLD);
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
                var chosen:Boolean = room.phase == 1 && card.map == room.next;
                shade.graphics.lineStyle(card.mine || chosen ? 3 : 1, chosen ? 0xFFFFFF : card.mine ? RoomsLook.GOLD : 0x26323C);
                shade.graphics.drawRect(0, 0, cw, ch);
                cell.addChild(shade);
                // The vote is shut during the wait: its map marked, the rest dim.
                if (room.phase == 1 && !chosen)
                    cell.alpha = 0.45;
                if (chosen)
                    look.text(cell, win.say("nextMap").toUpperCase(), cw - 120, 13, 108, 13, 0xFFFFFF, "right");
                if (e != null)
                {
                    var tag:Shape = new Shape();
                    tag.graphics.lineStyle(1, e.mode == 4 ? 0xB23A2A : 0x5A8FB5);
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
                    win.ask(RoomsWire.VOTE + Math.max(0, Math.min(2, int(control.name))), 1);
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
            look.text(look._content, room.members.length + " / " + room.size * 2 + "   ·   " + win.say("teams"),
                160, y + 11, look._w - 200, 14, RoomsLook.GREY);
            look.line(look._content, 17, y + 38, look._w - 34);
            var colW:Number = (look._w - 32 - 24) / 4;
            // Every place at once (20 at most): the rows share the panel's
            // height, 34 at most, no scrolling.
            var inner:Sprite = new Sprite();
            inner.x = 28;
            inner.y = y + 40;
            look._content.addChild(inner);
            var slots:int = room.size * 2;
            var rowH:int = Math.min(34, int((look._h - RoomsLook.FOOT_H - y - 8 - 46) / Math.ceil(slots / 4)));
            var mid:int = int((rowH - 20) / 2);
            for (var m:int = 0; m < slots; m++)
            {
                var member:Object = m < room.members.length ? room.members[m] : null;
                var mx:Number = (m % 4) * colW;
                var my:Number = int(m / 4) * rowH;
                var under:Shape = new Shape();
                under.graphics.beginFill(0x1A232B, 1);
                under.graphics.drawRect(mx, my + rowH - 1, colW - 10, 1);
                under.graphics.endFill();
                inner.addChild(under);
                if (member == null)
                {
                    look.text(inner, win.say("open"), mx + 34, my + mid, colW - 50, 15, 0x4D5C67);
                    continue;
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
                    "player": m + 20 * (int(room.serial) % 495)});
                look.text(inner, String(member.gear), mx, my + mid + 1, 26, 14, member.fit == 2 ? RoomsLook.RED : RoomsLook.DIM, "right");
                // A red mark: this kit cannot play the next match as it is
                // now; the match starts without it and takes it out of the room.
                var nameX:Number = mx + 34;
                if (member.fit != 0)
                {
                    var mark:Shape = new Shape();
                    mark.graphics.beginFill(RoomsLook.RED, 1);
                    mark.graphics.drawCircle(mx + 40, my + mid + 10, 7);
                    mark.graphics.endFill();
                    inner.addChild(mark);
                    look.text(inner, "!", mx + 33, my + mid + 1, 14, 12, 0xFFFFFF, "center");
                    nameX = mx + 52;
                }
                // A long name cut to its place, the owner's tag kept whole.
                // Away (its game gone for now): grey until it is back, or out
                // of the room after about 90 s (`memberColour`).
                RoomsLook.fitName(look.text(inner, "", nameX, my + mid, colW - 80 - (nameX - mx - 34), 16, RoomsLook.memberColour(member)),
                    member.name, member.owner ? "  (" + win.say("ownerTag") + ")" : "");
                // Only a private room (a PIN) turns a member out.
                if (room.owner && room.pin && !member.me)
                    look.button(inner, "×", mx + colW - 44, my + int((rowH - 28) / 2), 30, onKick, m, 28);
            }
            // A room of a lower max level that takes us: one click there
            // (the server offers it only while we play no match).
            var buttons:Array = [[win.say("leave"), onLeave, null]];
            if (room.joinIn >= 0)
                buttons.push([win.say("joinMatch"), onJoinMatch, null]);
            if (room.switchTo != null)
            {
                var to:Object = room.switchTo;
                buttons.push([win.say("switchTo"), onSwitch, to]);
                var made:Array = look.footer(buttons);
                // Yellow as the Rooms tab, its label white as LEAVE ROOM's
                // (the owner's choice of 2026-10-20, "art tinted, white text").
                _switchBtn = made[made.length - 1];
                RoomsLook.skinTint(_switchBtn, RoomsLook.YELLOW_TINT);
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

        private function myFit():int
        {
            if (win._room != null)
                for (var i:int = 0; i < win._room.members.length; i++)
                    if (win._room.members[i].me)
                        return win._room.members[i].fit;
            return 0;
        }

        // Each frame: the skin redraws its parts on hover; the switch's yellow
        // is put back.
        internal function keepSwitchTint():void
        {
            if (_switchBtn != null)
                RoomsLook.skinTint(_switchBtn, RoomsLook.YELLOW_TINT);
        }

        internal function alertNear():void
        {
            if (win._room == null || win._room.phase != 3 || _alerted)
                return;
            var left:int = win._room.secs - int((getTimer() - win._roomAt) / 1000);
            if (left <= ALERT_SECS)
            {
                _alerted = true;
                RoomsLook.gameSound(6);
            }
        }

        // The countdown, kept moving between the server's answers.
        internal function banner():void
        {
            if (win._room == null || _banner == null || _banner.parent != look._content)
                return;
            var passed:int = int((getTimer() - win._roomAt) / 1000);
            var fit:int = myFit();
            var unfit:String = fit == 1 ? win.say("unfitKit") : fit == 2 ? win.say("unfitGear") : "";
            if (win._room.phase == 3)
            {
                _bannerLabel.text = win.say("voting").toUpperCase();
                _banner.text = RoomsText.clock(win._room.secs - passed);
                _bannerHint.text = unfit != "" ? unfit : win.say("voteNow");
            }
            else if (win._room.phase == 1)
            {
                _bannerLabel.text = win.say("starts").toUpperCase();
                _banner.text = RoomsText.clock(win._room.secs - passed);
                var e:Object = win.entry(win._room.next);
                _bannerHint.text = unfit != "" ? unfit : (e != null ? win.say("nextMap") + ": " + win.levelName(e.level) + " · "
                    + win.weatherName(e.weather) + " · " + win.modeName(e.mode) : "");
            }
            else if (win._room.phase == 2)
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

        // The room offered of a lower max level: joining it leaves ours.
        private function onSwitch(e:Event):void
        {
            var to:Object = e.currentTarget.data;
            if (to == null || win._steps.length > 0)
                return;
            win.busy(e);
            win._code = 0;
            win.ask(RoomsWire.JOIN + to.key, 1);
            win._fastUntil = getTimer() + 5000;
        }

        private function onVote(e:Event):void
        {
            RoomsLook.clickSound();
            win.ask(RoomsWire.VOTE + Math.max(0, Math.min(2, int(e.currentTarget.name))), 1);
        }

        private function onKick(e:Event):void
        {
            // The place in the list this state gave, and that state's serial:
            // a kick from an older list does nothing.
            if (win._room != null)
                win.ask(RoomsWire.KICK + int(e.currentTarget.data) + 20 * (int(win._room.serial) % 495), 1);
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
