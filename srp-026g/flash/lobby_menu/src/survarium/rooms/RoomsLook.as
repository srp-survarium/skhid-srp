package survarium.rooms
{
    import flash.display.Bitmap;
    import flash.display.BitmapData;
    import flash.display.DisplayObject;
    import flash.display.DisplayObjectContainer;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.external.ExternalInterface;
    import flash.geom.ColorTransform;
    import flash.geom.Matrix;
    import flash.text.TextField;
    import flash.text.TextFieldType;
    import flash.text.TextFormat;
    import flash.utils.getDefinitionByName;
    import scaleform.clik.events.ButtonEvent;
    import scaleform.gfx.FocusManager;
    import scaleform.gfx.MouseEventEx;
    import survarium.constants.UiSound;

    // The rooms window's look, the game's: its colours, the maps' renders,
    // and the parts its views are drawn of (texts, the game's buttons, the
    // footer, the close button, scrolled boxes, inputs, check boxes), in the
    // window's content. Presses Scaleform gives the content itself, not the
    // control under them, go to that control here (`route`).
    public class RoomsLook
    {
        public static const GOLD:uint = 0xE0A64E;
        public static const RED:uint = 0xE0533F;
        // A member of a room sees where it is (the owner's of 2026-10-19:
        // players did not know they were in one): the top menu's tab and
        // the switch to a better room in this, Play turned red (it leaves).
        public static const YELLOW:uint = 0xFFD23F;
        // The colours as transforms, for drawn parts (the tab's icon, the
        // game's buttons): yellow and red over their own shading.
        public static const YELLOW_TINT:ColorTransform = new ColorTransform(1, 0.82, 0.25, 1, 60, 45, 0, 0);
        public static const RED_TINT:ColorTransform = new ColorTransform(1, 0.3, 0.25, 1, 70, 0, 0, 0);
        public static const LIGHT:uint = 0xE8EEF2;
        public static const GREY:uint = 0x93A3AF;
        public static const DIM:uint = 0x5F707C;
        // As tall as the game's close button (37 x 38) that sits in it.
        public static const HEAD_H:Number = 38;
        public static const FOOT_H:Number = 46;
        // The game's bold Calibri (the font of these lists) has U+2026.
        public static const DOTS:String = "…";
        // The render a level is drawn with: the v0.21 ports with the map they
        // are an older look of, the v0.11 and v0.69 ones with their one loading image.
        private static const RENDER_LEVEL:Object = {"10": 3, "11": 6};
        private static const RENDER_ONE:Object = {"12": "Level_v011e", "13": "Level_v011e", "14": "Level_v069d_rudnya"};
        private static const RENDER:Object = {"1": "day", "2": "rain", "3": "evn", "4": "night", "5": "prerain"};
        private static var _renders:Object = {};
        // A vote card's mode tag: the protective devices blue, team
        // deathmatch red, Research green, Blackout white.
        private static const MODE_COLOUR:Object = {"4": 0xB23A2A, "5": 0x6E9A3C, "6": 0xE8EEF2};

        public static function modeColour(mode:int):uint
        {
            var colour:* = MODE_COLOUR[String(mode)];
            return colour != null ? uint(colour) : 0x5A8FB5;
        }

        // A game's button in a colour: its art tinted, its label left white
        // (the owner's of 2026-10-20: the whole button tinted, its label too,
        // did not look good). The skin redraws its parts on hover: a button
        // kept so is tinted again each frame.
        public static function skinTint(btn:*, tint:ColorTransform):void
        {
            var box:DisplayObjectContainer = btn as DisplayObjectContainer;
            if (box == null)
                return;
            box.transform.colorTransform = new ColorTransform();
            var field:DisplayObject = null;
            try
            {
                field = btn.textField as DisplayObject;
            }
            catch (err:Error)
            {
            }
            for (var i:int = 0; i < box.numChildren; i++)
            {
                var part:DisplayObject = box.getChildAt(i);
                if (field != null && (part == field
                    || (part is DisplayObjectContainer && DisplayObjectContainer(part).contains(field))))
                    continue;
                part.transform.colorTransform = tint;
            }
        }

        // The window's content (the views' parts) and its backdrop (the
        // frame under them).
        internal var _content:Sprite;
        internal var _backdrop:Sprite;
        // The font of the bottom bar's tabs (the game's own).
        internal var _font:String;
        internal var _bold:Object;
        // The window's size in the design's units.
        internal var _w:Number = 900;
        internal var _h:Number = 500;
        // Controls placed straight in the content, each with what a press on
        // it does: Scaleform gave such presses to the content itself (the
        // left of the make view took none: PIN, sliders, modes).
        internal var _routes:Array = [];
        // A scrolled box: its rows go into the sprite it gives; done() sets
        // their height. The wheel and the bar's thumb move it.
        internal var _scrollInner:Sprite;
        internal var _scrollThumb:Sprite;
        internal var _scrollTop:Number = 0;
        internal var _scrollViewH:Number = 0;
        internal var _scrollTotal:Number = 0;
        internal var _scrollBarX:Number = 0;
        internal var _scrollPx:Number = 0;
        internal var _scrollGrab:Number = 0;

        public function RoomsLook(content:Sprite, backdrop:Sprite, font:String, bold:Object)
        {
            super();
            _content = content;
            _backdrop = backdrop;
            _font = font;
            _bold = bold;
        }

        internal function text(box:Sprite, value:String, x:Number, y:Number, w:Number, size:Number,
            color:uint, align:String = "left"):TextField
        {
            var tf:TextField = new TextField();
            var format:TextFormat = new TextFormat(_font, size, color, _bold);
            format.align = align;
            tf.defaultTextFormat = format;
            tf.embedFonts = true;
            tf.selectable = false;
            tf.mouseEnabled = false;
            tf.multiline = false;
            tf.wordWrap = false;
            tf.x = x;
            tf.y = y;
            tf.width = w;
            tf.height = size + 8;
            tf.text = value;
            box.addChild(tf);
            return tf;
        }

        internal function button(box:Sprite, label:String, x:Number, y:Number, w:Number, handler:Function,
            data:Object = null, h:Number = 32, primary:Boolean = false):*
        {
            // The game's framed button with a label (the game menu's look); the
            // red one for the main action, where it has a label too.
            var b:* = null;
            if (primary)
            {
                try
                {
                    var red:Class = getDefinitionByName("SrvButtonSkinnedRed") as Class;
                    b = new red();
                    if (b.textField == null)
                        b = null;
                }
                catch (e:Error)
                {
                    b = null;
                }
            }
            if (b == null)
            {
                var cls:Class = getDefinitionByName("SrvButtonSkinned") as Class;
                b = new cls();
            }
            b.label = label;
            b.x = x;
            b.y = y;
            box.addChild(b);
            try
            {
                b.setSize(w, h);
            }
            catch (e2:Error)
            {
                b.width = w;
                b.height = h;
            }
            b.data = data;
            b.addEventListener(ButtonEvent.CLICK, handler, false, 0, true);
            routeButton(b);
            return b;
        }

        // The strip along the window's bottom, as the shop's under its list;
        // `buttons` are [label, handler, data] laid out centred in it, or at
        // `at` (x of the first) when given.
        internal function footer(buttons:Array, at:Number = -1):Array
        {
            var made:Array = [];
            var y:Number = _h - FOOT_H;
            var strip:Shape = new Shape();
            strip.graphics.beginFill(0x07090A, 1);
            strip.graphics.drawRect(1, y, _w - 2, 1);
            strip.graphics.endFill();
            strip.graphics.beginFill(0x2A2F34, 1);
            strip.graphics.drawRect(1, y + 1, _w - 2, 1);
            strip.graphics.endFill();
            strip.graphics.beginFill(0x15181B, 1);
            strip.graphics.drawRect(1, y + 2, _w - 2, FOOT_H - 3);
            strip.graphics.endFill();
            _content.addChild(strip);
            var bw:Number = 170;
            var gap:Number = 12;
            var x:Number = at >= 0 ? at : (_w - buttons.length * bw - (buttons.length - 1) * gap) / 2;
            // Centred in the strip by the height the button really takes (the
            // game's keeps its own whatever is asked).
            for (var i:int = 0; i < buttons.length; i++)
            {
                // 32 high as All and None: squeezed lower, the game's button
                // keeps its label where its full height puts it.
                var b:* = button(_content, buttons[i][0], x + i * (bw + gap), y, bw, buttons[i][1], buttons[i][2], 32);
                b.validateNow();
                b.y = y + 1 + Math.round((FOOT_H - 1 - b.height) / 2);
                made.push(b);
            }
            return made;
        }

        // The game's own window close button (the inventory's, SquareVloseBtnChIL),
        // at its size in the header's right end; drawn when it is missing.
        internal function closeButton(handler:Function):void
        {
            try
            {
                var cls:Class = getDefinitionByName("SquareVloseBtnChIL") as Class;
                var b:* = new cls();
                // At its own size (37 x 38), in the header's right end.
                b.x = _w - 38;
                // Its art sits high in its box: 2 lower centres it.
                b.y = 2;
                _content.addChild(b);
                b.addEventListener(ButtonEvent.CLICK, handler, false, 0, true);
                routeButton(b);
                return;
            }
            catch (e:Error)
            {
            }
            headButton("×", _w - (HEAD_H - 2) - 2, handler);
        }

        // A small square button of the header, as the shop's ×.
        private function headButton(glyph:String, x:Number, handler:Function):void
        {
            var size:Number = HEAD_H - 4;
            var b:Sprite = new Sprite();
            b.x = x;
            b.y = 3;
            var face:Shape = new Shape();
            face.graphics.beginFill(0x4A535B, 1);
            face.graphics.drawRect(0, 0, size, size);
            face.graphics.endFill();
            face.graphics.beginFill(0x0D1012, 1);
            face.graphics.drawRect(1, 1, size - 2, size - 2);
            face.graphics.endFill();
            b.addChild(face);
            var over:Shape = new Shape();
            over.graphics.beginFill(0xFFFFFF, 0.12);
            over.graphics.drawRect(1, 1, size - 2, size - 2);
            over.graphics.endFill();
            over.visible = false;
            b.addChild(over);
            text(b, glyph, 0, glyph == "×" ? 0 : -1, size, 17, 0xE6E9EB, "center");
            b.buttonMode = true;
            b.mouseChildren = false;
            // Strong: a closure held weakly may be collected at once.
            b.addEventListener(MouseEvent.ROLL_OVER, function(e:Event):void { over.visible = true; });
            b.addEventListener(MouseEvent.ROLL_OUT, function(e:Event):void { over.visible = false; });
            b.addEventListener(MouseEvent.CLICK, handler, false, 0, true);
            _content.addChild(b);
        }

        // The rows scroll inside a holder at the box's place, its mask there
        // too: a mask drawn in the window's coordinates was hit-tested by
        // Scaleform at the rows' offset, over the controls left of the pool,
        // and took their clicks.
        internal function scrollBox(x:Number, y:Number, w:Number, h:Number):Sprite
        {
            var holder:Sprite = new Sprite();
            holder.x = x;
            holder.y = y;
            var mask:Shape = new Shape();
            mask.graphics.beginFill(0x000000, 1);
            mask.graphics.drawRect(0, 0, w, h);
            mask.graphics.endFill();
            holder.addChild(mask);
            var inner:Sprite = new Sprite();
            holder.addChild(inner);
            inner.mask = mask;
            _content.addChild(holder);
            _scrollInner = inner;
            _scrollTop = y;
            _scrollViewH = h;
            _scrollBarX = x + w + 4;
            return inner;
        }

        internal function scrollDone(total:Number):void
        {
            _scrollTotal = total;
            if (total > _scrollViewH)
            {
                var track:Shape = new Shape();
                track.graphics.beginFill(0x0F151B, 1);
                track.graphics.drawRect(_scrollBarX, _scrollTop, 6, _scrollViewH);
                track.graphics.endFill();
                _content.addChild(track);
                _scrollThumb = new Sprite();
                _scrollThumb.buttonMode = true;
                var th:Number = Math.max(30, _scrollViewH * _scrollViewH / total);
                _scrollThumb.graphics.beginFill(0x4C6070, 1);
                _scrollThumb.graphics.drawRect(0, 0, 6, th);
                _scrollThumb.graphics.endFill();
                _scrollThumb.x = _scrollBarX;
                _scrollThumb.addEventListener(MouseEvent.MOUSE_DOWN, onThumb, false, 0, true);
                _content.addChild(_scrollThumb);
            }
            else
            {
                _scrollThumb = null;
            }
            applyScroll();
        }

        internal function applyScroll():void
        {
            if (_scrollInner == null)
                return;
            var most:Number = Math.max(0, _scrollTotal - _scrollViewH);
            _scrollPx = Math.max(0, Math.min(most, _scrollPx));
            _scrollInner.y = -_scrollPx;
            if (_scrollThumb != null)
            {
                var room:Number = _scrollViewH - _scrollThumb.height;
                _scrollThumb.y = _scrollTop + (most > 0 ? room * _scrollPx / most : 0);
            }
        }

        private function onThumb(e:MouseEvent):void
        {
            if (_content.stage == null || _scrollThumb == null)
                return;
            _scrollGrab = _content.mouseY - _scrollThumb.y;
            _content.stage.addEventListener(MouseEvent.MOUSE_MOVE, onThumbMove);
            _content.stage.addEventListener(MouseEvent.MOUSE_UP, onThumbUp);
        }

        private function onThumbMove(e:MouseEvent):void
        {
            if (_scrollThumb == null)
                return;
            var room:Number = _scrollViewH - _scrollThumb.height;
            var most:Number = Math.max(0, _scrollTotal - _scrollViewH);
            var at:Number = _content.mouseY - _scrollGrab - _scrollTop;
            _scrollPx = room > 0 ? most * at / room : 0;
            applyScroll();
        }

        private function onThumbUp(e:MouseEvent):void
        {
            // The listeners are the stage's: removed from it whatever ours is
            // now (the window may be off the stage meanwhile).
            e.currentTarget.removeEventListener(MouseEvent.MOUSE_MOVE, onThumbMove);
            e.currentTarget.removeEventListener(MouseEvent.MOUSE_UP, onThumbUp);
        }

        internal function input(box:Sprite, x:Number, y:Number, value:String):TextField
        {
            var tf:TextField = text(box, value, x, y, 70, 16, GOLD);
            // Its own background and border: an empty field is a solid area
            // to click.
            tf.height = 24;
            tf.background = true;
            tf.backgroundColor = 0x070A0D;
            tf.border = true;
            tf.borderColor = 0x3A4854;
            tf.type = TextFieldType.INPUT;
            tf.restrict = "0-9";
            tf.maxChars = RoomsWire.PIN_DIGITS;
            tf.selectable = true;
            tf.mouseEnabled = true;
            tf.tabEnabled = true;
            tf.addEventListener(MouseEvent.MOUSE_DOWN, onPress, false, 0, true);
            route(tf, function(control:DisplayObject, e:MouseEvent):void
            {
                FocusManager.setFocus(control as TextField);
            });
            return tf;
        }

        // A control and what a press on it does, (control, event).
        internal function route(control:DisplayObject, press:Function):void
        {
            _routes.push({"control": control, "press": press});
        }

        // A game button (CLIK) placed in the content: a press the content
        // got itself clicks it as the game's own press would, sound and all.
        internal function routeButton(b:*):void
        {
            route(b, function(control:DisplayObject, e:MouseEvent):void
            {
                var button:* = control;
                if (button.enabled === false)
                    return;
                clickSound();
                control.dispatchEvent(new ButtonEvent(ButtonEvent.CLICK));
            });
        }

        // A press in the content: to the last control routed under it, unless
        // Scaleform gave it to that control (or to something inside it)
        // itself, which then has it (no double press).
        internal function onContentDown(e:MouseEvent):void
        {
            // Only the left button presses (a right-click on a member is its
            // equipment, `onContentClick`).
            var ex:MouseEventEx = e as MouseEventEx;
            if (ex != null && ex.buttonIdx != MouseEventEx.LEFT_BUTTON)
                return;
            for (var i:int = _routes.length - 1; i >= 0; i--)
            {
                var r:Object = _routes[i];
                var control:DisplayObject = r.control as DisplayObject;
                if (control.stage == null || !control.visible || !control.hitTestPoint(e.stageX, e.stageY, false))
                    continue;
                var at:DisplayObject = e.target as DisplayObject;
                while (at != null && at != control && at != _content)
                    at = at.parent;
                if (at != control)
                    r.press(control, e);
                return;
            }
        }

        private function onPress(e:MouseEvent):void
        {
            FocusManager.setFocus(e.currentTarget as TextField);
        }

        internal function line(box:Sprite, x:Number, y:Number, w:Number):void
        {
            var s:Shape = new Shape();
            s.graphics.beginFill(0x26323C, 1);
            s.graphics.drawRect(x, y, w, 1);
            s.graphics.endFill();
            box.addChild(s);
        }

        // The game's own check box (its bitmaps), drawn when they are missing.
        internal function box(on:Boolean):DisplayObject
        {
            try
            {
                var cls:Class = getDefinitionByName(on ? "check_checked" : "check_unchecked") as Class;
                var b:Bitmap = new Bitmap(new cls() as BitmapData);
                b.y = 14;
                return b;
            }
            catch (e:Error)
            {
            }
            var drawn:Shape = new Shape();
            drawn.graphics.lineStyle(1, 0x6D8799);
            drawn.graphics.beginFill(on ? GOLD : 0x0F151B, 1);
            drawn.graphics.drawRect(0, 14, 16, 16);
            drawn.graphics.endFill();
            return drawn;
        }

        // A map's render for its weather, made small once and kept.
        // A map's render scaled to a tile or card, kept for the next time.
        internal function mapRender(level:int, weather:int, w:int, h:int):Bitmap
        {
            var key:String = level + "_" + weather + "_" + w + "x" + h;
            var small:BitmapData = _renders[key] as BitmapData;
            if (small == null)
            {
                small = new BitmapData(w, h, false, 0x05080B);
                try
                {
                    var cls:Class = renderClass(level, weather);
                    var big:BitmapData = new cls() as BitmapData;
                    var scale:Number = Math.max(w / big.width, h / big.height);
                    var m:Matrix = new Matrix();
                    m.scale(scale, scale);
                    m.translate((w - big.width * scale) / 2, (h - big.height * scale) / 2);
                    small.draw(big, m, null, null, null, true);
                    big.dispose();
                }
                catch (e:Error)
                {
                }
                _renders[key] = small;
            }
            return new Bitmap(small);
        }

        // The library bitmap of a level in a weather; its day one when that
        // weather has none.
        private function renderClass(level:int, weather:int):Class
        {
            var one:String = RENDER_ONE[String(level)] as String;
            if (one != null)
                return getDefinitionByName(one) as Class;
            var base:int = RENDER_LEVEL[String(level)] != null ? int(RENDER_LEVEL[String(level)]) : level;
            try
            {
                return getDefinitionByName("Level" + base + "_" + RENDER[String(weather)]) as Class;
            }
            catch (e:Error)
            {
            }
            return getDefinitionByName("Level" + base + "_day") as Class;
        }

        // A name cut to its field by its width in the field's own font and
        // size: as much of it as fits, then "…", then `tail` whole (the room
        // view's "(owner)"); a name the server cut ends with "…" already.
        internal static function fitName(field:TextField, value:String, tail:String):void
        {
            var room:Number = field.width - 4;
            field.text = value + tail;
            if (field.textWidth <= room)
                return;
            if (value.charAt(value.length - 1) == DOTS)
                value = value.substr(0, value.length - 1);
            var low:int = 0;
            var high:int = value.length;
            // The longest head that fits with the dots and the tail.
            while (low < high)
            {
                var mid:int = int((low + high + 1) / 2);
                field.text = value.substr(0, mid) + DOTS + tail;
                if (field.textWidth <= room)
                    low = mid;
                else
                    high = mid - 1;
            }
            // No space left before the dots.
            while (low > 0 && value.charAt(low - 1) == " ")
                low--;
            field.text = value.substr(0, low) + DOTS + tail;
        }

        // A member's name: grey while its game is gone (`away`), red when its
        // kit cannot play, white for the player itself, gold in the room's
        // match. The original game marks no friend or clan member in any of
        // its player lists (results, squad, friends, clan, rankings, chat:
        // only the player, its squad and the enemy), so neither is marked
        // here (the owner's of 2026-10-20: only as the original did it).
        internal static function memberColour(m:Object):uint
        {
            return m.away ? 0x5E6E7A : m.fit != 0 ? RED : m.me ? 0xFFFFFF : (m.inMatch ? GOLD : LIGHT);
        }

        // A squad's mark (the owner's of 2026-10-31: squads seen in the room,
        // with the match's own squad icon): the pairs both in a room numbered
        // 1, 2, ... in its list, each shown as the match's Tab list shows a
        // squad: its squad_icon's frame of that number (a letter in a ring:
        // A, B, G, D, E; the hud has five, a sixth pair is A again), tinted
        // as the hud tints it, our own squad green (0xB3EA97), another tan
        // (0x9D7E57; the hud's enemy red has no room side to go by).
        internal static const SQUAD_ICON_W:int = 16;
        internal static const OUR_SQUAD:uint = 0xB3EA97;
        internal static const OTHER_SQUAD:uint = 0x9D7E57;
        private static const SQUAD_ICONS:Array = [SquadIcon1, SquadIcon2, SquadIcon3, SquadIcon4, SquadIcon5];

        // The state's `S;<marks>` row onto its members: a character each, in
        // base 36 (0 none).
        internal static function markSquads(members:Array, marks:String):void
        {
            for (var i:int = 0; i < members.length; i++)
                members[i].squad = i < marks.length ? int(parseInt(marks.charAt(i), 36)) : 0;
        }

        // The squad mark of our own squad in a list of members (0: none).
        internal static function ourSquad(members:Array):int
        {
            for (var i:int = 0; i < members.length; i++)
                if (members[i].me)
                    return int(members[i].squad);
            return 0;
        }

        // The squad icon of mark `squad` right after `name` (its text as
        // drawn), in `box`, `h` high. The name's field leaves SQUAD_ICON_W + 8
        // for it, so nothing else moves.
        internal function squadIcon(box:Sprite, name:TextField, squad:int, ours:Boolean, h:Number):void
        {
            if (squad <= 0)
                return;
            var kind:Class = SQUAD_ICONS[(squad - 1) % SQUAD_ICONS.length] as Class;
            var icon:Bitmap = new Bitmap(new kind() as BitmapData, "auto", true);
            var side:Number = Math.min(SQUAD_ICON_W, h);
            icon.width = side;
            icon.height = side;
            icon.x = int(name.x + Math.min(name.textWidth + 4, name.width) + 4);
            icon.y = int(name.y + (name.height - side) / 2);
            var tint:ColorTransform = new ColorTransform();
            tint.color = ours ? OUR_SQUAD : OTHER_SQUAD;
            icon.transform.colorTransform = tint;
            box.addChild(icon);
        }

        // The click the game's buttons make (ButtonClick: sound_play 0).
        internal static function clickSound():void
        {
            gameSound(UiSound.CLICK);
        }

        internal static function gameSound(id:int):void
        {
            try
            {
                ExternalInterface.call("sound_play", id);
            }
            catch (e:Error)
            {
            }
        }

        internal function onWheel(e:MouseEvent):void
        {
            _scrollPx += e.delta < 0 ? 60 : -60;
            applyScroll();
        }
    }
}
