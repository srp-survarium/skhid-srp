package
{
    import flash.display.DisplayObject;
    import flash.display.GradientType;
    import flash.display.Graphics;
    import flash.display.MovieClip;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.filters.DropShadowFilter;
    import flash.geom.Matrix;
    import flash.geom.Point;
    import flash.text.TextField;
    import flash.text.TextFieldAutoSize;
    import flash.text.TextFormat;
    import scaleform.clik.data.DataProvider;
    import scaleform.clik.events.ButtonEvent;
    import survarium.constants.KeyBindGroup;
    import survarium.menu.Settings;

    // The Controls tab's "Key bindings" (the owner, 2026-11-23: the tab's list "is so small, it will be
    // painful to rebind"; "you press on it and big window opens"; "this needs good modern design"): a button
    // at the foot of the tab's left column, beside Default settings, opens a window over the screen with
    // every key row at once, in columns: the match's keys, the lobby's, the chat's (each group under its
    // heading, in the exe's order), as many columns as the screen's height needs. Each row is tinted by its
    // kind of action (movement, combat, items, communication, interface: KINDS), brighter when a key is
    // bound; the kinds' chips in the title's band name the colours, and the pointer over one dims every
    // other kind's rows. Its rows are the tab's: the same objects the exe gave (root.set_keybindings), drawn
    // by the stock row (SrvBindKeyListRender), so a press of a key box is the stock rebinding
    // (start_bind_key, the exe's "press a key" message, its conflict message, root.set_keybind) and a change
    // is the tab's change, saved by the Settings' Apply. Esc, the close button or a press beside the window
    // closes it. Its texts (the button's and the title, the kinds' names) are the exe's (root.srp_hud_texts'
    // 16th to 21st): the button shows once they are there. A script-added child of the Settings window, over
    // its tabs, sized to the screen as the menu scales it (MainMenu: 1 at 1080p and below), so at 1280 x 720
    // as at 1920 x 1080 the window fits the screen.
    public class SrpKeysWindow extends Sprite
    {
        public static const NAME:String = "_srp_keys_window";
        public static const BUTTON:String = "_srp_keys_button";
        private static const KEYS_FRAME:int = 2;
        // The kinds of action, by the exe's action ids (item_hotkeys_026g.asm's `actions`), and their colours.
        public static const KINDS:Array = [
            {colour: 0x4FB8AD, ids: [0x04, 0x05, 0x06, 0x07, 0x08, 0x0B, 0x0C, 0x0D, 0x0E]},
            {colour: 0xE3953C, ids: [0x09, 0x0A, 0x1D, 0x1E, 0x20, 0x21, 0x22, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A,
                0x35, 0x36]},
            {colour: 0x94C451, ids: [0x23, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50,
                0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59]},
            {colour: 0x5EA3EA, ids: [0x37, 0x3F, 0x45, 0x46, 0x47]},
            {colour: 0xA397C4, ids: [0x19, 0x1A, 0x1B, 0x40, 0x41, 0x42, 0x43, 0x44]}];
        // The stock row (SrvBindKeyListRender): its height (the tab's lists' rowHeight) and its label's left.
        private static const ROW:Number = 30;
        private static const LABEL_X:Number = 65;
        // A column: at least the stock row's width, at most a little more for long labels.
        private static const MIN_COLUMN:Number = 270;
        private static const MAX_COLUMN:Number = 320;
        private static const GAP:Number = 32;           // between columns
        private static const PAD:Number = 28;           // inside the window's edges
        private static const HEADER:Number = 64;        // the title's band
        private static const HEADING:Number = 30;       // a group's heading
        private static const SECTION_GAP:Number = 16;   // over a heading below another group's rows
        private static const MARGIN:Number = 24;        // to the screen's edges at the least
        private static const TINT_X:Number = 10;        // a row's tint from left of its key box
        private static const CHIP_HEIGHT:Number = 26;
        private static const CHIP_GAP:Number = 8;
        // The groups in the window's order and their headings (the tab's own labels).
        private static const GROUPS:Array = ["s_game", "s_lobby", "s_chat_keys"];
        // The board's palette: its surface lit from above, its edge, the title's and the headings' ink.
        private static const SURFACE_TOP:uint = 0x1A1D20;
        private static const SURFACE_BOTTOM:uint = 0x121417;
        private static const EDGE:uint = 0x2C3238;
        private static const INK:uint = 0xE8E4DA;
        private static const DIM:uint = 0x8A8F94;
        // A row of a kind not the one the pointer is over.
        private static const FADED:Number = 0.2;

        // The tab's button that opens the window.
        public var button:SetingsButton;
        private var content:MovieClip;
        // The rows as the exe gave them (Settings' _keys), and the handler of a row's change (Settings').
        private var keys:Array = null;
        private var changed:Function;
        private var title:String = "";
        private var kindNames:Array = [];
        // The window's parts: the dimmed screen under it, its frame and surface, the title, the kinds' chips,
        // the close button, and the body: the lists (one per column's run of a group's rows), the headings and
        // each row's tint ({tint, list, index, kind, data}).
        private var backdrop:Sprite;
        private var panel:Sprite;
        private var frame:Shape;
        private var titleField:TextField;
        private var chips:Sprite;
        private var closeButton:ButtonCloseWindow2;
        private var body:Sprite;
        private var lists:Array = [];
        private var rowViews:Array = [];
        private var columnWidth:Number = MIN_COLUMN;
        // The stock row's label size (read from the first row made); 0 until then.
        private var labelSize:int = 0;

        public function SrpKeysWindow(content:MovieClip, changed:Function)
        {
            this.content = content;
            this.changed = changed;
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
            frame.filters = [new DropShadowFilter(12, 90, 0x000000, 0.75, 40, 40, 1, 2)];
            panel.addChild(frame);
            titleField = text(20, INK, 1.5);
            titleField.name = NAME + "_title";
            panel.addChild(titleField);
            chips = new Sprite();
            chips.name = NAME + "_chips";
            panel.addChild(chips);
            closeButton = new ButtonCloseWindow2();
            closeButton.name = NAME + "_close";
            closeButton.alpha = 1;
            closeButton.addEventListener(ButtonEvent.CLICK, onClose);
            panel.addChild(closeButton);
            body = new Sprite();
            body.name = NAME + "_body";
            panel.addChild(body);
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

        // After the tab's layout (Settings.layoutControls): at the foot of the left column as Default settings
        // is at the right column's, `shift` to the left of it (the left column's lists' x to the right's).
        public function layout(defaults:DisplayObject, shift:Number):void
        {
            button.width = defaults.width;
            button.x = Math.round(defaults.x + shift);
            button.y = defaults.y;
        }

        // The rows (Settings.applyKeys, at each bind of the Controls tab): the window's lists, when it is open.
        public function rows(keys:Array):void
        {
            this.keys = keys;
            if (open)
            {
                build();
            }
        }

        // A key set or kept (root.set_keybind): the rows drawn again.
        public function refresh():void
        {
            for each (var list:FilterInventoryList in lists)
            {
                list.invalidateData();
            }
            for each (var view:Object in rowViews)
            {
                tint(view);
            }
            addEventListener(Event.ENTER_FRAME, settle);
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
            for each (var list:FilterInventoryList in lists)
            {
                list.removeEventListener(Event.CHANGE, changed);
            }
            lists = [];
            rowViews = [];
            body.removeChildren();
            body.graphics.clear();
            chips.removeChildren();
        }

        // The groups' rows in the window's order: the match's (any group but the lobby's and the chat's, as the
        // tab's game list has them), the lobby's, the chat's.
        private function groups():Array
        {
            var out:Array = [[], [], []];
            for each (var key:Object in keys)
            {
                out[key.group_id == KeyBindGroup.LOBBY ? 1 : key.group_id == KeyBindGroup.CHAT ? 2 : 0].push(key);
            }
            return out;
        }

        // The pieces of each column for columns at most `tall` high: a group's heading (none at a column's top
        // where its rows go on from the last column) and its runs of rows, each with its top.
        private static function flow(groups:Array, tall:Number):Array
        {
            var columns:Array = [[]];
            var y:Number = 0;
            for (var g:int = 0; g < groups.length; g++)
            {
                var rows:Array = groups[g];
                if (rows.length == 0)
                {
                    continue;
                }
                var gap:Number = y > 0 ? SECTION_GAP : 0;
                // a heading starts a column rather than end one: two rows under it at least
                if (y > 0 && y + gap + HEADING + Math.min(2, rows.length) * ROW > tall)
                {
                    columns.push([]);
                    y = gap = 0;
                }
                columns[columns.length - 1].push({group: g, top: y + gap});
                y += gap + HEADING;
                var run:Object = null;
                for (var i:int = 0; i < rows.length; i++)
                {
                    if (y + ROW > tall && y > 0)
                    {
                        columns.push([]);
                        y = 0;
                        run = null;
                    }
                    if (run == null)
                    {
                        run = {group: g, top: y, rows: []};
                        columns[columns.length - 1].push(run);
                    }
                    run.rows.push(rows[i]);
                    y += ROW;
                }
            }
            return columns;
        }

        private static function columnHeight(column:Array):Number
        {
            var last:Object = column[column.length - 1];
            return last.top + (last.rows != null ? last.rows.length * ROW : HEADING);
        }

        // The window over the screen: as few columns as the screen's height allows, of even heights, as wide as
        // the screen's width allows; shrunk as a whole only on a screen too small for them.
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

            var parts:Array = groups();
            var total:Number = 0;
            for each (var rows:Array in parts)
            {
                total += rows.length > 0 ? SECTION_GAP + HEADING + rows.length * ROW : 0;
            }
            var room:Number = screenHeight - 2 * MARGIN - HEADER - PAD;
            var columns:Array = null;
            for (var n:int = 1; columns == null; n++)
            {
                // the shortest columns this many can be (whole rows), up to the room the screen has
                for (var tall:Number = Math.ceil(total / n / ROW) * ROW; tall <= Math.max(room, ROW * 4); tall += ROW)
                {
                    var tried:Array = flow(parts, tall);
                    if (tried.length <= n)
                    {
                        columns = tried;
                        break;
                    }
                }
            }
            var count:int = columns.length;
            var across:Number = screenWidth - 2 * MARGIN - 2 * PAD - (count - 1) * GAP;
            columnWidth = Math.max(MIN_COLUMN, Math.min(MAX_COLUMN, Math.floor(across / count)));
            var tallest:Number = 0;
            for each (var column:Array in columns)
            {
                tallest = Math.max(tallest, columnHeight(column));
            }
            var w:Number = 2 * PAD + count * columnWidth + (count - 1) * GAP;
            var h:Number = HEADER + PAD / 2 + tallest + PAD;

            for (var c:int = 0; c < count; c++)
            {
                var x:Number = PAD + c * (columnWidth + GAP);
                for each (var piece:Object in columns[c])
                {
                    if (piece.rows == null)
                    {
                        heading(piece.group, x, HEADER + PAD / 2 + piece.top);
                    }
                    else
                    {
                        run(piece.rows, x, HEADER + PAD / 2 + piece.top);
                    }
                }
            }
            draw(w, h);
            titleField.text = title.toUpperCase();
            titleField.x = PAD;
            titleField.y = Math.round((HEADER - titleField.height) / 2);
            closeButton.x = w - PAD / 2 - closeButton.width;
            closeButton.y = Math.round((HEADER - closeButton.height) / 2);
            legend(closeButton.x - 16);
            var scale:Number = Math.min(1, (screenWidth - 2 * MARGIN) / w, (screenHeight - 2 * MARGIN) / h);
            panel.scaleX = panel.scaleY = scale;
            panel.x = Math.round(from.x + (screenWidth - w * scale) / 2);
            panel.y = Math.round(from.y + (screenHeight - h * scale) / 2);
            addEventListener(Event.ENTER_FRAME, settle);
        }

        // The kinds' chips, right-aligned to `right` in the title's band: a dot of the kind's colour and its
        // name on a pill of it; the pointer over one dims every other kind.
        private function legend(right:Number):void
        {
            var x:Number = 0;
            for (var k:int = 0; k < KINDS.length && k < kindNames.length; k++)
            {
                var chip:Sprite = new Sprite();
                chip.name = NAME + "_chip_" + k;
                chip.mouseChildren = false;
                var label:TextField = text(13, INK, 0.6);
                label.text = kindNames[k];
                label.x = 24;
                label.y = Math.round((CHIP_HEIGHT - label.height) / 2);
                chip.addChild(label);
                var w:Number = Math.round(label.x + label.width + 12);
                var colour:uint = KINDS[k].colour;
                var g:Graphics = chip.graphics;
                g.lineStyle(1, colour, 0.55, true);
                g.beginFill(colour, 0.12);
                g.drawRoundRect(0.5, 0.5, w, CHIP_HEIGHT, CHIP_HEIGHT, CHIP_HEIGHT);
                g.endFill();
                g.lineStyle();
                g.beginFill(colour);
                g.drawCircle(13, CHIP_HEIGHT / 2 + 0.5, 4.5);
                g.endFill();
                chip.x = x;
                chip.addEventListener(MouseEvent.ROLL_OVER, onChipOver);
                chip.addEventListener(MouseEvent.ROLL_OUT, onChipOut);
                chips.addChild(chip);
                x += w + CHIP_GAP;
            }
            chips.x = Math.round(right - (x - CHIP_GAP));
            chips.y = Math.round((HEADER - CHIP_HEIGHT) / 2);
        }

        private function onChipOver(event:MouseEvent):void
        {
            focus(chips.getChildIndex(event.currentTarget as DisplayObject));
        }

        private function onChipOut(event:MouseEvent):void
        {
            focus(-1);
        }

        // Every row of kind `kind` (-1: all) at full strength, the others faded; the chips likewise.
        private function focus(kind:int):void
        {
            for each (var view:Object in rowViews)
            {
                var alpha:Number = kind == -1 || view.kind == kind ? 1 : FADED;
                view.tint.alpha = alpha;
                var row:Object = view.list.getRendererAt(view.index);
                if (row != null)
                {
                    row.alpha = alpha;
                }
            }
            for (var k:int = 0; k < chips.numChildren; k++)
            {
                chips.getChildAt(k).alpha = kind == -1 || k == kind ? 1 : 0.45;
            }
        }

        // A group's heading: its name (the tab's heading without its colon) in small capitals, a hairline on.
        private function heading(group:int, x:Number, y:Number):void
        {
            var field:TextField = text(12, DIM, 2);
            field.name = NAME + "_heading_" + group;
            field.text = Settings.get_label(GROUPS[group]).replace(/\s*:\s*$/, "").toUpperCase();
            field.x = x - TINT_X;
            field.y = y + Math.round((HEADING - field.height) / 2) - 2;
            body.addChild(field);
            var g:Graphics = body.graphics;
            g.beginFill(0xFFFFFF, 0.08);
            var lineX:Number = field.x + field.width + 10;
            g.drawRect(lineX, y + Math.round(HEADING / 2) - 2, Math.max(0, x + columnWidth - lineX), 1);
            g.endFill();
        }

        // A run of a group's rows: each row's tint under it, the stock list over them.
        private function run(rows:Array, x:Number, y:Number):void
        {
            var list:FilterInventoryList = new FilterInventoryList();
            for (var i:int = 0; i < rows.length; i++)
            {
                var view:Object = {tint: new Shape(), list: list, index: i, kind: kindOf(rows[i].action_id), data: rows[i]};
                view.tint.x = x - TINT_X;
                view.tint.y = y + i * ROW;
                body.addChild(view.tint);
                tint(view);
                rowViews.push(view);
            }
            list.name = NAME + "_list_" + lists.length;
            list.columnWidth = columnWidth;
            list.direction = "vertical";
            list.enabled = true;
            list.externalColumnCount = 0;
            list.focusable = true;
            list.itemRendererName = "SrvBindKeyListRender";
            list.margin = 0;
            list.noIndexOnLostFocus = true;
            list.inspectablePadding = {top: 0, right: 0, bottom: 0, left: 0};
            list.rowHeight = ROW;
            list.scrollBar = "";
            list.wrapping = "normal";
            list.alpha = 1;
            list.x = x;
            list.y = y;
            list.width = columnWidth;
            list.height = rows.length * ROW;
            list.dataProvider = new DataProvider(rows);
            list.addEventListener(Event.CHANGE, changed);
            body.addChild(list);
            lists.push(list);
        }

        // A row's tint: its kind's colour, a bar at its left edge and a wash across it, strong while a key is
        // bound, faint while none is; a row of no kind a plain band.
        private function tint(view:Object):void
        {
            var g:Graphics = view.tint.graphics;
            g.clear();
            var bound:Boolean = view.data.key != null && view.data.key != "";
            var w:Number = columnWidth + TINT_X;
            if (view.kind == -1)
            {
                g.beginFill(0xFFFFFF, 0.03);
                g.drawRect(0, 1, w, ROW - 2);
                g.endFill();
                return;
            }
            var colour:uint = KINDS[view.kind].colour;
            g.beginFill(colour, bound ? 0.11 : 0.035);
            g.drawRoundRect(0, 1, w, ROW - 2, 6, 6);
            g.endFill();
            g.beginFill(colour, bound ? 0.95 : 0.3);
            g.drawRect(0, 1, 3, ROW - 2);
            g.endFill();
        }

        // Once the lists have made their rows: the labels across the column (the stock row's field ends at the
        // tab's narrower column) and every key box's text drawn now (a script-made list's buttons otherwise
        // wait for a redraw to show their labels).
        private function settle(event:Event):void
        {
            removeEventListener(Event.ENTER_FRAME, settle);
            for each (var list:FilterInventoryList in lists)
            {
                list.validateNow();
                for (var i:int = 0; i < list.dataProvider.length; i++)
                {
                    var row:Object = list.getRendererAt(i);
                    if (row != null && row.textField != null)
                    {
                        row.textField.width = columnWidth - LABEL_X;
                        fit(row.textField);
                        row.btn_key.validateNow();
                    }
                }
            }
        }

        // A row's label on its one line: the stock row's size, a point smaller at a time (to MIN_LABEL_SIZE)
        // while it runs past the column (a long label of some dictionary on a narrow screen's columns). Kept
        // as the field's default, so the row's redraws (its label set again) keep it.
        public static const MIN_LABEL_SIZE:int = 11;

        private function fit(field:TextField):void
        {
            var format:TextFormat = field.defaultTextFormat;
            if (labelSize == 0)
            {
                labelSize = int(format.size);
            }
            for (var size:int = labelSize; size >= MIN_LABEL_SIZE; size--)
            {
                format.size = size;
                field.defaultTextFormat = format;
                field.setTextFormat(format);
                if (field.textWidth + 4 <= field.width)
                {
                    return;
                }
            }
        }

        // The board: a dark surface lit from above with rounded corners and a thin edge, the title's band
        // parted from the body by a hairline. The surface takes the window's presses; the backdrop's close it.
        private function draw(w:Number, h:Number):void
        {
            var g:Graphics = frame.graphics;
            g.clear();
            var light:Matrix = new Matrix();
            light.createGradientBox(w, h, Math.PI / 2);
            g.lineStyle(1, EDGE, 1, true);
            g.beginGradientFill(GradientType.LINEAR, [SURFACE_TOP, SURFACE_BOTTOM], [1, 1], [0, 255], light);
            g.drawRoundRect(0.5, 0.5, w - 1, h - 1, 24, 24);
            g.endFill();
            g.lineStyle();
            g.beginFill(0xFFFFFF, 0.06);
            g.drawRect(PAD - TINT_X, HEADER, w - 2 * (PAD - TINT_X), 1);
            g.endFill();
            panel.graphics.clear();
            panel.graphics.beginFill(0x000000, 0.01);
            panel.graphics.drawRect(0, 0, w, h);
            panel.graphics.endFill();
        }

        // A text in the Settings' own font (its title's), on one line.
        private function text(size:Number, colour:uint, spacing:Number):TextField
        {
            var stock:TextField = content.parent != null ? Object(content.parent).textFieldLabel as TextField : null;
            var format:TextFormat = stock != null ? stock.getTextFormat() : new TextFormat();
            format.size = size;
            format.color = colour;
            format.letterSpacing = spacing;
            var field:TextField = new TextField();
            field.embedFonts = stock != null && stock.embedFonts;
            field.defaultTextFormat = format;
            field.autoSize = TextFieldAutoSize.LEFT;
            field.selectable = false;
            field.mouseEnabled = false;
            return field;
        }
    }
}
