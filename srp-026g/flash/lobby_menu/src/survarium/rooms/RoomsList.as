package survarium.rooms
{
    import flash.display.Bitmap;
    import flash.display.DisplayObject;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.KeyboardEvent;
    import flash.events.MouseEvent;
    import flash.text.TextField;
    import flash.utils.getTimer;
    import scaleform.gfx.FocusManager;
    import scaleform.gfx.MouseEventEx;
    import flash.ui.Keyboard;
    import survarium.constants.RoomCode;
    import survarium.constants.RoomFit;
    import survarium.constants.RoomPhase;
    import survarium.constants.RoomSide;
    import survarium.player.LocalPlayerProfile;

    // The list, after the design: the table of the rooms on the left, the
    // room chosen on the right with its map's render, its facts and its
    // members (`S1;G`, the owner's of 2026-10-20: "down there is enough
    // space for a list of players"), and JOIN (Queue for a full room).
    public class RoomsList
    {
        private static const SIDE_W:Number = 330;
        private static const MEMBERS_W:Number = SIDE_W - 32;
        // A side's bar while the room plays its match.
        private static const SIDE_COLOURS:Array = [0x5A8FB5, 0xC98A3C];

        private var win:RoomsWindow;
        private var look:RoomsLook;
        internal var _selKey:int = 0;
        // The last row pressed, and when: a second press on it soon joins.
        private var _rowKey:int = 0;
        private var _rowAt:int = 0;
        // Its rows' state cells by room key (and the side panel's), so the
        // times change in place.
        internal var _stateCells:Object = {};
        // A right-click on a member of the side panel: the game's
        // right-click menu with "Show player's information", and its window
        // with the member's kits (LobbyMenu.showPlayerInfoMenu, the
        // player-gear feature); the request names the member by its account
        // (`S1;G`).
        internal var _listHits:Array = [];
        internal var _membersBox:Sprite = null;
        // The height the list has below the facts.
        private var _membersH:Number = 150;
        // The PIN field of a room with one, and what was typed in it, kept
        // across redraws.
        internal var _joinPin:TextField;
        internal var _pinText:String = "";
        // The friend's room whose PIN is asked (0: none), and when: its field
        // takes the keys once drawn in the next seconds (the list asked for
        // comes then).
        private var _pinFor:int = 0;
        private var _pinForAt:int = 0;

        public function RoomsList(win:RoomsWindow)
        {
            super();
            this.win = win;
            look = win.look;
        }

        internal function render():void
        {
            var tableW:Number = look._w - SIDE_W - 42;
            look.text(look._content, win.say("st_srp_rooms_servers") + ": " + win._total, 14, 52, 200, 16, RoomsLook.LIGHT);
            win.message(52);
            if (win._code != RoomCode.NONE)
                look._content.getChildAt(look._content.numChildren - 1).x = 220;
            else if (win._queueKey != 0)
            {
                // Waiting in a full room's queue: our place and its name.
                var queued:String = "";
                for (var qi:int = 0; qi < win._rooms.length; qi++)
                    if (win._rooms[qi].key == win._queueKey)
                        queued = " · " + win._rooms[qi].name;
                look.text(look._content, win.say("st_srp_rooms_in_queue") + ": " + win._queuePlace + queued, 220, 52, 360, 16, RoomsLook.GOLD);
            }
            var heads:Array = ["st_srp_rooms_name", "st_srp_rooms_level", "st_srp_rooms_max", "st_srp_rooms_map",
                "st_srp_rooms_players", "st_srp_rooms_state"];
            // The headers at their rows' x, each as wide as its column: the
            // rows are in the scrolled box (at 14, tableW - 12 wide), the
            // name 10 in.
            for (var h:int = 0; h < heads.length; h++)
                look.text(look._content, win.say(heads[h]).toUpperCase(), 14 + columnX(h, tableW - 12) + (h == 0 ? 10 : 0), 84,
                    columnW(h, tableW - 12) - (h == 0 ? 10 : 0), 12, RoomsLook.DIM);
            look.line(look._content, 14, 106, tableW - 12);
            var inner:Sprite = look.scrollBox(14, 108, tableW - 12, look._h - RoomsLook.FOOT_H - 108 - 8);
            var sel:Object = null;
            if (win._rooms.length == 0)
                look.text(inner, win.say(win.playLit() ? "st_srp_rooms_none" : LocalPlayerProfile.mayCreate() ? "st_srp_rooms_none_no_play" : "st_srp_rooms_none_quiet"), 0, 14, tableW - 20, 16, RoomsLook.GREY);
            for (var i:int = 0; i < win._rooms.length; i++)
            {
                var room:Object = win._rooms[i];
                if (_selKey == 0)
                    _selKey = room.key;
                var on:Boolean = room.key == _selKey;
                if (on)
                    sel = room;
                var row:Sprite = new Sprite();
                row.y = i * 42;
                row.graphics.beginFill(on ? 0x3C2C10 : 0x000000, on ? 0.6 : 0.01);
                row.graphics.drawRect(0, 0, tableW - 12, 41);
                row.graphics.endFill();
                if (on)
                {
                    row.graphics.beginFill(RoomsLook.GOLD, 1);
                    row.graphics.drawRect(0, 0, 3, 41);
                    row.graphics.endFill();
                }
                row.graphics.beginFill(0x1E2830, 1);
                row.graphics.drawRect(0, 41, tableW - 12, 1);
                row.graphics.endFill();
                var tw:Number = tableW - 12;
                // Our own room (the All rooms tab) in the yellow of a member's
                // Rooms tab.
                var mine:Boolean = win._room != null && room.key == win._room.key;
                // A long name cut to its column, as the members' names are.
                RoomsLook.fitName(look.text(row, "", 10, 10, columnW(0, tw) - 10, 17, mine ? RoomsLook.YELLOW : on ? 0xFFFFFF : RoomsLook.LIGHT),
                    (room.pin ? "PIN · " : "") + room.name, "");
                look.text(row, String(room.level), columnX(1, tw), 10, columnW(1, tw), 17, RoomsLook.LIGHT);
                // A private room has no max level: its cell empty.
                look.text(row, room.pin ? "" : win.levelText(room.max), columnX(2, tw), 10, columnW(2, tw), 17, RoomsLook.LIGHT);
                var e:Object = win.entry(room.map);
                // The next map, its weather under it; no card leads alone yet
                // (a tie, no votes): the start draws one.
                if (e != null)
                {
                    RoomsLook.fitName(look.text(row, "", columnX(3, tw), 4, columnW(3, tw), 14, RoomsLook.GREY), win.levelName(e.level), "");
                    look.text(row, win.weatherName(e.weather), columnX(3, tw), 21, columnW(3, tw), 12, RoomsLook.DIM);
                }
                else
                    look.text(row, room.phase == RoomPhase.VOTE ? win.say("st_srp_rooms_voting") : "", columnX(3, tw), 10, columnW(3, tw), 15,
                        RoomsLook.GREY);
                look.text(row, room.players + " / " + room.size * 2, columnX(4, tw), 10, columnW(4, tw), 17, RoomsLook.LIGHT);
                _stateCells[String(room.key)] = look.text(row, win.phaseText(room), columnX(5, tw), 10, columnW(5, tw), 15,
                    room.phase == RoomPhase.IN_MATCH ? RoomsLook.GOLD : room.phase == RoomPhase.VOTE || room.phase == RoomPhase.COUNTDOWN ? 0x8FD18F : RoomsLook.GREY);
                row.buttonMode = true;
                row.mouseChildren = false;
                row.name = String(room.key);
                row.addEventListener(MouseEvent.CLICK, onRow, false, 0, true);
                inner.addChild(row);
                // A press the content got itself (as on the left side) picks
                // the row too; only where the list shows it (not scrolled away
                // under its mask).
                look.route(row, function(control:DisplayObject, e:MouseEvent):void
                {
                    var shown:DisplayObject = (control.parent as Sprite).mask;
                    if (shown != null && !shown.hitTestPoint(e.stageX, e.stageY, false))
                        return;
                    pickRow(int(control.name));
                });
            }
            look.scrollDone(win._rooms.length * 42);
            var joinable:Object = null;
            for (var j:int = 0; j < win._rooms.length; j++)
                if (win._rooms[j].key == _selKey)
                    joinable = win._rooms[j];
            // A member queues for no room (from no room only): Join, off for
            // a full room and for its own.
            var queuedHere:Boolean = queuedFor(joinable);
            var joinWord:String = queuedHere ? win.say("st_srp_rooms_leave_queue")
                : joinable != null && joinable.players >= joinable.size * 2 && win._room == null
                    ? win.say("st_srp_rooms_queue") : win.say("st_srp_rooms_join");
            var made:Array = look.footer(joinable != null ? [[win.say("st_srp_rooms_new"), win.createView.onNew, null], [joinWord, onJoin, joinable]]
                : [[win.say("st_srp_rooms_new"), win.createView.onNew, null]]);
            if (joinable != null && !queuedHere && (!joins(joinable) || !LocalPlayerProfile.mayEnter(joinable.max)))
            {
                made[1].enabled = false;
                // Greyed by a reason (presets.md): the reasons in its tip.
                if (joins(joinable))
                    LocalPlayerProfile.reasonTip(look._content, made[1].x, made[1].y, made[1].width, made[1].height);
            }
            // Making a room: greyed under the maintenance rule.
            if (!LocalPlayerProfile.mayCreate())
            {
                made[0].enabled = false;
                LocalPlayerProfile.reasonTip(look._content, made[0].x, made[0].y, made[0].width, made[0].height);
            }
            renderSide(sel, tableW + 28);
        }

        // The table's columns (name, level, max level, next map, players,
        // state) as shares of its width: each as wide as its header and its
        // texts are in every language (the longest map name, "Any", a vote's
        // time), the name the rest (cut to it).
        private static const COLS:Array = [0, 0.215, 0.303, 0.444, 0.731, 0.827, 1];

        private static function columnX(c:int, w:Number):Number
        {
            return w * COLS[c];
        }

        // A column's texts' width, a gap before the next.
        private static function columnW(c:int, w:Number):Number
        {
            return w * (COLS[c + 1] - COLS[c]) - 6;
        }

        // The room chosen: its next map's render, what it is, and JOIN. One
        // layout whether a map is known or not (a vote with no card ahead
        // alone, or a room before its vote, has none): the render's place
        // and the two lines under it are kept, so nothing moves as votes come.
        private function renderSide(room:Object, x:Number):void
        {
            var panel:Shape = new Shape();
            panel.graphics.lineStyle(1, 0x26323C);
            panel.graphics.beginFill(0x080C10, 0.78);
            panel.graphics.drawRect(x, 52, SIDE_W, look._h - RoomsLook.FOOT_H - 60);
            panel.graphics.endFill();
            look._content.addChild(panel);
            _membersBox = null;
            if (room == null)
                return;
            var e:Object = win.entry(room.map);
            // A private room has no max level (the owner's of 2026-10-26).
            var facts:Array = [[win.say("st_srp_rooms_level"), String(room.level)]];
            if (!room.pin)
                facts.push([win.say("st_srp_rooms_max"), win.levelText(room.max)]);
            facts.push([win.say("st_srp_rooms_players"), room.players + " / " + room.size * 2], [win.say("st_srp_rooms_state"), win.phaseText(room)]);
            // Its members below the facts (the owner's of 2026-10-20, "below
            // state"): the map's render made up to 60 lower when the panel is
            // short for the rows its players need at 18 each (two columns).
            var bottom:Number = look._h - RoomsLook.FOOT_H - 14;
            var above:Number = 12 + 30 + 48 + facts.length * 26 + (room.pin ? 36 : 0) + 10;
            var need:int = memberRows(room) * 18;
            var picH:int = 150 - int(Math.max(0, Math.min(60, need - (bottom - 52 - 150 - above))));
            if (e != null)
            {
                // Made (and kept) at its full height, its middle shown: one
                // render a map and weather, whatever the room's players.
                // (A mask over a Sprite, as scrollBox's.)
                var crop:Shape = new Shape();
                crop.graphics.beginFill(0x000000, 1);
                crop.graphics.drawRect(x, 52, SIDE_W, picH);
                crop.graphics.endFill();
                look._content.addChild(crop);
                var framed:Sprite = new Sprite();
                var pic:Bitmap = look.mapRender(e.level, e.weather, SIDE_W, 150);
                pic.x = x;
                pic.y = 52 - int((150 - picH) / 2);
                framed.addChild(pic);
                look._content.addChild(framed);
                framed.mask = crop;
            }
            else
            {
                // No map yet: the render's place, empty, at its size.
                var empty:Shape = new Shape();
                empty.graphics.lineStyle(1, 0x26323C);
                empty.graphics.beginFill(0x05080B, 1);
                empty.graphics.drawRect(x, 52, SIDE_W, picH);
                empty.graphics.endFill();
                look._content.addChild(empty);
            }
            var y:Number = 52 + picH + 12;
            look.text(look._content, room.name, x + 16, y, SIDE_W - 32, 20, 0xFFFFFF);
            y += 30;
            if (e != null)
            {
                // The map and its weather, the mode under them (the three
                // are too long for one line in most languages).
                look.text(look._content, win.levelName(e.level) + " · " + win.weatherName(e.weather), x + 16, y, SIDE_W - 32, 15,
                    RoomsLook.GREY);
                look.text(look._content, win.modeName(e.mode), x + 16, y + 20, SIDE_W - 32, 15, RoomsLook.GREY);
            }
            else
                look.text(look._content, win.say("st_srp_rooms_map_by_vote"), x + 16, y, SIDE_W - 32, 15, RoomsLook.GREY);
            y += 48;
            for (var i:int = 0; i < facts.length; i++)
            {
                look.text(look._content, facts[i][0], x + 16, y, 140, 15, RoomsLook.DIM);
                var cell:TextField = look.text(look._content, facts[i][1], x + 160, y, SIDE_W - 176, 15, RoomsLook.LIGHT);
                if (i == facts.length - 1)
                    _stateCells["side"] = cell;
                y += 26;
            }
            if (room.pin)
            {
                y += 6;
                look.text(look._content, win.say("st_srp_rooms_pin"), x + 16, y + 4, 60, 15, RoomsLook.DIM);
                _joinPin = look.input(look._content, x + 160, y, _pinText);
                pinAsked(room);
                y += 30;
            }
            look.line(look._content, x + 16, y + 2, SIDE_W - 32);
            y += 10;
            _membersBox = new Sprite();
            _membersBox.x = x + 16;
            _membersBox.y = y;
            look._content.addChild(_membersBox);
            _membersH = bottom - y;
            drawMembers();
        }

        // The rows the list of a room of the table needs in two columns (the
        // render above is made lower for them): half its players, or while
        // it plays a side's, the room's size at most.
        private function memberRows(room:Object):int
        {
            var n:int = Math.min(RoomsWire.MEMBERS_MAX, int(room.players));
            return Math.max(1, room.phase == RoomPhase.IN_MATCH ? Math.min(n, int(room.size)) : int((n + 1) / 2));
        }

        // The chosen room's members in two columns (one layout, however many;
        // the owner's of 2026-10-29), and while it plays its match, side 1 on the left
        // and side 2 on the right (a bar of the side's colour), those not
        // in the match where there is room. Each row: the equipment level
        // in a column of its own, the red mark of a kit that cannot play,
        // the name cut to what is left (`fitName`).
        internal function drawMembers():void
        {
            var box:Sprite = _membersBox;
            _listHits = [];
            if (box == null)
                return;
            while (box.numChildren > 0)
                box.removeChildAt(0);
            var room:Object = chosenRoom();
            if (win._view != "list" || room == null || win._seen == null || win._seen.key != room.key)
                return;
            var all:Array = win._seen.members;
            var playing:Boolean = false;
            var i:int;
            for (i = 0; i < all.length; i++)
                if (all[i].side == RoomSide.FIRST || all[i].side == RoomSide.SECOND)
                    playing = true;
            var cols:Array;
            if (!playing)
                cols = [all.slice(0, int((all.length + 1) / 2)), all.slice(int((all.length + 1) / 2))];
            else
            {
                cols = [[], []];
                var rest:Array = [];
                for (i = 0; i < all.length; i++)
                {
                    if (all[i].side == RoomSide.FIRST || all[i].side == RoomSide.SECOND)
                        cols[all[i].side - RoomSide.FIRST].push(all[i]);
                    else
                        rest.push(all[i]);
                }
                for (i = 0; i < rest.length; i++)
                    cols[cols[0].length <= cols[1].length ? 0 : 1].push(rest[i]);
            }
            var rows:int = Math.max(1, Math.max(cols[0].length, cols.length > 1 ? cols[1].length : 0));
            var rowH:int = int(Math.max(12, Math.min(22, _membersH / rows)));
            var size:int = rowH >= 20 ? 14 : rowH >= 17 ? 13 : 12;
            var colW:Number = (MEMBERS_W - 12) / 2;
            for (var c:int = 0; c < cols.length; c++)
                for (var r:int = 0; r < cols[c].length; r++)
                    // Never past the panel's bottom (a window at its least).
                    if ((r + 1) * rowH <= _membersH + 2)
                        drawMember(box, cols[c][r], c * (colW + 12), r * rowH, colW, rowH, size);
        }

        private function drawMember(box:Sprite, m:Object, x:Number, y:Number, w:Number, rowH:int, size:int):void
        {
            var row:Sprite = new Sprite();
            row.x = x;
            row.y = y;
            // The whole row takes the mouse (alpha 0 is not hit).
            row.graphics.beginFill(0x000000, 0.01);
            row.graphics.drawRect(0, 0, w, rowH);
            row.graphics.endFill();
            if (m.side == RoomSide.FIRST || m.side == RoomSide.SECOND)
            {
                row.graphics.beginFill(uint(SIDE_COLOURS[m.side - RoomSide.FIRST]), 1);
                row.graphics.drawRect(0, 2, 3, rowH - 4);
                row.graphics.endFill();
            }
            box.addChild(row);
            var top:Number = int((rowH - size - 4) / 2);
            // The level in its own column, right-aligned: never under the name.
            look.text(row, String(m.gear), 4, top + 1, 22, size - 1, m.fit == RoomFit.GEAR ? RoomsLook.RED : RoomsLook.DIM, "right");
            var nameX:Number = 30;
            if (m.fit != RoomFit.YES)
            {
                var mark:Shape = new Shape();
                mark.graphics.beginFill(RoomsLook.RED, 1);
                mark.graphics.drawCircle(37, rowH / 2, Math.min(6, rowH / 2 - 1));
                mark.graphics.endFill();
                row.addChild(mark);
                look.text(row, "!", 31, top, 12, size - 2, 0xFFFFFF, "center");
                nameX = 46;
            }
            // A squad's icon after the name: the name gives it its room.
            var iconW:Number = m.squad > 0 ? RoomsLook.SQUAD_ICON_W + 8 : 0;
            var nameField:TextField = look.text(row, "", nameX, top, w - nameX - iconW, size, RoomsLook.memberColour(m));
            RoomsLook.fitName(nameField, m.name, "");
            look.squadIcon(row, nameField, m.squad, m.squad == RoomsLook.ourSquad(win._seen.members), Math.min(14, rowH - 4));
            _listHits.push({"control": row, "name": m.name, "how": LobbyMenu.PLAYER_INFO_BY_ACCOUNT,
                "player": m.account});
        }

        // The list's times, written into its cells as drawn.
        internal function listTimes():void
        {
            for (var i:int = 0; i < win._rooms.length; i++)
            {
                var room:Object = win._rooms[i];
                var cell:TextField = _stateCells[String(room.key)] as TextField;
                if (cell != null)
                    cell.text = win.phaseText(room);
                if (room.key == _selKey && _stateCells["side"] != null)
                    TextField(_stateCells["side"]).text = win.phaseText(room);
            }
        }

        private function onRow(e:Event):void
        {
            // The left button only: two right-clicks are no join.
            var ex:MouseEventEx = e as MouseEventEx;
            if (ex != null && ex.buttonIdx != MouseEventEx.LEFT_BUTTON)
                return;
            pickRow(int(e.currentTarget.name));
        }

        // A row pressed: chosen; pressed again within 450 ms, joined (as
        // Join, Queue for a full room).
        private function pickRow(key:int):void
        {
            RoomsLook.clickSound();
            var now:int = getTimer();
            if (key == _rowKey && now - _rowAt < 450)
            {
                _rowAt = 0;
                joinRoom(chosenRoom());
                return;
            }
            _rowKey = key;
            _rowAt = now;
            _selKey = key;
            win.render();
            win.askMembers(false);
        }

        internal function chosenRoom():Object
        {
            for (var i:int = 0; i < win._rooms.length; i++)
                if (win._rooms[i].key == _selKey)
                    return win._rooms[i];
            return null;
        }

        // In the list: Enter joins the room chosen, Up and Down choose the
        // one above or below (scrolled into view). Not while something is
        // typed in another text field (a chat message); the PIN's is ours.
        internal function onKey(e:KeyboardEvent):void
        {
            try
            {
                if (!win._shown || win._view != "list" || (e.keyCode != Keyboard.ENTER && e.keyCode != Keyboard.UP && e.keyCode != Keyboard.DOWN)
                    || win.resultsUp())
                    return;
                var focus:TextField = win.stage != null ? win.stage.focus as TextField : null;
                if (focus != null && focus != _joinPin && focus.text.length > 0)
                    return;
                if (e.keyCode == Keyboard.ENTER)
                {
                    joinRoom(chosenRoom());
                    return;
                }
                var at:int = -1;
                for (var i:int = 0; i < win._rooms.length; i++)
                    if (win._rooms[i].key == _selKey)
                        at = i;
                var next:int = Math.max(0, Math.min(win._rooms.length - 1, at + (e.keyCode == Keyboard.DOWN ? 1 : -1)));
                if (win._rooms.length == 0 || next == at)
                    return;
                _selKey = win._rooms[next].key;
                // Its row (42 high) in the scrolled box's view.
                var top:Number = next * 42;
                if (top < look._scrollPx)
                    look._scrollPx = top;
                else if (top + 42 > look._scrollPx + look._scrollViewH)
                    look._scrollPx = top + 42 - look._scrollViewH;
                RoomsLook.clickSound();
                win.render();
                win.askMembers(false);
            }
            catch (err:Error)
            {
            }
        }

        private function onJoin(e:Event):void
        {
            var room:Object = e.currentTarget.data;
            if (room == null || win._steps.length > 0)
                return;
            win.busy(e);
            if (queuedFor(room))
                win.leaveQueue();
            else
                joinRoom(room);
        }

        private function queuedFor(room:Object):Boolean
        {
            return room != null && win._room == null && win._queueKey == room.key;
        }

        // Whether Join takes us there: a member (the All rooms tab) not into
        // its own room, nor a full one (its queue is from no room only).
        private function joins(room:Object):Boolean
        {
            return win._room == null || (room.key != win._room.key && room.players < room.size * 2);
        }

        // Join (or Queue for a full room) as the button does; the PIN typed
        // goes first. A member joining another room leaves its own (the
        // server's, as Leave). A join (or queue) asks first whether it offers
        // one of our kit presets (presets.md; RoomsWindow.askOffer), then
        // comes back with `preset`: 0 none, else put on before the join (or
        // queue: the server's PRESET joins a room with places, else queues).
        // Greyed by a reason (presets.md) unless the popup can offer a
        // loadout that can play (mayEnter).
        private function joinRoom(room:Object, preset:int = -1):void
        {
            // A Join of the player's own: no longer the window Play opened.
            win._openedByPlayAt = 0;
            if (room == null || win._steps.length > 0 || !joins(room) || !LocalPlayerProfile.mayEnter(room.max))
                return;
            win._busyAt = getTimer();
            win._stepsAt = win._busyAt;
            // A full room: wait in its queue (the server joins one a place
            // has freed for meanwhile).
            var start:int = room.players >= room.size * 2 ? RoomsWire.QUEUE : RoomsWire.JOIN;
            win._code = RoomCode.NONE;
            win._steps = [];
            var typed:String = _joinPin != null ? _joinPin.text : _pinText;
            if (room.pin && typed.length != RoomsWire.PIN_DIGITS)
            {
                win.pinRefused();
                return;
            }
            if (preset < 0)
            {
                win.askOffer(room);
                return;
            }
            var at:int = start + room.key;
            if (preset > 0)
                at = RoomsWire.PRESET + (preset - 1) * RoomsWire.PRESET_STEP + room.key;
            if (room.pin)
            {
                win._steps.push(at);
                win.ask(RoomsWire.PIN + int(typed), 1);
            }
            else
            {
                win.ask(at, 1);
            }
            win._fastUntil = getTimer() + 5000;
        }

        // The join of `room` after its preset offer (RoomsWindow.takeOffer).
        internal function joinWith(room:Object, preset:int):void
        {
            joinRoom(room, preset);
        }

        // A friend's room (RoomsWindow.joinFriend; `room`: {key, players,
        // size, pin}): chosen and joined as Join does; one with a PIN chosen,
        // its PIN field given the keys (typed, then Join).
        internal function joinFriend(room:Object):void
        {
            _selKey = int(room.key);
            look._scrollPx = 0;
            if (room.pin)
            {
                _pinFor = _selKey;
                _pinForAt = getTimer();
                _pinText = "";
                if (_joinPin != null)
                    _joinPin.text = "";
                win.askList();
                win.render();
                return;
            }
            win.render();
            joinRoom(room);
        }

        // The PIN field of the friend's room drawn: typed in first.
        private function pinAsked(room:Object):void
        {
            if (_pinFor == 0 || room.key != _pinFor || _joinPin == null)
                return;
            if (getTimer() - _pinForAt > 3000)
            {
                _pinFor = 0;
                return;
            }
            FocusManager.setFocus(_joinPin);
        }
    }
}
