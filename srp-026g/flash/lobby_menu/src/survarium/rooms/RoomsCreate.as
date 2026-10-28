package survarium.rooms
{
    import flash.display.Bitmap;
    import flash.display.DisplayObject;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.text.TextField;
    import flash.utils.getTimer;
    import survarium.player.LocalPlayerProfile;
    import survarium.controls.PlayerInfo;
    import survarium.constants.GameMode;
    import survarium.constants.RoomCode;

    // Making a private room: required PIN, modes, and the pool's map and
    // weather tiles. It opens as
    // the player's last room made by hand, which the server keeps (`S1;F`,
    // the owner's of 2026-10-20) and the window has had since the lobby
    // came up (RoomsWindow._form; the owner's of 2026-10-25: opened filled,
    // nothing applied after it is drawn); the defaults until then.
    public class RoomsCreate
    {
        private static const FORM_W:Number = 420;
        // The protective device's three kinds together (the owner, 2026-10-26), then team deathmatch and Research.
        private static const MODE_ORDER:Array = [GameMode.GATHER_VICTORY_ITEMS, GameMode.VICTORY_ITEMS_SPAWN, GameMode.BLACKOUT,
            GameMode.TEAM_DEATHMATCH, GameMode.RESEARCH];
        private static const TRACK_X:Number = 14;
        // The wait index sent when the wait is none of WAITS: 30 s.
        private static const WAIT_INDEX_DEFAULT:int = 3;
        // The window's defaults, the server's (room_settings::DEFAULT_*; a
        // Rust test keeps them the same): 8 v 8, from 1 v 1, any max level,
        // 30 s; every mode, the pool without the experimental maps.
        public static const DEFAULT_SIZE:int = 8;
        public static const DEFAULT_MIN_SIDE:int = 1;
        public static const DEFAULT_WAIT:int = 30;

        private var win:RoomsWindow;
        private var look:RoomsLook;
        private var _size:int = DEFAULT_SIZE;
        // The players a side the room needs before its vote: 1 v 1 to 3 v 3.
        private var _minSide:int = DEFAULT_MIN_SIDE;
        private var _max:int = RoomsWire.LEVEL_ANY;
        private var _wait:int = DEFAULT_WAIT;
        // The pool's tiles out (by "level_weather") and the modes out.
        private var _off:Object = {};
        private var _modesOff:Object = {};
        // What the create on its way makes (`kept`): the window's start
        // once its room is made (`made`, Back pressed meanwhile too); none
        // once it is refused or Play chosen instead (`notMade`).
        private var _making:Object = null;
        internal var _makePin:TextField;
        // What was typed in it, kept across redraws (a tile clicked, a mode).
        internal var _makePinText:String = "";
        private var _createButton:Object;

        public function RoomsCreate(win:RoomsWindow)
        {
            super();
            this.win = win;
            look = win.look;
        }

        internal function onNew(e:Event):void
        {
            // The maintenance rule: no room made (the server's; presets.md).
            if (!LocalPlayerProfile.mayCreate())
                return;
            win._openedByPlayAt = 0;
            win._view = "create";
            win._code = RoomCode.NONE;
            look._scrollPx = 0;
            _makePinText = "";
            if (win._poolTotal < 0)
                win.ask(RoomsWire.POOL, RoomsWire.POOL_ASK_ROWS);
            start(win._form);
            win.render();
        }

        // The window's start: the settings of the player's last room made
        // by hand (`form`, RoomsWindow._form), or the defaults while the
        // window has none. Not the PIN. A max level under the player's
        // equipment now is raised to it, as the server's form does.
        private function start(form:Object):void
        {
            if (form == null)
            {
                _size = DEFAULT_SIZE;
                _minSide = DEFAULT_MIN_SIDE;
                _max = RoomsWire.LEVEL_ANY;
                _wait = DEFAULT_WAIT;
                _modesOff = {};
                defaultPool();
                return;
            }
            _size = form.size;
            _minSide = form.min;
            _wait = form.wait;
            _max = form.max;
            if (_max != RoomsWire.LEVEL_ANY && _max < win._gear)
                _max = Math.min(win._gear, RoomsWire.LEVEL_MAX);
            _off = copy(form.off);
            _modesOff = copy(form.modesOff);
        }

        private static function copy(set:Object):Object
        {
            var out:Object = {};
            for (var key:String in set)
                out[key] = set[key];
            return out;
        }

        // `S1;F` as the window keeps it (RoomsWindow._form), read against
        // the pool: the sliders (a value out of a slider's range: its
        // default), the modes out, and a tile out unless an entry of it is
        // in (`bits`: entry `index` by bit `index % 20` of chunk `index / 20`).
        internal function formOf(head:Array, bits:Array):Object
        {
            var size:int = int(head[3]);
            var min:int = int(head[4]);
            var max:int = int(head[5]);
            var wait:int = int(head[6]);
            var modesOff:Object = {};
            if (head.length > 7 && head[7] != "")
            {
                var modes:Array = String(head[7]).split(",");
                for (var m:int = 0; m < modes.length; m++)
                    modesOff[String(int(modes[m]))] = true;
            }
            var on:Object = {};
            var i:int;
            for (i = 0; i < win._pool.length; i++)
            {
                var p:Object = win._pool[i];
                var chunk:uint = uint(bits[int(p.index / RoomsWire.BITS_PER_CHUNK)]);
                if ((chunk & (1 << (p.index % RoomsWire.BITS_PER_CHUNK))) != 0)
                    on[p.level + "_" + p.weather] = true;
            }
            var off:Object = {};
            for (i = 0; i < win._pool.length; i++)
            {
                var key:String = win._pool[i].level + "_" + win._pool[i].weather;
                if (on[key] != true)
                    off[key] = true;
            }
            return {"size": stops("size").indexOf(size) >= 0 ? size : DEFAULT_SIZE,
                "min": stops("min").indexOf(min) >= 0 ? min : DEFAULT_MIN_SIDE,
                "max": max >= 0 && max <= RoomsWire.LEVEL_MAX ? max : RoomsWire.LEVEL_ANY,
                "wait": stops("wait").indexOf(wait) >= 0 ? wait : DEFAULT_WAIT,
                "modesOff": modesOff, "off": off};
        }

        // What the server keeps of the room being made (room_settings::
        // of_room), as `formOf` reads it back: a tile in when an entry of it
        // is in the pool made, a mode in when an entry of it is.
        private function kept():Object
        {
            var tileIn:Object = {};
            var modeIn:Object = {};
            var i:int;
            for (i = 0; i < win._pool.length; i++)
            {
                var p:Object = win._pool[i];
                if (!chosen(p))
                    continue;
                tileIn[p.level + "_" + p.weather] = true;
                modeIn[String(p.mode)] = true;
            }
            var off:Object = {};
            var modesOff:Object = {};
            for (i = 0; i < win._pool.length; i++)
            {
                var e:Object = win._pool[i];
                var key:String = e.level + "_" + e.weather;
                if (tileIn[key] != true)
                    off[key] = true;
                if (modeIn[String(e.mode)] != true)
                    modesOff[String(e.mode)] = true;
            }
            return {"size": _size, "min": _minSide, "max": _max, "wait": _wait, "modesOff": modesOff, "off": off};
        }

        // RoomsWindow: a room new to us, ours, came while a create was on
        // its way. The server keeps the settings it was made with (room_remember):
        // the window's next start, kept with them.
        internal function made():void
        {
            if (_making != null)
                win._form = _making;
            _making = null;
        }

        // RoomsWindow: the create was refused (a code), or Play chosen
        // instead: nothing was kept.
        internal function notMade():void
        {
            _making = null;
        }

        private function onBack(e:Event):void
        {
            win._view = "list";
            win._code = RoomCode.NONE;
            look._scrollPx = 0;
            win.render();
        }

        internal function render():void
        {
            var y:Number = 50;
            look.text(look._content, win.say("name").toUpperCase(), 14, y, 100, 12, RoomsLook.DIM);
            look.text(look._content, myName(), 110, y - 3, FORM_W - 110, 17, RoomsLook.LIGHT);
            y += 30;
            look.text(look._content, win.say("pin").toUpperCase(), 14, y + 4, 100, 12, RoomsLook.DIM);
            _makePin = look.input(look._content, 110, y, _makePinText);
            _makePin.addEventListener(Event.CHANGE, onPinChange, false, 0, true);
            look.text(look._content, win.say("pinHint"), 190, y + 4, FORM_W - 190, 14, RoomsLook.GREY);
            y += 38;
            // Only private rooms can be made by hand; public rooms grow
            // with Play's demand. A private room is started by its owner.
            look.text(look._content, win.say("privateRoom"), TRACK_X, y + 4, FORM_W - TRACK_X, 14, RoomsLook.GOLD);
            y += 48;
            // The modes the server's pool offers, in MODE_ORDER: the three kinds
            // of protective device apart (plain, chargers, blackout), team
            // deathmatch and Research; a mode out takes all its entries out of
            // the pool. Rows of 24 px: four fit where three of 30 did.
            look.text(look._content, win.say("modes").toUpperCase(), 14, y, 260, 12, RoomsLook.DIM);
            y += 22;
            var offered:Array = poolModes();
            for (var mi:int = 0; mi < offered.length; mi++)
            {
                var mode:int = offered[mi];
                var toggle:Sprite = new Sprite();
                toggle.x = 14;
                toggle.y = y - 14;
                var area:Shape = new Shape();
                area.graphics.beginFill(0x000000, 0.01);
                area.graphics.drawRect(0, 10, FORM_W - 28, 22);
                area.graphics.endFill();
                toggle.addChild(area);
                toggle.addChild(look.box(_modesOff[String(mode)] != true));
                look.text(toggle, win.modeName(mode), 26, 12, FORM_W - 40, 16, RoomsLook.LIGHT);
                toggle.buttonMode = true;
                toggle.mouseChildren = false;
                toggle.name = String(mode);
                toggle.addEventListener(MouseEvent.CLICK, onModeToggle, false, 0, true);
                look.route(toggle, function(control:DisplayObject, e:MouseEvent):void
                {
                    flipMode(control.name);
                });
                look._content.addChild(toggle);
                y += 24;
            }
            win.message(look._h - RoomsLook.FOOT_H - 30);
            var made:Array = look.footer([[win.say("back"), onBack, null], [win.say("create"), onCreate, null]]);
            // No map chosen: nothing to make, the button is off.
            _createButton = made[1];
            _createButton.enabled = poolChosen() && _makePinText.length == RoomsWire.PIN_DIGITS;
            renderPool();
        }

        // Validate the PIN without replacing the focused input.
        private function onPinChange(e:Event):void
        {
            if (_makePin == null)
                return;
            _makePinText = _makePin.text;
            if (_createButton != null)
                _createButton.enabled = poolChosen() && _makePinText.length == RoomsWire.PIN_DIGITS;
        }

        private function myName():String
        {
            try
            {
                return String(PlayerInfo.nickname);
            }
            catch (e:Error)
            {
            }
            return "";
        }

        // The pool: a row a map, its checkbox and name, then a tile a weather
        // with that weather's render; a tile takes all the modes of its map
        // and weather.
        private function renderPool():void
        {
            var x0:Number = FORM_W + 24;
            var w:Number = look._w - x0 - 14;
            var frame:Shape = new Shape();
            frame.graphics.lineStyle(1, 0x26323C);
            frame.graphics.beginFill(0x080C10, 0.78);
            frame.graphics.drawRect(x0, 46, w, look._h - RoomsLook.FOOT_H - 52);
            frame.graphics.endFill();
            look._content.addChild(frame);
            var tiles:Array = poolTiles();
            var on:int = 0;
            for (var t:int = 0; t < tiles.length; t++)
                if (tiles[t].on)
                    on++;
            look.text(look._content, win.say("pool").toUpperCase() + ": " + on + " / " + tiles.length,
                x0 + 16, 58, 260, 16, RoomsLook.LIGHT);
            look.text(look._content, win.say("poolHint"), x0 + 230, 60, w - 430, 14, RoomsLook.GREY);
            look.button(look._content, win.say("all"), x0 + w - 190, 54, 84, onAll, true);
            look.button(look._content, win.say("nothing"), x0 + w - 100, 54, 84, onAll, false);
            // The maps in the pool's order, each with its weathers.
            var rows:Array = [];
            var at:Object = {};
            for (var i:int = 0; i < tiles.length; i++)
            {
                var tile:Object = tiles[i];
                var row:Object = at[String(tile.level)];
                if (row == null)
                {
                    row = {"level": tile.level, "tiles": []};
                    at[String(tile.level)] = row;
                    rows.push(row);
                }
                row.tiles.push(tile);
            }
            // The map's name on its own line, its weathers' tiles under it,
            // as wide as the pool lets three be.
            var top:Number = 94;
            var areaW:Number = w - 40;
            var cw:int = int((areaW - 4 - 2 * 10) / 3);
            var ch:int = int(cw * 9 / 16);
            var rowH:int = 28 + ch + 14;
            var area:Sprite = look.scrollBox(x0 + 16, top, areaW, look._h - RoomsLook.FOOT_H - 6 - top - 8);
            var y:Number = 0;
            for (var r:int = 0; r < rows.length; r++)
            {
                var map:Object = rows[r];
                var any:Boolean = false;
                for (var c:int = 0; c < map.tiles.length; c++)
                    if (map.tiles[c].on)
                        any = true;
                var check:Sprite = new Sprite();
                check.x = 0;
                check.y = y - 12;
                // The whole line takes the click, the gap between the box and
                // the name too.
                var line_hit:Shape = new Shape();
                line_hit.graphics.beginFill(0x000000, 0.01);
                line_hit.graphics.drawRect(0, 10, areaW - 4, 26);
                line_hit.graphics.endFill();
                check.addChild(line_hit);
                check.addChild(look.box(any));
                look.text(check, win.levelName(map.level), 24, 13, areaW - 40, 16, RoomsLook.LIGHT);
                check.buttonMode = true;
                check.mouseChildren = false;
                check.name = String(map.level);
                check.addEventListener(MouseEvent.CLICK, onMapRow, false, 0, true);
                area.addChild(check);
                for (var k:int = 0; k < map.tiles.length; k++)
                {
                    var one:Object = map.tiles[k];
                    var chip:Sprite = new Sprite();
                    chip.x = 2 + k * (cw + 10);
                    chip.y = y + 28;
                    var pic:Bitmap = look.mapRender(one.level, one.weather, cw, ch);
                    pic.alpha = one.on ? 1 : 0.35;
                    chip.addChild(pic);
                    var edge:Shape = new Shape();
                    edge.graphics.beginFill(0x000000, 0.66);
                    edge.graphics.drawRect(0, ch - 22, cw, 22);
                    edge.graphics.endFill();
                    edge.graphics.lineStyle(2, one.on ? RoomsLook.GOLD : 0x26323C);
                    edge.graphics.drawRect(0, 0, cw, ch);
                    chip.addChild(edge);
                    look.text(chip, win.weatherName(one.weather).toUpperCase() + (one.extra ? " · " + win.say("experimental").toUpperCase() : ""),
                        0, ch - 22 + 3, cw, 12, 0xFFFFFF, "center");
                    chip.buttonMode = true;
                    chip.mouseChildren = false;
                    chip.name = one.key;
                    chip.addEventListener(MouseEvent.CLICK, onTile, false, 0, true);
                    area.addChild(chip);
                }
                look.line(area, 0, y + rowH - 6, areaW - 4);
                y += rowH;
            }
            look.scrollDone(y);
        }

        private function onModeToggle(e:Event):void
        {
            flipMode(e.currentTarget.name);
        }

        private function flipMode(mode:String):void
        {
            RoomsLook.clickSound();
            _modesOff[mode] = _modesOff[mode] != true;
            win.render();
        }

        private function onMapRow(e:Event):void
        {
            RoomsLook.clickSound();
            var level:String = e.currentTarget.name;
            var tiles:Array = poolTiles();
            var any:Boolean = false;
            var i:int;
            for (i = 0; i < tiles.length; i++)
                if (String(tiles[i].level) == level && tiles[i].on)
                    any = true;
            for (i = 0; i < tiles.length; i++)
                if (String(tiles[i].level) == level)
                    _off[tiles[i].key] = any;
            win.render();
        }

        // A slider's places: the values it stops at, in order.
        private function stops(what:String):Array
        {
            var list:Array = [];
            if (what == "size")
                list = [6, 7, 8, 9, 10];
            else if (what == "min")
                list = [1, 2, 3];
            else if (what == "wait")
                list = RoomsWire.WAITS.concat();
            else
            {
                // Any, then the player's own level up to 15: never below it.
                list.push(RoomsWire.LEVEL_ANY);
                for (var m:int = Math.max(1, win._gear); m <= RoomsWire.LEVEL_MAX; m++)
                    list.push(m);
            }
            return list;
        }

        // The modes of the pool's entries, in MODE_ORDER.
        private function poolModes():Array
        {
            var modes:Array = [];
            for (var m:int = 0; m < MODE_ORDER.length; m++)
                for (var i:int = 0; i < win._pool.length; i++)
                    if (int(win._pool[i].mode) == MODE_ORDER[m])
                    {
                        modes.push(MODE_ORDER[m]);
                        break;
                    }
            return modes;
        }

        // The pool's entries by map and weather: a tile takes all its modes.
        private function poolTiles():Array
        {
            var tiles:Array = [];
            var at:Object = {};
            for (var i:int = 0; i < win._pool.length; i++)
            {
                var e:Object = win._pool[i];
                var key:String = e.level + "_" + e.weather;
                var tile:Object = at[key];
                if (tile == null)
                {
                    tile = {"key": key, "level": e.level, "weather": e.weather, "extra": e.extra,
                        "entries": [], "on": _off[key] != true};
                    at[key] = tile;
                    tiles.push(tile);
                }
                tile.entries.push(e);
            }
            return tiles;
        }

        // The default pool: the server's, the experimental maps out.
        internal function defaultPool():void
        {
            _off = {};
            for (var i:int = 0; i < win._pool.length; i++)
                if (win._pool[i].extra)
                    _off[win._pool[i].level + "_" + win._pool[i].weather] = true;
        }

        // Whether the pool as chosen has an entry a create can name.
        private function poolChosen():Boolean
        {
            for (var i:int = 0; i < win._pool.length; i++)
                if (chosen(win._pool[i]))
                    return true;
            return false;
        }

        // Whether a pool entry is in the room made: its tile and its mode
        // in, and a bit a create can name it by.
        private function chosen(p:Object):Boolean
        {
            return _off[p.level + "_" + p.weather] != true && _modesOff[String(p.mode)] != true && p.index < RoomsWire.POOL_BITS;
        }

        private function onAll(e:Event):void
        {
            var on:Boolean = e.currentTarget.data == true;
            var tiles:Array = poolTiles();
            for (var i:int = 0; i < tiles.length; i++)
                _off[tiles[i].key] = !on;
            win.render();
        }

        private function onTile(e:Event):void
        {
            RoomsLook.clickSound();
            var key:String = e.currentTarget.name;
            _off[key] = _off[key] != true;
            win.render();
        }

        private function onCreate(e:Event):void
        {
            // One at a time: its PIN, pool and create are still on their way.
            if (win._steps.length > 0)
                return;
            win.busy(e);
            var typedPin:String = _makePin != null ? _makePin.text : "";
            if (typedPin.length != RoomsWire.PIN_DIGITS)
            {
                win.pinRefused();
                return;
            }
            win._code = RoomCode.NONE;
            win._steps = [];
            // The pool in pieces of 20 entries, each in by its bit.
            var chunks:Array = [0, 0, 0, 0, 0, 0, 0, 0];
            var any:Boolean = false;
            for (var i:int = 0; i < win._pool.length; i++)
            {
                var p:Object = win._pool[i];
                if (!chosen(p))
                    continue;
                chunks[int(p.index / RoomsWire.BITS_PER_CHUNK)] |= 1 << (p.index % RoomsWire.BITS_PER_CHUNK);
                any = true;
            }
            var used:int = win._pool.length > 0 ? int((win._pool.length + (RoomsWire.BITS_PER_CHUNK - 1)) / RoomsWire.BITS_PER_CHUNK) : 1;
            for (var c:int = 0; c < used && c < RoomsWire.CHUNKS; c++)
                win._steps.push(RoomsWire.BITS + c * RoomsWire.CHUNK_STEP + chunks[c]);
            var wait:int = RoomsWire.WAITS.indexOf(_wait);
            if (wait < 0)
                wait = WAIT_INDEX_DEFAULT;
            // Keep the form's legacy fields on the wire; the server ignores
            // them for a private room and remembers its map/mode selection.
            var code:int = ((((_size - RoomsWire.SIZE_MIN) * RoomsWire.LEVELS + _max) * RoomsWire.WAIT_COUNT + wait) * 2 + 1) * RoomsWire.MIN_SIDES + (_minSide - 1);
            win._steps.push(RoomsWire.CREATE + code);
            win._steps.unshift(RoomsWire.PIN + int(typedPin));
            if (!any)
            {
                win._steps = [];
                win.render();
                return;
            }
            _making = kept();
            win._fastUntil = getTimer() + 5000;
            win.next();
        }
    }
}
