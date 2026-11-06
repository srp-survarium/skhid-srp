package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.geom.Rectangle;
    import scaleform.clik.controls.DropdownMenu;
    import scaleform.clik.core.UIComponent;
    import scaleform.clik.data.DataProvider;
    import scaleform.clik.events.ButtonEvent;
    import scaleform.clik.events.ListEvent;
    import scaleform.clik.managers.PopUpManager;
    import scaleform.clik.utils.Padding;

    // The Graphics tab's "Vulkan options..." (docs/notes/dxvk-option.md): a button across the column's
    // row under the Renderer list, the tab's own (the Optimal and Default buttons' SetingsButton), whose
    // press opens a panel right under it, its edges the button's, over the column (a popup, as a list's),
    // holding the five options exe patch 83 keeps in user.cfg and gives DXVK at the next start with
    // Vulkan/DXVK: the frame rate limit and the performance overlay (lists, the exe's index), Reduce
    // input lag, Tear-free with vsync off and Prefer the dedicated GPU (checkboxes, "1" or "0"). A press
    // outside the panel (and outside a list of its own while that is open) or Esc closes it; the button
    // again too. SrpRendererChoice owns it: the button is disabled while the Renderer list shows
    // DirectX 11, and its options are the Renderer's pending
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
        // Left of the rows (a list's box ends at the panel's right edge, its field as wide as it can).
        private static const INSET:Number = 4;
        // The last row's (a checkbox's) drawn height: its box.
        private static const LAST_ROW:Number = 26;

        // The button that opens the panel.
        public var button:SetingsButton;
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
            button = new SetingsButton();
            button.name = "_srp_vulkan";
            button.autoSize = "none";
            button.focusable = true;
            button.toggle = false;
            // Scaleform leaves a script-created control fully transparent.
            button.alpha = 1;
            button.visible = false;
            button.addEventListener(ButtonEvent.CLICK, onPress);
            content.addChild(button);
            panel = new Sprite();
            panel.name = "_srp_vulkan_panel";
            background = new Shape();
            panel.addChild(background);
            // the controls one Sprite under the panel (press routing: memory note, the lobby's rooms)
            body = new Sprite();
            panel.addChild(body);
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

        // The button's text (the exe's root.srp_hud_texts).
        public function texts(label:String):void
        {
            button.label = label;
        }

        // Whether the button can be shown: its text and every option's answer there.
        public function get ready():Boolean
        {
            if (button.label == null || button.label == "")
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
            panel.stage.removeEventListener(MouseEvent.MOUSE_DOWN, onStagePress);
            panel.parent.removeChild(panel);
            return true;
        }

        // A press of the button: the panel opened under it (a popup over the column), or closed.
        private function onPress(event:Event):void
        {
            if (close() || !button.enabled)
            {
                return;
            }
            PopUpManager.show(panel, button.x, button.y + button.height, button.parent);
            panel.stage.addEventListener(MouseEvent.MOUSE_DOWN, onStagePress);
        }

        // A press outside the button, the panel and an open list of the panel closes it.
        private function onStagePress(event:MouseEvent):void
        {
            for (var at:DisplayObject = event.target as DisplayObject; at != null; at = at.parent)
            {
                if (at == button || at == panel
                    || at is UIComponent && UIComponent(at).focusTarget != null && panel.contains(UIComponent(at).focusTarget))
                {
                    return;
                }
            }
            close();
        }

        // Across the Renderer list's row, `y` under it; the panel the button's width. Its five rows on one
        // step, the lists' (`step`, Monitor's), the padding the same over the first and under the last.
        // `rows`: the column's rows under it, which the open panel lies over: it ends in a gap between
        // two of them, none showing half below its edge; the step grows as much as that takes.
        public function layout(renderer:DisplayObject, y:Number, step:Number, rows:Array):void
        {
            button.x = renderer.x;
            button.y = y + (renderer.height - button.height) / 2;
            button.width = renderer.width;
            var gaps:int = NAMES.length - 1;
            var height:Number = 2 * PADDING + gaps * step + LAST_ROW;
            // in the tab's coordinates: where the panel opens (right under the button) and ends
            var top:Number = button.y + button.height;
            var bottom:Number = top + height;
            for each (var under:DisplayObject in rows)
            {
                var span:Rectangle = under != null ? under.getBounds(button.parent) : null;
                if (span != null && span.top < bottom && span.bottom > bottom)
                {
                    bottom = span.bottom + 2;
                }
            }
            step += (bottom - top - height) / gaps;
            height = bottom - top;
            var width:Number = button.width;
            for (var i:int = 0; i < NAMES.length; i++)
            {
                var control:DisplayObject = controls[NAMES[i]];
                control.x = INSET;
                control.y = PADDING + i * step;
                if (i < LISTS)
                {
                    var list:DropdownMenuSkinnedLabel = control as DropdownMenuSkinnedLabel;
                    list.width = width - INSET;
                    // its list under its box, which keeps to the right as the Renderer's
                    list.menuOffset = new Padding(4, 0, 0, SrpRendererChoice.MENU_LEFT - (renderer.width - list.width));
                }
            }
            background.graphics.clear();
            // the lists' popup look (OptionScrollingList): black, a thin border; opaque, so a press on
            // it is the panel's, never a row's under it
            background.graphics.lineStyle(1, 0x3A3A3A);
            background.graphics.beginFill(0x000000, 1);
            background.graphics.drawRect(0, 0, width, height);
            background.graphics.endFill();
        }

        // Shown on the Graphics frame once ready; disabled (no panel) while `enabled` is false.
        public function show(visible:Boolean, enabled:Boolean):void
        {
            button.visible = visible && ready;
            button.enabled = enabled;
            if (!button.visible || !enabled)
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

