package
{
    import flash.display.BitmapData;
    import flash.display.DisplayObject;
    import flash.display.Graphics;
    import flash.display.MovieClip;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.external.ExternalInterface;
    import flash.geom.Matrix;
    import flash.geom.Point;
    import flash.text.TextField;
    import flash.text.TextFieldAutoSize;
    import flash.text.TextFormat;
    import flash.text.TextFormatAlign;
    import scaleform.clik.events.ButtonEvent;
    import survarium.constants.KeyBindGroup;
    import survarium.menu.BindKeyRender;
    import survarium.menu.Settings;

    // Settings > Controls > Bindings (the owner, 2026-11-23: "controls > bindings > some new bigger window in
    // similar style, but which can follow layout we already have"; "so it is visible the whole state"; "we
    // don't need two"): the key bindings' one place. The Controls tab's game and chat lists are hidden (its lobby
    // list stays in its right column, for reading: "not bindable either way"); a button in its left
    // column under the mouse settings, over Default settings, opens a window in the
    // Settings window's own skin (its frame and title bar: SrpSettingsGround, the title's text, its close
    // button) laid out as the owner's key page: the keyboard and the mouse with every bound key's action
    // written on it, the kinds' chips under them, and the bindings' lists by kind under those.
    //
    // - The keyboard: a full-size layout without the numpad (the keys the key binder names, by their names;
    //   a key bound but not drawn shows beside the mouse); each key its dictionary's name in its corner (the
    //   exe's), a bound key the match's action(s) on it, tinted by its kind (movement, combat, items, communication, interface: KINDS); a free key plain; a key the
    //   binder does not name (or Esc) inert. The mouse: its buttons, the wheel's click, up and down, the two
    //   side buttons.
    // - The kinds' chips: the pointer over one dims every key and row of another kind.
    // - The lists: the match's rows the exe gives (root.set_keybindings; not the lobby's or the chat's, which
    //   are not bound here), by kind in the exe's order, each row its action and its key.
    //
    // Rebinding is the game's: a press of a row asks the exe for a key as the stock row does (start_bind_key,
    // the menu's "press a key" message, its conflict message, root.set_keybind); a press of a key asks the
    // action selector (`selector`, when the main menu has one) which action it takes, given to the exe as a
    // press of that key would give it (srp_bind_key "action code"; key_bindings.rs), or that it takes none
    // (srp_unbind_key "code": its rows lose it as a conflict's reassign takes it), and without one starts
    // the stock wait for the key's first action. Either is the Controls tab's change, saved by Apply. The
    // exe's table of keys (root.srp_keys: the binder's keys' names, codes and texts, each row's key name) is
    // asked at the window's opening and after each key set. Esc, the close button or a press beside the
    // window closes it. Its texts (the button's and the title, the kinds' names) are root.srp_hud_texts'
    // 16th to 21st: the button shows once they are there. A script-added child of the Settings window, sized
    // to the screen as the menu scales it (1 at 1080p and below).
    public class SrpKeysWindow extends Sprite
    {
        public static const NAME:String = "_srp_keys_window";
        public static const BUTTON:String = "_srp_keys_button";
        // The tab's buttons (SetingsButton's height) and the room between them.
        private static const BUTTON_HEIGHT:Number = 26;
        private static const BUTTON_GAP:Number = 10;
        private static const KEYS_FRAME:int = 2;
        // The kinds of action, by the exe's action ids (item_hotkeys_026g.asm's `actions`), and their colours: the
        // groups of the exe's rows (hide_ui_026g.asm's `controls`): movement; the weapon's, melee and use as combat;
        // the three "next" keys, the quick slots and the item keys as items; communication; the interface.
        public static const KINDS:Array = [
            {colour: 0x4FB8AD, ids: [0x04, 0x05, 0x06, 0x07, 0x08, 0x0B, 0x0C, 0x0D, 0x0E]},
            {colour: 0xE3953C, ids: [0x09, 0x0A, 0x1D, 0x1E, 0x21, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A,
                0x35, 0x36]},
            {colour: 0x94C451, ids: [0x20, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50,
                0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59]},
            {colour: 0x5EA3EA, ids: [0x22, 0x37, 0x3F, 0x47]},
            {colour: 0xA397C4, ids: [0x19, 0x1A, 0x1B]}];
        // The action selector a key's press asks (the main menu sets it): selector(request, answer), the request
        // {key: the key's name, code, keyName: its text, actions: the rows it has, rows: every row of the window,
        // kinds: KINDS}, answer(action id), answer(UNBIND) to take the key from its rows, answer(-1) to leave it.
        public static var selector:Function = null;
        public static const UNBIND:int = -2;

        // The keyboard, in key units (x, y, width): the main block, the function row over it, the navigation keys
        // and the arrows right of it; by the key binder's names.
        private static const KEYBOARD:Array = keyboard();
        private static const DESK_UNITS:Number = 18.25;     // the keyboard's width
        private static const DESK_ROWS:Number = 6.25;       // its height
        private static const MOUSE_GAP:Number = 1.25;       // keyboard to mouse (the side buttons in it)
        private static const MOUSE_UNITS:Number = 3.4;      // the mouse's width
        // The mouse's parts (x, y, width, height in units of the mouse's own box) by the binder's names: the buttons,
        // the wheel's up, click and down between them, the side buttons out on its left.
        private static const MOUSE:Array = [
            ["mouse1", 0, 0, 1.0, 3.0], ["mouse2", 2.4, 0, 1.0, 3.0],
            ["mouse_wheel_up", 1.0, 0, 1.4, 1.0], ["mouse3", 1.0, 1.0, 1.4, 1.0], ["mouse_wheel_down", 1.0, 2.0, 1.4, 1.0],
            ["mouse5", -1.1, 3.25, 1.1, 0.9], ["mouse4", -1.1, 4.15, 1.1, 0.9]];
        private static const MOUSE_HEIGHT:Number = 5.25;
        // Units: the largest key and the least.
        private static const MAX_UNIT:Number = 64;
        private static const MIN_UNIT:Number = 32;
        // The window's width at most (its inside).
        private static const MAX_WIDTH:Number = 1400;
        // The Settings window's frame (SrpSettingsGround, 650 x 565): its border and its title's band, kept as they
        // are; its inside stretched.
        private static const FRAME_LEFT:Number = 6, FRAME_RIGHT:Number = 6, FRAME_TOP:Number = 39, FRAME_BOTTOM:Number = 22;
        // The title and the close button as the Settings window has them (textFieldLabel, btn_close).
        private static const TITLE_Y:Number = 6.75, CLOSE_RIGHT:Number = 34.5, CLOSE_SCALE:Number = 0.756;
        private static const SIDE:Number = 22;              // inside the frame's sides
        private static const SPACE:Number = 9;             // between the window's parts
        private static const MARGIN:Number = 12;            // to the screen's edges at the least
        private static const CHIP_HEIGHT:Number = 22;
        // The lists: a row, a heading, the gap between columns, a column's least width.
        private static const LIST_ROW:Number = 19;
        private static const LIST_HEADING:Number = 24;
        private static const LIST_GAP:Number = 20;
        private static const LIST_COLUMN:Number = 180;
        private static const KEY_BOX:Number = 52;           // a row's key box at the least
        private static const KEY_FONT:Number = 11;          // its text's size, every row's the same
        private static const MIN_ACTION_SIZE:Number = 8;    // the keys' actions' least size
        // The headings' look (the Controls tab's "Lobby:", "Chat:": s_lobby), the stock key box's.
        private static const HEADING_COLOUR:uint = 0xABAAAE;
        private static const INK:uint = 0xDADADA;
        private static const FADED:Number = 0.22;
        // The stock key box: black, a light frame, white bold text; a free one dimmer; the awaited one the game's
        // orange (the tab bar's).
        private static const BOX_FILL:uint = 0x0A0B0C;
        private static const BOX_EDGE:uint = 0x8F9296;
        private static const BOX_EDGE_FREE:uint = 0x3E4246;
        private static const AWAITED:uint = 0xF08A24;

        // The tab's button that opens the window.
        public var button:SetingsButton;
        private var content:MovieClip;
        // The rows as the exe gave them (Settings' _keys), the window's texts.
        private var keys:Array = null;
        private var title:String = "";
        private var kindNames:Array = [];
        // The exe's table (root.srp_keys): each binder key by name {code, text}, each name by code, and each row's
        // key name by action; empty until it answers.
        private var binder:Object = {};
        private var byCode:Object = {};
        private var rowKeys:Object = {};
        // The row whose key the stock wait is for (a row's or a key's press), or null.
        private var awaited:Object = null;
        // The parts: the dimmed screen, the board (frame, title, close button), its body (keyboard, mouse, chips,
        // lists), each key's view {name, sprite, kinds} and each row's {row, sprite, kind}.
        private var backdrop:Sprite;
        private var panel:Sprite;
        private var frame:Shape;
        private var titleField:TextField;
        private var closeButton:ButtonCloseWindow2;
        private var body:Sprite;
        private var chips:Sprite;
        private var keyViews:Array = [];
        private var rowViews:Array = [];
        private var unit:Number = MAX_UNIT;
        // The lists' key box (as wide as the binder's widest key text at KEY_FONT), the keys' names' and actions'
        // one size each (the unit's): set by build.
        private var keyBox:Number = KEY_BOX;
        private var legendSize:Number = 10;
        private var actionSize:Number = 10;

        public function SrpKeysWindow(content:MovieClip)
        {
            this.content = content;
            name = NAME;
            visible = false;
            button = new SetingsButton();
            button.name = BUTTON;
            button.autoSize = "none";
            button.focusable = true;
            button.toggle = false;
            button.visible = false;
            button.addEventListener(ButtonEvent.CLICK, onOpen);
            content.addChild(button);
            backdrop = new Sprite();
            backdrop.addEventListener(MouseEvent.CLICK, onBackdrop);
            addChild(backdrop);
            panel = new Sprite();
            panel.name = NAME + "_panel";
            addChild(panel);
            frame = new Shape();
            panel.addChild(frame);
            titleField = text(17, 0xC6D0D1, true);
            titleField.name = NAME + "_title";
            panel.addChild(titleField);
            closeButton = new ButtonCloseWindow2();
            closeButton.name = NAME + "_close";
            closeButton.alpha = 1;
            closeButton.scaleX = closeButton.scaleY = CLOSE_SCALE;
            closeButton.addEventListener(ButtonEvent.CLICK, onClose);
            panel.addChild(closeButton);
            body = new Sprite();
            body.name = NAME + "_body";
            panel.addChild(body);
            chips = new Sprite();
            chips.name = NAME + "_chips";
        }

        // The kind of an action (KINDS' index), or -1.
        public static function kindOf(action:int):int
        {
            for (var k:int = 0; k < KINDS.length; k++)
            {
                if (KINDS[k].ids.indexOf(action) != -1)
                {
                    return k;
                }
            }
            return -1;
        }

        // The exe's texts (root.srp_hud_texts): the button's and the title, the kinds' names.
        public function texts(label:String, kinds:Array):void
        {
            title = label;
            kindNames = kinds;
            button.label = label;
            show();
        }

        // On every bind of a tab (create_content): on top of the tab, shown on the Controls frame.
        public function place():void
        {
            content.setChildIndex(button, content.numChildren - 1);
            show();
        }

        // After the tab's layout (Settings.layoutControls): the key bindings' place on the tab, the left column
        // under the mouse settings (the tab's lists hidden: the bindings are this window's alone; the owner,
        // 2026-11-23: "we don't need two", "both options can move to left"): the button from `left` to `right`
        // at `top`, Default settings under it as wide.
        public function layout(defaults:DisplayObject, left:Number, right:Number, top:Number):void
        {
            button.x = Math.round(left);
            button.width = Math.round(right - left);
            button.y = Math.round(top);
            defaults.x = button.x;
            defaults.width = button.width;
            defaults.y = button.y + BUTTON_HEIGHT + BUTTON_GAP;
        }

        // The rows (Settings.applyKeys, at each bind of the Controls tab).
        public function rows(keys:Array):void
        {
            this.keys = keys;
            if (open)
            {
                build();
            }
        }

        // A key set or kept (root.set_keybind): the exe's table asked again at the next frame, once for the many a
        // reset or a conflict sends (its answer draws the window again).
        public function refresh():void
        {
            if (open)
            {
                addEventListener(Event.ENTER_FRAME, onRefresh);
            }
        }

        private function onRefresh(event:Event):void
        {
            removeEventListener(Event.ENTER_FRAME, onRefresh);
            if (open)
            {
                SrpExe.call("srp_keys", null);
            }
        }

        // The exe's table (root.srp_keys): lines "K<TAB>name<TAB>code<TAB>text" (the binder's keys) and
        // "A<TAB>action<TAB>name" (each row's key, "" none).
        public function answerKeys(table:String):void
        {
            binder = {};
            byCode = {};
            rowKeys = {};
            for each (var line:String in table.split("\n"))
            {
                var fields:Array = line.split("\t");
                if (fields[0] == "K" && fields.length == 4 && binder[fields[1]] == undefined)
                {
                    binder[fields[1]] = {code: int(fields[2]), text: fields[3]};
                    byCode[int(fields[2])] = fields[1];
                }
                else if (fields[0] == "A" && fields.length == 3)
                {
                    rowKeys[int(fields[1])] = fields[2];
                }
            }
            if (open)
            {
                build();
            }
        }

        public function get open():Boolean
        {
            return visible;
        }

        // Esc, the close button, a press beside the window, the Settings closed; whether it was open.
        public function close():Boolean
        {
            if (!open)
            {
                return false;
            }
            visible = false;
            stage.removeEventListener(Event.RESIZE, onResize);
            stage.removeEventListener(Event.CANCEL, onWaitEnd, true);
            awaited = null;
            clear();
            return true;
        }

        private function show():void
        {
            button.visible = content.currentFrame == KEYS_FRAME && title != "";
            // Scaleform leaves a script-created control fully transparent.
            button.alpha = 1;
        }

        private function onOpen(event:ButtonEvent):void
        {
            if (keys == null || stage == null)
            {
                return;
            }
            parent.setChildIndex(this, parent.numChildren - 1);
            visible = true;
            stage.addEventListener(Event.RESIZE, onResize);
            build();
            SrpExe.call("srp_keys", null);
        }

        private function onClose(event:ButtonEvent):void
        {
            close();
        }

        private function onBackdrop(event:MouseEvent):void
        {
            if (event.target == backdrop)
            {
                close();
            }
        }

        private function onResize(event:Event):void
        {
            build();
        }

        private function clear():void
        {
            // (Scaleform's removeChildren throws on an empty container: one at a time)
            empty(body);
            body.graphics.clear();
            empty(chips);
            keyViews = [];
            rowViews = [];
        }

        private static function empty(container:Sprite):void
        {
            while (container.numChildren > 0)
            {
                container.removeChildAt(container.numChildren - 1);
            }
        }

        // The window's rows: the match's (the lobby's are the tab's to read, the chat's cannot be set: the owner,
        // 2026-11-23, "not bindable either way").
        private function shownRows():Array
        {
            var out:Array = [];
            for each (var row:Object in keys)
            {
                if (row.group_id != KeyBindGroup.CHAT && row.group_id != KeyBindGroup.LOBBY)
                {
                    out.push(row);
                }
            }
            return out;
        }

        // The lists' sections, each {heading, kind, rows}: a section per kind (its rows in the exe's order), then
        // the rows of no kind.
        private function sections():Array
        {
            var out:Array = [];
            var k:int;
            for (k = 0; k < KINDS.length; k++)
            {
                out.push({heading: k < kindNames.length ? kindNames[k] : "", kind: k, rows: []});
            }
            var other:Object = {heading: Settings.get_label("s_game"), kind: -1, rows: []};
            for each (var row:Object in shownRows())
            {
                k = kindOf(row.action_id);
                if (k == -1)
                {
                    other.rows.push(row);
                }
                else
                {
                    out[k].rows.push(row);
                }
            }
            out.push(other);
            return out.filter(function(s:Object, i:int, a:Array):Boolean { return s.rows.length > 0; });
        }

        // The columns for the sections at most `tall` high: each column's pieces, a heading ({section, top}; none at
        // a column's top where its rows go on) or a run of rows ({section, top, rows}).
        private static function flow(sections:Array, tall:Number):Array
        {
            var columns:Array = [[]];
            var y:Number = 0;
            for each (var section:Object in sections)
            {
                var rows:Array = section.rows;
                if (y > 0 && y + LIST_HEADING + Math.min(2, rows.length) * LIST_ROW > tall)
                {
                    columns.push([]);
                    y = 0;
                }
                columns[columns.length - 1].push({section: section, top: y});
                y += LIST_HEADING;
                var run:Object = null;
                for (var i:int = 0; i < rows.length; i++)
                {
                    if (y + LIST_ROW > tall && y > 0)
                    {
                        columns.push([]);
                        y = 0;
                        run = null;
                    }
                    if (run == null)
                    {
                        run = {section: section, top: y, rows: []};
                        columns[columns.length - 1].push(run);
                    }
                    run.rows.push(rows[i]);
                    y += LIST_ROW;
                }
                y += LIST_ROW / 2;
            }
            return columns;
        }

        private static function columnHeight(column:Array):Number
        {
            var last:Object = column[column.length - 1];
            return last.top + (last.rows != null ? last.rows.length * LIST_ROW : LIST_HEADING);
        }

        // The window over the screen, at most MAX_WIDTH wide: the lists in as many columns as its width takes, the
        // keyboard the largest the rest of the screen's height leaves (to MAX_UNIT); shrunk as a whole only on a
        // screen too small for the least unit.
        private function build():void
        {
            clear();
            var from:Point = parent.globalToLocal(new Point(0, 0));
            var to:Point = parent.globalToLocal(new Point(stage.stageWidth, stage.stageHeight));
            var screenWidth:Number = to.x - from.x, screenHeight:Number = to.y - from.y;
            var g:Graphics = backdrop.graphics;
            g.clear();
            g.beginFill(0x000000, 0.6);
            g.drawRect(from.x, from.y, screenWidth, screenHeight);
            g.endFill();

            var parts:Array = sections();
            var roomWidth:Number = screenWidth - 2 * MARGIN - FRAME_LEFT - FRAME_RIGHT - 2 * SIDE;
            var roomHeight:Number = screenHeight - 2 * MARGIN;
            var fixed:Number = FRAME_TOP + SPACE + SPACE + CHIP_HEIGHT + SPACE + SPACE + SPACE + FRAME_BOTTOM;
            // the lists as many columns as the window's width takes; the keyboard the largest the rest leaves
            var width:Number = Math.min(roomWidth, MAX_WIDTH);
            keyBox = KEY_BOX;
            var probe:TextField = text(KEY_FONT, 0xFFFFFF, true);
            for each (var key:Object in binder)
            {
                probe.text = String(key.text);
                keyBox = Math.max(keyBox, Math.ceil(probe.textWidth) + 12);
            }
            var columns:Array = listColumns(parts, Math.max(1, Math.floor((width + LIST_GAP) / (LIST_COLUMN + keyBox - KEY_BOX + LIST_GAP))));
            var tallest:Number = 0;
            for each (var column:Array in columns)
            {
                tallest = Math.max(tallest, columnHeight(column));
            }
            unit = Math.floor(Math.min(MAX_UNIT, width / (DESK_UNITS + MOUSE_GAP + MOUSE_UNITS),
                (roomHeight - fixed - tallest) / DESK_ROWS));
            unit = Math.max(MIN_UNIT, unit);
            var w:Number = FRAME_LEFT + FRAME_RIGHT + 2 * SIDE + width;
            var deskTop:Number = FRAME_TOP + SPACE;
            var chipsTop:Number = deskTop + unit * DESK_ROWS + SPACE;
            var listTop:Number = chipsTop + CHIP_HEIGHT + SPACE + SPACE;
            var h:Number = listTop + tallest + SPACE + FRAME_BOTTOM;
            var left:Number = FRAME_LEFT + SIDE;
            var deskWidth:Number = unit * (DESK_UNITS + MOUSE_GAP + MOUSE_UNITS);
            var deskLeft:Number = left + Math.round((width - deskWidth) / 2);
            drawMouse(deskLeft + unit * (DESK_UNITS + MOUSE_GAP), deskTop);
            drawCaps(capsOf(deskLeft, deskTop));
            legend(left, chipsTop, width);
            drawLists(columns, left, listTop, width);
            drawFrame(w, h);
            titleField.text = title;
            titleField.x = Math.round((w - titleField.width) / 2);
            titleField.y = TITLE_Y;
            closeButton.x = w - CLOSE_RIGHT;
            closeButton.y = TITLE_Y;
            var scale:Number = Math.min(1, (screenWidth - 2 * MARGIN) / w, (screenHeight - 2 * MARGIN) / h);
            panel.scaleX = panel.scaleY = scale;
            panel.x = Math.round(from.x + (screenWidth - w * scale) / 2);
            panel.y = Math.round(from.y + (screenHeight - h * scale) / 2);
        }

        // The lists in `count` columns, as short as they can be.
        private static function listColumns(parts:Array, count:int):Array
        {
            var total:Number = 0;
            for each (var section:Object in parts)
            {
                total += LIST_HEADING + section.rows.length * LIST_ROW + LIST_ROW / 2;
            }
            for (var tall:Number = Math.ceil(total / count / LIST_ROW) * LIST_ROW; ; tall += LIST_ROW)
            {
                var columns:Array = flow(parts, tall);
                if (columns.length <= count)
                {
                    return columns;
                }
            }
            return null;
        }

        // The rows that have a key name now: the exe's table's, or (before it answers, or for a key it does
        // not name) the binder key whose text is the row's.
        private function keyName(row:Object):String
        {
            var name:String = rowKeys[row.action_id];
            if (name != null && binder[name] != undefined && binder[name].text == row.key)
            {
                return name;
            }
            if (row.key == null || row.key == "")
            {
                return "";
            }
            for (var each:String in binder)
            {
                if (binder[each].text == row.key)
                {
                    return each;
                }
            }
            return "";
        }

        // The rows each key has: {name: [rows]}.
        private function rowsByKey():Object
        {
            var out:Object = {};
            for each (var row:Object in shownRows())
            {
                var name:String = keyName(row);
                if (name != "")
                {
                    (out[name] = out[name] || []).push(row);
                }
            }
            return out;
        }

        // The keys to draw, each [name, x, y, w, h, rows]: the keyboard's, the mouse's, and the bound ones the
        // keyboard and the mouse do not draw (the numpad's, the mouse's other buttons) under the mouse.
        private function capsOf(x:Number, y:Number):Array
        {
            var bound:Object = rowsByKey();
            var drawn:Object = {};
            var out:Array = [];
            for each (var key:Array in KEYBOARD)
            {
                drawn[key[0]] = true;
                out.push([key[0], x + key[1] * unit, y + key[2] * unit, key[3] * unit, unit, bound[key[0]]]);
            }
            var mouseX:Number = x + unit * (DESK_UNITS + MOUSE_GAP);
            for each (var part:Array in MOUSE)
            {
                drawn[part[0]] = true;
                out.push([part[0], mouseX + part[1] * unit, y + part[2] * unit, part[3] * unit, part[4] * unit, bound[part[0]]]);
            }
            var n:int = 0;
            for (var name:String in bound)
            {
                if (!drawn[name])
                {
                    out.push([name, mouseX + (n % 3) * unit, y + unit * (MOUSE_HEIGHT + 0.35) + int(n / 3) * unit, unit, unit,
                        bound[name]]);
                    n++;
                }
            }
            return out;
        }

        // The keys drawn with one size for every key's name (the unit's) and one for every key's actions (the
        // owner, 2026-11-23: one size for all, none shrunk alone): the largest, from the names' down to
        // MIN_ACTION_SIZE, at which every key's actions fit under its name, wrapped by words to its width; at the
        // least size an action's last lines are cut at its key's foot (the lists under the keyboard have every
        // action whole).
        private function drawCaps(caps:Array):void
        {
            legendSize = Math.max(9, Math.round(unit * 0.2));
            var probe:TextField = text(legendSize, INK, true);
            probe.multiline = probe.wordWrap = true;
            var room:Function = function(c:Array):Number {
                probe.defaultTextFormat = sized(probe.defaultTextFormat, legendSize);
                probe.width = inner(c[3]) - 6;
                probe.text = legendOf(c[0]);
                return inner(c[4]) - (probe.textHeight + 2) - 4;
            };
            var fits:Function = function(c:Array):Boolean {
                var actions:String = actionsOf(c[5]);
                if (actions == "")
                {
                    return true;
                }
                var height:Number = room(c);
                probe.defaultTextFormat = sized(probe.defaultTextFormat, actionSize);
                probe.width = inner(c[3]) - 6;
                probe.text = actions;
                return probe.textHeight + 2 <= height;
            };
            for (actionSize = legendSize; actionSize > MIN_ACTION_SIZE && !caps.every(
                function(c:Array, i:int, a:Array):Boolean { return fits(c); }); actionSize--)
            {
            }
            for each (var c:Array in caps)
            {
                cap(c[0], c[1], c[2], c[3], c[4], c[5]);
            }
        }

        private static function sized(format:TextFormat, size:Number):TextFormat
        {
            format.size = size;
            return format;
        }

        private function inner(size:Number):Number
        {
            return size - Math.max(3, Math.round(unit * 0.09));
        }

        private function legendOf(name:String):String
        {
            var key:Object = binder[name];
            return key != null ? String(key.text) : name.replace(/^k/, "");
        }

        private static function actionsOf(rows:Array):String
        {
            return (rows || []).map(function(r:Object, i:int, a:Array):String { return label(r); }).join(" · ");
        }

        // The mouse's shell (its parts are keys: capsOf).
        private function drawMouse(x:Number, y:Number):void
        {
            var shell:Shape = new Shape();
            var g:Graphics = shell.graphics;
            g.lineStyle(1, BOX_EDGE_FREE, 1, true);
            g.beginFill(BOX_FILL, 0.85);
            g.drawRoundRect(x, y, MOUSE_UNITS * unit, MOUSE_HEIGHT * unit, unit * 2.4, unit * 2.4);
            g.endFill();
            body.addChild(shell);
        }

        // A key: its name's text in its corner; its rows' actions on it; tinted by its first row's kind; plain when
        // free, inert when the binder has no such key (or it is Esc).
        private function cap(name:String, x:Number, y:Number, w:Number, h:Number, rows:Array):void
        {
            var gap:Number = Math.max(3, Math.round(unit * 0.09));
            var key:Object = binder[name];
            var live:Boolean = key != null && key.code != 1;
            var sprite:Sprite = new Sprite();
            sprite.name = NAME + "_key_" + name;
            sprite.x = Math.round(x);
            sprite.y = Math.round(y);
            sprite.mouseChildren = false;
            var match:Array = rows || [];
            var first:Object = match.length > 0 ? match[0] : null;
            var kind:int = first != null ? kindOf(first.action_id) : -1;
            var view:Object = {name: name, sprite: sprite, kinds: kindsOf(rows), rows: rows || [], w: w - gap, h: h - gap,
                kind: kind, live: live};
            paintCap(view, false);
            var legendField:TextField = text(legendSize, first != null ? 0xFFFFFF : 0x8A8D90, true);
            legendField.multiline = legendField.wordWrap = true;
            legendField.width = view.w - 6;
            legendField.text = legendOf(name);
            legendField.x = 3;
            legendField.y = 1;
            sprite.addChild(legendField);
            var top:Number = legendField.height - 2;
            var actions:String = actionsOf(match);
            if (actions != "")
            {
                var field:TextField = text(actionSize, INK, true);
                field.multiline = field.wordWrap = true;
                field.width = view.w - 6;
                field.text = actions;
                field.x = 3;
                field.y = top;
                sprite.addChild(field);
            }
            // every text inside the key (a field is as tall as its lines; the last ones cut)
            for (var t:int = 0; t < sprite.numChildren; t++)
            {
                var inside:TextField = sprite.getChildAt(t) as TextField;
                if (inside != null && inside.y + inside.height > view.h)
                {
                    inside.autoSize = TextFieldAutoSize.NONE;
                    inside.height = Math.max(4, view.h - inside.y);
                }
            }
            if (live)
            {
                sprite.addEventListener(MouseEvent.ROLL_OVER, function(e:MouseEvent):void { paintCap(view, true); });
                sprite.addEventListener(MouseEvent.ROLL_OUT, function(e:MouseEvent):void { paintCap(view, false); });
                sprite.addEventListener(MouseEvent.CLICK, function(e:MouseEvent):void { pressKey(view); });
            }
            else
            {
                sprite.alpha = 0.45;
            }
            body.addChild(sprite);
            keyViews.push(view);
        }

        // A key's box in the stock key box's look: black, a light frame (dim when free), its kind's bar along its
        // foot and a faint wash of it; lighter under the pointer; the game's orange while its wait runs.
        private function paintCap(view:Object, over:Boolean):void
        {
            var g:Graphics = view.sprite.graphics;
            g.clear();
            var waiting:Boolean = awaited != null && view.rows.indexOf(awaited) != -1;
            var edge:uint = waiting ? AWAITED : over ? 0xFFFFFF : view.kind != -1 || view.rows.length > 0 ? BOX_EDGE : BOX_EDGE_FREE;
            g.lineStyle(1, edge, 1, true);
            g.beginFill(over ? 0x1C1E21 : BOX_FILL, 0.88);
            g.drawRect(0.5, 0.5, view.w - 1, view.h - 1);
            g.endFill();
            g.lineStyle();
            if (view.kind != -1)
            {
                var colour:uint = KINDS[view.kind].colour;
                g.beginFill(colour, 0.13);
                g.drawRect(1, 1, view.w - 2, view.h - 2);
                g.endFill();
                g.beginFill(colour, 0.95);
                g.drawRect(1, view.h - 4, view.w - 2, 3);
                g.endFill();
            }
        }

        private static function kindsOf(rows:Array):Array
        {
            var out:Array = [];
            for each (var row:Object in rows || [])
            {
                out.push(kindOf(row.action_id));
            }
            return out;
        }

        // A row's action as it reads on a key: its label without trailing blanks.
        private static function label(row:Object):String
        {
            return String(row.label).replace(/\s+$/, "");
        }

        // The kinds' chips: a square of the kind's colour and its name, in the headings' look; the pointer over one
        // dims every key and row of another kind.
        private function legend(x:Number, y:Number, width:Number):void
        {
            var at:Number = 0;
            for (var k:int = 0; k < KINDS.length && k < kindNames.length; k++)
            {
                var chip:Sprite = new Sprite();
                chip.name = NAME + "_chip_" + k;
                chip.mouseChildren = false;
                var field:TextField = text(14, HEADING_COLOUR, true);
                field.text = kindNames[k];
                field.x = 18;
                field.y = Math.round((CHIP_HEIGHT - field.height) / 2);
                chip.addChild(field);
                var g:Graphics = chip.graphics;
                g.beginFill(0x000000, 0.01);
                g.drawRect(0, 0, field.x + field.width + 4, CHIP_HEIGHT);
                g.endFill();
                g.beginFill(KINDS[k].colour);
                g.drawRect(2, Math.round(CHIP_HEIGHT / 2) - 5, 10, 10);
                g.endFill();
                chip.x = at;
                chip.addEventListener(MouseEvent.ROLL_OVER, onChipOver);
                chip.addEventListener(MouseEvent.ROLL_OUT, onChipOut);
                chips.addChild(chip);
                at += field.x + field.width + 26;
            }
            chips.x = Math.round(x + (width - (at - 26)) / 2);
            chips.y = Math.round(y);
            body.addChild(chips);
            var g2:Graphics = body.graphics;
            g2.beginFill(0xFFFFFF, 0.07);
            g2.drawRect(x, y + CHIP_HEIGHT + SPACE, width, 1);
            g2.endFill();
        }

        private function onChipOver(event:MouseEvent):void
        {
            focus(chips.getChildIndex(event.currentTarget as DisplayObject));
        }

        private function onChipOut(event:MouseEvent):void
        {
            focus(-1);
        }

        // Every key and row of kind `kind` (-1: all) at full strength, the others faded; the chips likewise.
        private function focus(kind:int):void
        {
            for each (var key:Object in keyViews)
            {
                if (key.live)
                {
                    key.sprite.alpha = kind == -1 || key.kinds.indexOf(kind) != -1 ? 1 : FADED;
                }
            }
            for each (var view:Object in rowViews)
            {
                view.sprite.alpha = kind == -1 || view.kind == kind ? 1 : FADED;
            }
            for (var k:int = 0; k < chips.numChildren; k++)
            {
                chips.getChildAt(k).alpha = kind == -1 || k == kind ? 1 : 0.5;
            }
        }

        // The lists: each section's heading in the headings' look, a line of its kind's colour under it; each row
        // its action and its key in the stock key box's look.
        private function drawLists(columns:Array, x:Number, y:Number, width:Number):void
        {
            var count:int = columns.length;
            var columnWidth:Number = Math.floor((width - (count - 1) * LIST_GAP) / Math.max(1, count));
            for (var c:int = 0; c < count; c++)
            {
                var left:Number = x + c * (columnWidth + LIST_GAP);
                for each (var piece:Object in columns[c])
                {
                    if (piece.rows == null)
                    {
                        listHeading(piece.section, left, y + piece.top, columnWidth);
                    }
                    else
                    {
                        for (var i:int = 0; i < piece.rows.length; i++)
                        {
                            listRow(piece.rows[i], left, y + piece.top + i * LIST_ROW, columnWidth);
                        }
                    }
                }
            }
        }

        private function listHeading(section:Object, x:Number, y:Number, width:Number):void
        {
            var field:TextField = text(13, HEADING_COLOUR, true);
            field.name = NAME + "_heading_" + body.numChildren;
            var heading:String = String(section.heading).replace(/\s*:\s*$/, "");
            field.text = heading + ":";
            field.x = x - 2;
            field.y = y + 1;
            body.addChild(field);
            var g:Graphics = body.graphics;
            g.beginFill(section.kind == -1 ? HEADING_COLOUR : KINDS[section.kind].colour, section.kind == -1 ? 0.35 : 0.8);
            g.drawRect(x, y + LIST_HEADING - 5, width, 2);
            g.endFill();
        }

        // A row: its label, its key in a small key box at its right; a press asks the exe for a key (the stock
        // wait).
        private function listRow(row:Object, x:Number, y:Number, width:Number):void
        {
            var sprite:Sprite = new Sprite();
            sprite.name = NAME + "_row_" + row.action_id;
            sprite.mouseChildren = false;
            sprite.x = Math.round(x);
            sprite.y = Math.round(y);
            var view:Object = {row: row, sprite: sprite, kind: kindOf(row.action_id), width: width};
            var field:TextField = text(12, INK, true);
            field.text = label(row);
            field.x = 0;
            field.y = Math.round((LIST_ROW - field.height) / 2);
            fitLine(field, width - keyBox - 8);
            sprite.addChild(field);
            var keyField:TextField = text(KEY_FONT, 0xFFFFFF, true);
            keyField.autoSize = TextFieldAutoSize.NONE;
            keyField.width = keyBox - 4;
            keyField.height = LIST_ROW - 4;
            var centred:TextFormat = keyField.defaultTextFormat;
            centred.align = TextFormatAlign.CENTER;
            keyField.defaultTextFormat = centred;
            keyField.text = row.key != null ? String(row.key) : "";
            keyField.x = width - keyBox + 2;
            keyField.y = 1;
            sprite.addChild(keyField);
            view.keyField = keyField;
            paintRow(view, false);
            sprite.addEventListener(MouseEvent.ROLL_OVER, function(e:MouseEvent):void { paintRow(view, true); });
            sprite.addEventListener(MouseEvent.ROLL_OUT, function(e:MouseEvent):void { paintRow(view, false); });
            sprite.addEventListener(MouseEvent.CLICK, function(e:MouseEvent):void { startWait(view.row); });
            body.addChild(sprite);
            rowViews.push(view);
        }

        private function paintRow(view:Object, over:Boolean):void
        {
            var g:Graphics = view.sprite.graphics;
            g.clear();
            g.beginFill(0xFFFFFF, over ? 0.07 : 0.01);
            g.drawRect(-4, 0, view.width + 4, LIST_ROW - 1);
            g.endFill();
            var free:Boolean = view.row.key == null || view.row.key == "";
            var waiting:Boolean = awaited == view.row;
            g.lineStyle(1, waiting ? AWAITED : free ? BOX_EDGE_FREE : BOX_EDGE, 1, true);
            g.beginFill(BOX_FILL, 0.9);
            g.drawRect(view.width - keyBox + 0.5, 1.5, keyBox - 1, LIST_ROW - 4);
            g.endFill();
            g.lineStyle();
            if (view.kind != -1)
            {
                g.beginFill(KINDS[view.kind].colour, free ? 0.3 : 0.9);
                g.drawRect(-4, 2, 2, LIST_ROW - 5);
                g.endFill();
            }
        }

        // A press of a key: the selector asked which action it takes, or (without one) the stock wait for its first
        // action.
        private function pressKey(view:Object):void
        {
            var key:Object = binder[view.name];
            if (key == null || awaited != null)
            {
                return;
            }
            if (selector != null)
            {
                var code:int = key.code;
                selector({key: view.name, code: code, keyName: key.text, actions: view.rows, rows: shownRows(), kinds: KINDS},
                    function(action:int):void {
                        if (action >= 0)
                        {
                            SrpExe.call("srp_bind_key", action + " " + code);
                        }
                        else if (action == UNBIND)
                        {
                            SrpExe.call("srp_unbind_key", String(code));
                        }
                    });
                return;
            }
            if (view.rows.length > 0)
            {
                startWait(view.rows[0]);
            }
        }

        // The stock wait for a row's key, as its key box starts it (BindKeyRender.startKey): the exe asked
        // (start_bind_key), the menu told (its "press a key" message) by an event whose target's data is the row;
        // it ends with the menu's end_keybind (its Event.CANCEL).
        private function startWait(row:Object):void
        {
            if (awaited != null || row.type == 0)
            {
                return;
            }
            awaited = row;
            try
            {
                ExternalInterface.call("start_bind_key", row.action_id);
            }
            catch (error:Error)
            {
            }
            stage.addEventListener(Event.CANCEL, onWaitEnd, true);
            var carrier:MovieClip = new MovieClip();
            carrier.data = row;
            body.addChild(carrier);
            carrier.dispatchEvent(new Event(BindKeyRender.START_KEYBIND));
            body.removeChild(carrier);
            redrawWait();
        }

        private function onWaitEnd(event:Event):void
        {
            stage.removeEventListener(Event.CANCEL, onWaitEnd, true);
            awaited = null;
            redrawWait();
        }

        private function redrawWait():void
        {
            for each (var view:Object in rowViews)
            {
                paintRow(view, false);
            }
            for each (var key:Object in keyViews)
            {
                if (key.live)
                {
                    paintCap(key, false);
                }
            }
        }

        // The Settings window's frame at w x h: its art's border and title band as they are, its inside stretched.
        private function drawFrame(w:Number, h:Number):void
        {
            var art:BitmapData = new SrpSettingsGround();
            var g:Graphics = frame.graphics;
            g.clear();
            var xs:Array = [0, FRAME_LEFT, art.width - FRAME_RIGHT, art.width];
            var ys:Array = [0, FRAME_TOP, art.height - FRAME_BOTTOM, art.height];
            var tx:Array = [0, FRAME_LEFT, w - FRAME_RIGHT, w];
            var ty:Array = [0, FRAME_TOP, h - FRAME_BOTTOM, h];
            for (var i:int = 0; i < 3; i++)
            {
                for (var j:int = 0; j < 3; j++)
                {
                    var sx:Number = (tx[i + 1] - tx[i]) / (xs[i + 1] - xs[i]);
                    var sy:Number = (ty[j + 1] - ty[j]) / (ys[j + 1] - ys[j]);
                    var m:Matrix = new Matrix(sx, 0, 0, sy, tx[i] - xs[i] * sx, ty[j] - ys[j] * sy);
                    g.beginBitmapFill(art, m, false, true);
                    g.drawRect(tx[i], ty[j], tx[i + 1] - tx[i], ty[j + 1] - ty[j]);
                    g.endFill();
                }
            }
            panel.graphics.clear();
            panel.graphics.beginFill(0x000000, 0.01);
            panel.graphics.drawRect(0, 0, w, h);
            panel.graphics.endFill();
        }

        // A text in the Settings' own font (its title's, Calibri), on one line.
        private function text(size:Number, colour:uint, bold:Boolean):TextField
        {
            var stock:TextField = content.parent != null ? Object(content.parent).textFieldLabel as TextField : null;
            var format:TextFormat = stock != null ? stock.getTextFormat() : new TextFormat();
            format.size = size;
            format.color = colour;
            format.bold = bold;
            format.align = TextFormatAlign.LEFT;
            format.leading = 0;
            var field:TextField = new TextField();
            field.embedFonts = stock != null && stock.embedFonts;
            field.defaultTextFormat = format;
            field.autoSize = TextFieldAutoSize.LEFT;
            field.selectable = false;
            field.mouseEnabled = false;
            return field;
        }

        // A one-line text a point smaller at a time (to 8) while it runs past `width`, cut there when it still does.
        private static function fitLine(field:TextField, width:Number):void
        {
            var format:TextFormat = field.defaultTextFormat;
            var size:int = int(format.size);
            while (field.textWidth + 4 > width && size > 8)
            {
                size--;
                format.size = size;
                field.defaultTextFormat = format;
                field.setTextFormat(format);
            }
            if (field.textWidth + 4 > width)
            {
                var height:Number = field.textHeight + 4;
                field.autoSize = TextFieldAutoSize.NONE;
                field.width = Math.max(4, width);
                field.height = height;
            }
        }

        // The keyboard's keys: [name, x, y, width] in key units.
        private static function keyboard():Array
        {
            var out:Array = [];
            var row:Function = function(y:Number, x:Number, keys:Array):void {
                for each (var key:* in keys)
                {
                    var name:String = key is Array ? key[0] : key;
                    var w:Number = key is Array ? key[1] : 1;
                    if (name != "")
                    {
                        out.push([name, x, y, w]);
                    }
                    x += w;
                }
            };
            row(0, 0, ["kESCAPE", ["", 1], "kF1", "kF2", "kF3", "kF4", ["", 0.5], "kF5", "kF6", "kF7", "kF8", ["", 0.5],
                "kF9", "kF10", "kF11", "kF12", ["", 0.25], "kSYSRQ", "kSCROLL", "kPAUSE"]);
            row(1.25, 0, ["kGRAVE", "k1", "k2", "k3", "k4", "k5", "k6", "k7", "k8", "k9", "k0", "kMINUS", "kEQUALS", ["kBACK", 2],
                ["", 0.25], "kINSERT", "kHOME", "kPRIOR"]);
            row(2.25, 0, [["kTAB", 1.5], "kQ", "kW", "kE", "kR", "kT", "kY", "kU", "kI", "kO", "kP", "kLBRACKET", "kRBRACKET",
                ["kBACKSLASH", 1.5], ["", 0.25], "kDELETE", "kEND", "kNEXT"]);
            row(3.25, 0, [["kCAPITAL", 1.75], "kA", "kS", "kD", "kF", "kG", "kH", "kJ", "kK", "kL", "kSEMICOLON", "kAPOSTROPHE",
                ["kRETURN", 2.25]]);
            row(4.25, 0, [["kLSHIFT", 2.25], "kZ", "kX", "kC", "kV", "kB", "kN", "kM", "kCOMMA", "kPERIOD", "kSLASH",
                ["kRSHIFT", 2.75], ["", 1.25], "kUP"]);
            row(5.25, 0, [["kLCONTROL", 1.25], ["kLWIN", 1.25], ["kLMENU", 1.25], ["kSPACE", 6.25], ["kRMENU", 1.25],
                ["kRWIN", 1.25], ["kAPPS", 1.25], ["kRCONTROL", 1.25], ["", 0.25], "kLEFT", "kDOWN", "kRIGHT"]);
            return out;
        }
    }
}
