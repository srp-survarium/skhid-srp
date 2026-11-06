package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.geom.Rectangle;
    import scaleform.clik.controls.DropdownMenu;
    import scaleform.clik.data.DataProvider;
    import scaleform.clik.events.ListEvent;
    import scaleform.clik.utils.Padding;

    // The Graphics tab's "Vulkan options" (docs/notes/dxvk-option.md): a row under the Renderer list,
    // built as it is (its label on its left, a box), whose box opens a panel over the column the way a
    // list's popup opens, holding the five options exe patch 83 keeps in user.cfg and gives DXVK at the
    // next start with Vulkan/DXVK: the frame rate limit and the performance overlay (lists, the exe's
    // index), Reduce input lag, Tear-free with vsync off and Prefer the dedicated GPU (checkboxes, "1"
    // or "0"). A click outside the panel or Esc closes it. SrpRendererChoice owns it: the row is
    // greyed while the Renderer list shows DirectX 11, and its options are the Renderer's pending
    // change of the tab (Apply saves them, Cancel puts the saved ones back). The exe answers
    // ExternalInterface.call(name[, value]) with root.srp_check_option(name, label, saved, running[,
    // the list's choices' texts]), running the value this process runs (DXVK's of the start, without
    // DXVK the saved one).
    public class SrpVulkanOptions
    {
        public static const FRAME_LIMIT:String = "srp_dxvk_frame_limit";
        public static const OVERLAY:String = "srp_dxvk_overlay";
        public static const LOW_LATENCY:String = "srp_dxvk_low_latency";
        public static const TEAR_FREE:String = "srp_dxvk_tear_free";
        public static const DEDICATED_GPU:String = "srp_dxvk_dedicated_gpu";
        // In the panel's order: the two lists, then the three checkboxes.
        public static const NAMES:Array = [FRAME_LIMIT, OVERLAY, LOW_LATENCY, TEAR_FREE, DEDICATED_GPU];
        private static const LISTS:int = 2;
        // Over the panel's first row and under its last.
        private static const PADDING:Number = 8;
        // The last row's (a checkbox's) drawn height: its box.
        private static const LAST_ROW:Number = 26;

        // The row: its label, and a box that opens the panel (DropdownMenu's `panel`).
        public var row:DropdownMenuSkinnedLabel;
        // The panel and the controls in it, by name.
        public var panel:Sprite;
        private var body:Sprite;
        private var background:Shape;
        private var controls:Object = {};
        // Each option's saved and running value ("1"/"0" or the list's index); "" until the exe answers.
        private var saved:Object = {};
        private var running:Object = {};
        // Settings' handler of a change (listen).
        private var changed:Function = null;

        public function SrpVulkanOptions(content:MovieClip)
        {
            row = new DropdownMenuSkinnedLabel();
            row.name = "_srp_vulkan";
            SrpRendererChoice.style(row);
            row.menuRowCount = 1;
            row.alpha = 1;
            row.visible = false;
            content.addChild(row);
            panel = new Sprite();
            panel.name = "_srp_vulkan_panel";
            background = new Shape();
            panel.addChild(background);
            // the controls one Sprite under the panel (press routing: memory note, the lobby's rooms)
            body = new Sprite();
            panel.addChild(body);
            row.panel = panel;
            row.menuOffset = new Padding(4, 0, 0, -PADDING);
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
                    var box:CheckBoxSkinned = new CheckBoxSkinned();
                    box.autoSize = "none";
                    box.enableInitCallback = false;
                    box.focusable = true;
                    box.selected = false;
                    box.addEventListener(Event.SELECT, onChange);
                    control = box;
                }
                control.name = "_" + name;
                // Scaleform leaves a script-created control fully transparent.
                control.alpha = 1;
                body.addChild(control);
                controls[name] = control;
                saved[name] = running[name] = "";
            }
        }

        // The row's label and its box's text (the exe's root.srp_hud_texts).
        public function texts(label:String, open:String):void
        {
            row.dataProvider = new DataProvider([open]);
            row.selectedIndex = 0;
            row.label = label;
        }

        // Whether the row can be shown: its texts and every option's answer there.
        public function get ready():Boolean
        {
            if (row.dataProvider == null || row.dataProvider.length != 1)
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
                if (saved[name] != "" && value(name) != saved[name])
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
                if (saved[name] != "" && value(name) != saved[name])
                {
                    saved[name] = value(name);
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

        // The panel closed (a tab change, the window closed, Esc); whether it was open.
        public function close():Boolean
        {
            var open:Boolean = row.selected;
            if (open)
            {
                row.close();
            }
            return open;
        }

        // At the Renderer list's place and size, `y` under it. The panel's five rows on one step, the
        // lists' (`step`, Monitor's), the padding the same over the first and under the last.
        // `rows`: the column's rows under it, which the open panel lies over: it ends in a gap between
        // two of them, none showing half below its edge; the step grows as much as that takes.
        public function layout(renderer:DisplayObject, y:Number, step:Number, rows:Array):void
        {
            row.x = renderer.x;
            row.y = y;
            row.width = renderer.width;
            var gaps:int = NAMES.length - 1;
            var height:Number = 2 * PADDING + gaps * step + LAST_ROW;
            // in the tab's coordinates: where the popup opens (under the box, its offset's top) and ends
            var top:Number = y + row.height + row.menuOffset.top;
            var bottom:Number = top + height;
            for each (var under:DisplayObject in rows)
            {
                var span:Rectangle = under != null ? under.getBounds(row.parent) : null;
                if (span != null && span.top < bottom && span.bottom > bottom)
                {
                    bottom = span.bottom + 2;
                }
            }
            step += (bottom - top - height) / gaps;
            height = bottom - top;
            for (var i:int = 0; i < NAMES.length; i++)
            {
                var control:DisplayObject = controls[NAMES[i]];
                control.x = PADDING;
                control.y = PADDING + i * step;
                if (i < LISTS)
                {
                    control.width = renderer.width;
                }
            }
            var width:Number = renderer.width + 2 * PADDING;
            background.graphics.clear();
            // the lists' popup look (OptionScrollingList): black, a thin border; opaque, so a press on
            // it is the panel's, never a row's under it
            background.graphics.lineStyle(1, 0x3A3A3A);
            background.graphics.beginFill(0x000000, 1);
            background.graphics.drawRect(0, 0, width, height);
            background.graphics.endFill();
        }

        // Shown on the Graphics frame once ready; greyed (no panel) while `enabled` is false.
        public function show(visible:Boolean, enabled:Boolean):void
        {
            row.visible = visible && ready;
            row.enabled = enabled;
            if (!row.visible || !enabled)
            {
                close();
            }
        }

        private function value(name:String):String
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
        }

        private function onChange(event:Event):void
        {
            if (changed != null)
            {
                changed(event);
            }
        }
    }
}

