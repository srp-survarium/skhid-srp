package
{
    import flash.display.BitmapData;
    import flash.display.DisplayObject;
    import flash.display.Graphics;
    import flash.display.MovieClip;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.geom.Matrix;
    import flash.geom.Rectangle;
    import scaleform.clik.controls.DropdownMenu;
    import scaleform.clik.data.DataProvider;
    import scaleform.clik.events.ListEvent;
    import scaleform.clik.utils.Padding;

    // The Graphics tab's "Vulkan options" (docs/notes/dxvk-option.md): a row of the column under the Renderer
    // list, built as it is (its label, a stock drop-down box showing "Default" while the five are at their
    // defaults, else "Custom"), whose box opens a panel under it where the Renderer's opens its list (the
    // box in its open state meanwhile), over the column, its rows on the column's grid, holding the five
    // options exe patch 83 keeps in user.cfg and gives DXVK at the next start with Vulkan/DXVK: the frame
    // rate limit and the performance overlay (lists, the exe's index), Reduce input lag, Tear-free with
    // vsync off and Prefer the dedicated GPU (checkboxes, "1" or "0"). The panel is the box's drop-down
    // (DropdownMenu's ready popup): a press outside it (and outside a list of its own while that is open),
    // on the box again, or Esc closes it. SrpRendererChoice owns it: the box is disabled while the Renderer
    // list shows DirectX 11, and its options are the Renderer's pending change of the tab (Apply saves
    // them, Cancel puts the saved ones back). The exe answers ExternalInterface.call(name[, value]) with
    // root.srp_check_option(name, label, saved, running[, the list's choices' texts]), running the value
    // this process runs (DXVK's of the start, without DXVK the saved one).
    public class SrpVulkanOptions
    {
        public static const FRAME_LIMIT:String = "srp_dxvk_frame_limit";
        public static const OVERLAY:String = "srp_dxvk_overlay";
        public static const LOW_LATENCY:String = "srp_dxvk_low_latency";
        public static const TEAR_FREE:String = "srp_dxvk_tear_free";
        public static const DEDICATED_GPU:String = "srp_dxvk_dedicated_gpu";
        // In the panel's order: the two lists, then the three checkboxes.
        public static const NAMES:Array = [FRAME_LIMIT, OVERLAY, LOW_LATENCY, TEAR_FREE, DEDICATED_GPU];
        // Each option's default (exe patch 83's): the lists' first choice, Prefer the dedicated GPU on.
        private static const DEFAULTS:Array = ["0", "0", "0", "0", "1"];
        private static const LISTS:int = 2;
        // Over the panel's first row and under its last; out of the column on its left and right.
        private static const PADDING:Number = 8;
        private static const MARGIN:Number = 6;
        // The game's tooltip (SrvProfileTipRenderer, the lobby's bottom buttons'): each option's hint on hover.
        private static const TIP:String = "SrvProfileTipRenderer";
        // The last row's (a checkbox's) drawn height: its box.
        private static const LAST_ROW:Number = 26;
        // The window's border, outside in: its light line, then its dark band.
        private static const EDGE_LIGHT:uint = 0x585A5D;
        private static const EDGE_DARK:Array = [0x1A1A18, 0x131311, 0x0B0B0B];

        // The row: its label and the box that opens the panel.
        public var box:DropdownMenuSkinnedLabel;
        // The panel (the box's drop-down) and the controls in it, by name.
        public var panel:MovieClip;
        private var body:Sprite;
        // The panel's surface: the Settings window's border on its four sides, inside it the window's own
        // background art where it lies under the panel (draw), so only the border tells inside from outside.
        private var frame:Sprite;
        private var content:MovieClip;
        private var controls:Object = {};
        // The box's two values (all five at their defaults, or not); "" until the exe answers.
        private var standard:String = "";
        private var custom:String = "";
        // Each option's saved and running value ("1"/"0" or the list's index); "" until the exe answers.
        private var saved:Object = {};
        private var running:Object = {};
        // Settings' handler of a change (listen).
        private var changed:Function = null;
        // The panel's size (layout).
        private var panelWidth:Number = 0;
        private var panelHeight:Number = 0;

        public function SrpVulkanOptions(content:MovieClip)
        {
            this.content = content;
            panel = new MovieClip();
            panel.name = "_srp_vulkan_panel";
            frame = new Sprite();
            panel.addChild(frame);
            // the controls one Sprite under the panel (press routing: memory note, the lobby's rooms)
            body = new Sprite();
            panel.addChild(body);
            box = new DropdownMenuSkinnedLabel();
            box.name = "_srp_vulkan";
            SrpRendererChoice.style(box);
            box.dropdown = panel;
            // a press on it keeps the focus where it is: a press in the panel is no change of focus that closes it
            box.focusable = false;
            // Scaleform leaves a script-created control fully transparent.
            box.alpha = 1;
            box.visible = false;
            content.addChild(box);
            for (var i:int = 0; i < NAMES.length; i++)
            {
                var name:String = NAMES[i];
                var control:DisplayObject;
                if (i < LISTS)
                {
                    var list:DropdownMenuSkinnedLabel = new DropdownMenuSkinnedLabel();
                    SrpRendererChoice.style(list);
                    list.addEventListener(ListEvent.INDEX_CHANGE, onChange);
                    control = list;
                }
                else
                {
                    var check:CheckBoxSkinned = new CheckBoxSkinned();
                    check.autoSize = "none";
                    check.enableInitCallback = false;
                    check.focusable = true;
                    check.selected = false;
                    check.addEventListener(Event.SELECT, onChange);
                    control = check;
                }
                control.name = "_" + name;
                // Scaleform leaves a script-created control fully transparent.
                control.alpha = 1;
                body.addChild(control);
                controls[name] = control;
                saved[name] = running[name] = "";
            }
            panel.addEventListener(Event.ADDED_TO_STAGE, onOpen);
            panel.addEventListener(Event.REMOVED_FROM_STAGE, onClose);
        }

        // The exe's texts (root.srp_hud_texts): the row's label, each option's hint in NAMES' order (the game's
        // tooltip on hover: CLIK's ToolTip, every control's base), the box's two values.
        public function texts(label:String, hints:Array, standard:String, custom:String):void
        {
            box.label = label;
            this.standard = standard;
            this.custom = custom;
            for (var i:int = 0; i < NAMES.length && i < hints.length; i++)
            {
                var control:Object = controls[NAMES[i]];
                control.tooltipRendererName = TIP;
                control.tipData = {name: hints[i]};
            }
            value();
        }

        // Whether the row can be shown: its texts and every option's answer there.
        public function get ready():Boolean
        {
            if (box.textFieldLabel.text == "" || standard == "")
            {
                return false;
            }
            for each (var name:String in NAMES)
            {
                if (saved[name] == "")
                {
                    return false;
                }
            }
            return true;
        }

        public static function named(name:String):Boolean
        {
            return NAMES.indexOf(name) != -1;
        }

        // The exe's answer: the name, the label, the saved and running values, a list's choices.
        public function answer(values:Array):void
        {
            var name:String = values[0];
            var control:Object = controls[name];
            var list:DropdownMenu = control as DropdownMenu;
            if (control == null || values.length < 4 || (list != null && values.length < 5))
            {
                return;
            }
            if (list != null)
            {
                list.dataProvider = new DataProvider(values.slice(4));
                list.menuRowCount = values.length - 4;
                list.label = values[1];
            }
            else
            {
                control.label = values[1];
            }
            saved[name] = values[2];
            running[name] = values[3];
            put(name, saved[name]);
        }

        // On the Graphics frame: every option asked for.
        public function ask():void
        {
            for each (var name:String in NAMES)
            {
                SrpExe.call(name, null);
            }
        }

        // An option's control is not its saved value: a change for Apply.
        public function get pending():Boolean
        {
            for each (var name:String in NAMES)
            {
                if (saved[name] != "" && state(name) != saved[name])
                {
                    return true;
                }
            }
            return false;
        }

        // A saved value is not the one this process runs: the next start changes it.
        public function get restart():Boolean
        {
            for each (var name:String in NAMES)
            {
                if (saved[name] != running[name])
                {
                    return true;
                }
            }
            return false;
        }

        // Apply: each changed option saved (the exe writes user.cfg and answers).
        public function save():void
        {
            for each (var name:String in NAMES)
            {
                if (saved[name] != "" && state(name) != saved[name])
                {
                    saved[name] = state(name);
                    SrpExe.call(name, saved[name]);
                }
            }
        }

        // Cancel: the saved values.
        public function revert():void
        {
            for each (var name:String in NAMES)
            {
                if (saved[name] != "")
                {
                    put(name, saved[name]);
                }
            }
        }

        // Each control's change, for Settings (pending).
        public function listen(handler:Function):void
        {
            changed = handler;
        }

        // Whether the panel is open.
        public function get open():Boolean
        {
            return panel.stage != null;
        }

        // The panel closed (a tab change, the window closed, Esc); whether it was open.
        public function close():Boolean
        {
            if (!open)
            {
                return false;
            }
            box.close();
            return true;
        }

        // The row one step under the Renderer list (`renderer`), built as it is: `y` its row, the box at the
        // Renderer's place and width. The panel where the Renderer opens its list (`menuOffset`: right under the
        // box), out by MARGIN on both sides of the column, its rows on the column's grid: each list at the
        // Renderer list's place and width (its label at the labels' x, its box at the boxes'), each checkbox at
        // the column's (`rows[1]`, Fullscreen's) x. Its five rows on one step, the lists' (`step`, Monitor's),
        // the padding the same over the first and under the last. `rows`: the column's rows under it, which the
        // open panel lies over: it ends in a gap between two of them, none showing half below its edge; the
        // step grows as much as that takes.
        public function layout(renderer:DropdownMenuSkinnedLabel, y:Number, step:Number, rows:Array):void
        {
            box.x = renderer.x;
            box.y = y;
            box.width = renderer.width;
            var offset:Padding = renderer.menuOffset;
            box.menuOffset = new Padding(offset.top, 0, 0, -MARGIN);
            var panelX:Number = box.x - MARGIN;
            var gaps:int = NAMES.length - 1;
            var height:Number = 2 * PADDING + gaps * step + LAST_ROW;
            // in the tab's coordinates: where the panel opens and ends
            var top:Number = box.y + box.height + offset.top;
            var bottom:Number = top + height;
            for each (var under:DisplayObject in rows)
            {
                var span:Rectangle = under != null ? under.getBounds(box.parent) : null;
                if (span != null && span.top < bottom && span.bottom > bottom)
                {
                    bottom = span.bottom + 2;
                }
            }
            step += (bottom - top - height) / gaps;
            panelHeight = bottom - top;
            panelWidth = renderer.width + 2 * MARGIN;
            for (var i:int = 0; i < NAMES.length; i++)
            {
                var control:DisplayObject = controls[NAMES[i]];
                control.x = (i < LISTS ? renderer.x : rows[1].x) - panelX;
                control.y = PADDING + i * step;
                if (i < LISTS)
                {
                    control.width = renderer.width;
                }
            }
            if (open)
            {
                draw();
            }
        }

        // Shown on the Graphics frame once ready; disabled (greyed as a stock list, no panel) while `enabled` is
        // false.
        public function show(visible:Boolean, enabled:Boolean):void
        {
            box.visible = visible && ready;
            box.enabled = enabled;
            if (!box.visible || !enabled)
            {
                close();
            }
        }

        // The panel's surface at its size, from where it lies on the window: opaque, so the rows under it never
        // show and a press on it is the panel's. The window's background art is its first child (SrvSettingsWndow's
        // shape, which fills itself with that bitmap over its bounds).
        private function draw():void
        {
            var g:Graphics = frame.graphics;
            g.clear();
            g.beginFill(EDGE_LIGHT);
            g.drawRect(0, 0, panelWidth, panelHeight);
            for (var i:int = 0; i < EDGE_DARK.length; i++)
            {
                g.beginFill(EDGE_DARK[i]);
                g.drawRect(1 + i, 1 + i, panelWidth - 2 * (1 + i), panelHeight - 2 * (1 + i));
            }
            var inset:Number = 1 + EDGE_DARK.length;
            var window:DisplayObject = content.parent != null ? content.parent.getChildAt(0) : null;
            if (window != null && frame.stage != null)
            {
                var art:BitmapData = new SrpSettingsGround();
                var spread:Rectangle = window.getBounds(window);
                // the art in the window's shape, then on the stage, then in the panel
                var place:Matrix = new Matrix(spread.width / art.width, 0, 0, spread.height / art.height, spread.x, spread.y);
                place.concat(window.transform.concatenatedMatrix);
                var back:Matrix = frame.transform.concatenatedMatrix;
                back.invert();
                place.concat(back);
                g.beginBitmapFill(art, place, false, true);
            }
            else
            {
                g.beginFill(EDGE_DARK[EDGE_DARK.length - 1]);
            }
            g.drawRect(inset, inset, panelWidth - 2 * inset, panelHeight - 2 * inset);
            g.endFill();
        }

        // The panel shown (the box opened it): its surface where it now lies.
        private function onOpen(event:Event):void
        {
            draw();
        }

        // The panel gone (the box closed it): a list of its own open closes with it.
        private function onClose(event:Event):void
        {
            for (var i:int = 0; i < LISTS; i++)
            {
                var list:DropdownMenu = controls[NAMES[i]] as DropdownMenu;
                if (list.selected)
                {
                    list.close();
                }
            }
        }

        // The box's value: "Default" while every option is at its default, else "Custom".
        private function value():void
        {
            if (standard == "")
            {
                return;
            }
            var all:Boolean = true;
            for (var i:int = 0; i < NAMES.length; i++)
            {
                all = all && state(NAMES[i]) == DEFAULTS[i];
            }
            box.dataProvider = new DataProvider([all ? standard : custom]);
            box.selectedIndex = 0;
            box.invalidateSelectedIndex();
        }

        private function state(name:String):String
        {
            var control:Object = controls[name];
            return control is DropdownMenu ? String(control.selectedIndex) : control.selected ? "1" : "0";
        }

        private function put(name:String, state:String):void
        {
            var control:Object = controls[name];
            if (control is DropdownMenu)
            {
                control.selectedIndex = int(state);
            }
            else
            {
                control.selected = state == "1";
            }
            value();
        }

        private function onChange(event:Event):void
        {
            value();
            if (changed != null)
            {
                changed(event);
            }
        }
    }
}
