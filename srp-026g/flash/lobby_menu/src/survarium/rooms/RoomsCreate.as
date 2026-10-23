package survarium.rooms
{
    import flash.display.Bitmap;
    import flash.display.DisplayObject;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.geom.Point;
    import flash.text.TextField;
    import flash.utils.getTimer;
    import survarium.player.LocalPlayerProfile;
    import survarium.controls.PlayerInfo;
    import survarium.constants.GameMode;
    import survarium.constants.RoomCode;

    // Making a room: size, players to start, max level, wait, PIN, the
    // modes, then the pool's tiles (a map and weather each). It starts as
    // the player's last room made by hand, which the server keeps (`S1;F`,
    // the owner's of 2026-10-20).
    public class RoomsCreate
    {
        private static const FORM_W:Number = 420;
        private static const MODE_ORDER:Array = [GameMode.GATHER_VICTORY_ITEMS, GameMode.VICTORY_ITEMS_SPAWN, GameMode.TEAM_DEATHMATCH,
            GameMode.RESEARCH];
        // A slider after the design: the label and the gold value on one
        // line, a thin track gold up to a round gold knob, its range small
        // under it, a line between one slider and the next. Dragged or clicked; while dragged only the
        // slider is redrawn.
        private static const TRACK_X:Number = 14;
        private static const TRACK_W:Number = 406;
        private static const KNOB_R:Number = 8;
        private static const SLIDER_H:Number = 64;
        // The wait index sent when the wait is none of WAITS: 30 s.
        private static const WAIT_INDEX_DEFAULT:int = 3;

        private var win:RoomsWindow;
        private var look:RoomsLook;
        private var _size:int = 8;
        // The players a side the room needs before its vote: 1 v 1 to 3 v 3.
        private var _minSide:int = 1;
        private var _max:int = RoomsWire.LEVEL_ANY;
        private var _wait:int = 30;
        // The pool's tiles out (by "level_weather") and the modes out.
        private var _off:Object = {};
        private var _modesOff:Object = {};
        internal var _sliders:Object = {};
        private var _drag:String = null;
        // The create window as the server says it starts (`_saved`, the
        // player's last room made by hand, fitted to the pool and its
        // equipment now): the sliders, the modes, and a tile out unless an
        // entry of it is in. A value out of a slider's range leaves it as it is.
        internal var _saved:Object = null;
        internal var _makePin:TextField;
        // What was typed in it, kept across redraws (a tile clicked, a mode).
        internal var _makePinText:String = "";

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
            _max = RoomsWire.LEVEL_ANY;
            _makePinText = "";
            if (win._poolTotal < 0)
                win.ask(RoomsWire.POOL, RoomsWire.POOL_ASK_ROWS);
            // The settings of the player's last room made by hand (the
            // defaults with none): the window starts with them. Not the PIN.
            _saved = null;
            win.ask(RoomsWire.SETTINGS, 1);
            win.render();
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
            look.text(look._content, win.say("pinHint"), 190, y + 4, FORM_W - 190, 14, RoomsLook.GREY);
            y += 38;
            slider(y, win.say("size"), "size");
            y += SLIDER_H;
            slider(y, win.say("minPlayers"), "min");
            y += SLIDER_H;
            slider(y, win.say("max"), "max");
            y += SLIDER_H;
            slider(y, win.say("wait"), "wait");
            y += SLIDER_H + 12;
            // The modes: the two kinds of protective device apart, team
            // deathmatch and Research; a mode out takes all its entries out of
            // the pool. Rows of 24 px: four fit where three of 30 did.
            look.text(look._content, win.say("modes").toUpperCase(), 14, y, 260, 12, RoomsLook.DIM);
            y += 22;
            for (var mi:int = 0; mi < MODE_ORDER.length; mi++)
            {
                var mode:int = MODE_ORDER[mi];
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
            made[1].enabled = poolChosen();
            renderPool();
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

        private function slider(y:Number, label:String, what:String):void
        {
            look.text(look._content, label.toUpperCase(), TRACK_X, y + 4, 260, 12, RoomsLook.DIM);
            var value:TextField = look.text(look._content, "", TRACK_X + TRACK_W - 160, y - 2, 160, 18, RoomsLook.GOLD, "right");
            var box:Sprite = new Sprite();
            box.x = TRACK_X;
            box.y = y + 24;
            box.name = what;
            box.buttonMode = true;
            var hit:Shape = new Shape();
            // Not alpha 0: Scaleform hit-tests no fully clear fill.
            hit.graphics.beginFill(0x000000, 0.01);
            hit.graphics.drawRect(-KNOB_R, -4, TRACK_W + 2 * KNOB_R, 28);
            hit.graphics.endFill();
            box.addChild(hit);
            var bar:Shape = new Shape();
            box.addChild(bar);
            box.addEventListener(MouseEvent.MOUSE_DOWN, onSlide, false, 0, true);
            look.route(box, function(control:DisplayObject, e:MouseEvent):void
            {
                startSlide(control.name, e.stageX);
            });
            look._content.addChild(box);
            // Its range, small and dim right under the track, and a line under
            // the whole: each slider a block of its own.
            var list:Array = stops(what);
            look.text(look._content, stopText(what, int(list[0])), TRACK_X, y + 40, 120, 11, 0x5E6E7A);
            look.text(look._content, stopText(what, int(list[list.length - 1])), TRACK_X + TRACK_W - 120, y + 40, 120, 11,
                0x5E6E7A, "right");
            var rule:Shape = new Shape();
            rule.graphics.beginFill(0x232C34, 1);
            rule.graphics.drawRect(TRACK_X, y + SLIDER_H - 6, TRACK_W, 1);
            rule.graphics.endFill();
            look._content.addChild(rule);
            _sliders[what] = {"box": box, "bar": bar, "value": value};
            drawSlider(what);
        }

        private function stopText(what:String, v:int):String
        {
            return what == "wait" ? startText(v) : what == "size" || what == "min" ? v + " v " + v
                : win.levelText(v);
        }

        // The time to start a wait makes: the vote's own 15 s and the wait
        // (the owner's of 2026-10-19: the vote runs until the match starts,
        // its time added to the wait; 30 s at the least).
        private function startText(wait:int):String
        {
            var all:int = wait + RoomsWire.VOTE_SECS;
            if (all < 60)
                return all + " " + win.say("sec");
            return int(all / 60) + " " + win.say("min") + (all % 60 != 0 ? " " + (all % 60) + " " + win.say("sec") : "");
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

        private function valueOf(what:String):int
        {
            if (what == "max" && _max != RoomsWire.LEVEL_ANY && _max < win._gear)
                _max = RoomsWire.LEVEL_ANY;
            return what == "size" ? _size : what == "min" ? _minSide : what == "wait" ? _wait : _max;
        }

        private function drawSlider(what:String):void
        {
            var s:Object = _sliders[what];
            if (s == null)
                return;
            var list:Array = stops(what);
            var at:int = Math.max(0, list.indexOf(valueOf(what)));
            var kx:Number = list.length > 1 ? TRACK_W * at / (list.length - 1) : 0;
            var g:* = s.bar.graphics;
            g.clear();
            // Plain rectangles of one height: the gold part on the track
            // exactly (Scaleform's round rectangles come out uneven).
            g.beginFill(0x3A434B, 1);
            g.drawRect(0, 8, TRACK_W, 4);
            g.endFill();
            if (kx > 0)
            {
                g.beginFill(RoomsLook.GOLD, 1);
                g.drawRect(0, 8, kx, 4);
                g.endFill();
            }
            g.beginFill(0x0B0E11, 1);
            g.drawCircle(kx, 10, KNOB_R + 2);
            g.endFill();
            g.beginFill(RoomsLook.GOLD, 1);
            g.drawCircle(kx, 10, KNOB_R);
            g.endFill();
            var v:int = valueOf(what);
            s.value.text = stopText(what, v);
        }

        private function onSlide(e:MouseEvent):void
        {
            startSlide(e.currentTarget.name, e.stageX);
        }

        private function startSlide(what:String, stageX:Number):void
        {
            RoomsLook.clickSound();
            _drag = what;
            slideTo(stageX);
            if (win.stage == null)
                return;
            win.stage.addEventListener(MouseEvent.MOUSE_MOVE, onSlideMove);
            win.stage.addEventListener(MouseEvent.MOUSE_UP, onSlideUp);
        }

        private function onSlideMove(e:MouseEvent):void
        {
            slideTo(e.stageX);
        }

        private function onSlideUp(e:MouseEvent):void
        {
            e.currentTarget.removeEventListener(MouseEvent.MOUSE_MOVE, onSlideMove);
            e.currentTarget.removeEventListener(MouseEvent.MOUSE_UP, onSlideUp);
            _drag = null;
        }

        private function slideTo(stageX:Number):void
        {
            var s:Object = _drag != null ? _sliders[_drag] : null;
            if (s == null)
                return;
            var local:Number = s.box.globalToLocal(new Point(stageX, 0)).x;
            var list:Array = stops(_drag);
            var at:int = Math.round(Math.max(0, Math.min(1, local / TRACK_W)) * (list.length - 1));
            var v:int = int(list[at]);
            if (_drag == "size")
                _size = v;
            else if (_drag == "min")
                _minSide = v;
            else if (_drag == "wait")
                _wait = v;
            else
                _max = v;
            drawSlider(_drag);
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

        internal function applySaved():void
        {
            var s:Object = _saved;
            _saved = null;
            if (stops("size").indexOf(s.size) >= 0)
                _size = s.size;
            if (stops("min").indexOf(s.min) >= 0)
                _minSide = s.min;
            if (stops("wait").indexOf(s.wait) >= 0)
                _wait = s.wait;
            if (s.max >= 0 && s.max <= RoomsWire.LEVEL_MAX)
                _max = s.max;
            _modesOff = {};
            for (var m:int = 0; m < s.modesOff.length; m++)
                _modesOff[String(s.modesOff[m])] = true;
            var on:Object = {};
            var i:int;
            for (i = 0; i < win._pool.length; i++)
            {
                var p:Object = win._pool[i];
                var chunk:uint = uint(s.bits[int(p.index / RoomsWire.BITS_PER_CHUNK)]);
                if ((chunk & (1 << (p.index % RoomsWire.BITS_PER_CHUNK))) != 0)
                    on[p.level + "_" + p.weather] = true;
            }
            _off = {};
            for (i = 0; i < win._pool.length; i++)
            {
                var key:String = win._pool[i].level + "_" + win._pool[i].weather;
                if (on[key] != true)
                    _off[key] = true;
            }
        }

        // A new creator starts with the server's pool, the experimental maps out.
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
            {
                var p:Object = win._pool[i];
                if (_off[p.level + "_" + p.weather] != true && _modesOff[String(p.mode)] != true && p.index < RoomsWire.POOL_BITS)
                    return true;
            }
            return false;
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
            if (typedPin.length > 0 && typedPin.length != RoomsWire.PIN_DIGITS)
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
                if (_off[p.level + "_" + p.weather] == true || _modesOff[String(p.mode)] == true || p.index >= RoomsWire.POOL_BITS)
                    continue;
                chunks[int(p.index / RoomsWire.BITS_PER_CHUNK)] |= 1 << (p.index % RoomsWire.BITS_PER_CHUNK);
                any = true;
            }
            var used:int = win._pool.length > 0 ? int((win._pool.length + (RoomsWire.BITS_PER_CHUNK - 1)) / RoomsWire.BITS_PER_CHUNK) : 1;
            for (var c:int = 0; c < used && c < RoomsWire.CHUNKS; c++)
                win._steps.push(RoomsWire.BITS + c * RoomsWire.CHUNK_STEP + chunks[c]);
            var pinText:String = _makePin != null ? _makePin.text : "";
            var withPin:Boolean = pinText.length > 0;
            var wait:int = RoomsWire.WAITS.indexOf(_wait);
            if (wait < 0)
                wait = WAIT_INDEX_DEFAULT;
            valueOf("max");
            var code:int = ((((_size - RoomsWire.SIZE_MIN) * RoomsWire.LEVELS + _max) * RoomsWire.WAIT_COUNT + wait) * 2 + (withPin ? 1 : 0)) * RoomsWire.MIN_SIDES + (_minSide - 1);
            win._steps.push(RoomsWire.CREATE + code);
            if (withPin)
                win._steps.unshift(RoomsWire.PIN + int(pinText));
            if (!any)
            {
                win._steps = [];
                win.render();
                return;
            }
            win._fastUntil = getTimer() + 5000;
            win.next();
        }
    }
}
