package
{
    import flash.display.BitmapData;
    import flash.display.DisplayObject;
    import flash.display.Graphics;
    import flash.display.MovieClip;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.display.Stage;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.filters.DropShadowFilter;
    import flash.geom.Matrix;
    import flash.geom.Point;
    import flash.geom.Rectangle;
    import scaleform.clik.controls.DropdownMenu;
    import scaleform.clik.data.DataProvider;
    import scaleform.clik.events.ButtonEvent;
    import scaleform.clik.events.ListEvent;
    import scaleform.clik.managers.PopUpManager;

    // The Graphics tab's "Vulkan options" (docs/notes/dxvk-option.md): a button on the row under the
    // Renderer list, the tab's own (the Optimal and Default buttons' SetingsButton at its own size), with a
    // drop-down's arrow at its right end (the Renderer box's own art), whose press opens a panel under it
    // (the button held pressed, its arrow turned up), over the column (a popup, as a list's), its rows on
    // the column's grid,
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
        // Over the panel's first row and under its last; out of the column on its left and right.
        private static const PADDING:Number = 8;
        private static const MARGIN:Number = 6;
        // The game's tooltip (SrvProfileTipRenderer, the lobby's bottom buttons'): each option's hint on hover.
        private static const TIP:String = "SrvProfileTipRenderer";
        // The last row's (a checkbox's) drawn height: its box.
        private static const LAST_ROW:Number = 26;

        // The button that opens the panel.
        public var button:SetingsButton;
        // The panel and the controls in it, by name.
        public var panel:Sprite;
        private var body:Sprite;
        // The panel's frame and surface (draw): the Settings window's own border, inside it the window's own
        // background art where it lies under the panel; its shadow on the tab under it.
        private var frame:Shape;
        private var shadow:Shape;
        // The window's border, outside in: its light line, then its dark band.
        private static const EDGE_LIGHT:uint = 0x585A5D;
        private static const EDGE_DARK:Array = [0x1A1A18, 0x131311, 0x0B0B0B];
        // The drop-down's arrow over the button's right end (arrowArt; beside it in the tab, not in it: the
        // button sizes itself by its children's bounds), turned up while the panel is open.
        public var arrow:Sprite;
        // The tab's preset buttons' width (Optimal's), the button's own; the arrow's width (its art's, cut) and
        // the room it takes at the button's right end.
        private var presetWidth:Number = 0;
        private var arrowWidth:Number = 0;
        private var arrowRoom:Number = 0;
        // The panel's place and size (layout), and the content it opens over.
        private var content:MovieClip;
        private var panelWidth:Number = 0;
        private var panelHeight:Number = 0;
        // The arrow's size: the Renderer box's arrow, half again as big (a button's arrow, read at a glance).
        private static const ARROW_SCALE:Number = 1.5;
        // The panel's tab round the button: out by this on its top and right (its left is the panel's).
        private static const TAB:Number = 4;
        private var controls:Object = {};
        // Each option's saved and running value ("1"/"0" or the list's index); "" until the exe answers.
        private var saved:Object = {};
        private var running:Object = {};
        // Settings' handler of a change (listen).
        private var changed:Function = null;

        public function SrpVulkanOptions(content:MovieClip)
        {
            this.content = content;
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
            arrow = arrowArt();
            arrow.visible = false;
            content.addChild(arrow);
            panel = new Sprite();
            panel.name = "_srp_vulkan_panel";
            // the shadow under everything, taking no press: its outline is filled (hidden by its knock-out) over
            // the button too, and a press there is the button's
            var shade:Sprite = new Sprite();
            shade.mouseEnabled = shade.mouseChildren = false;
            shadow = new Shape();
            shade.addChild(shadow);
            panel.addChild(shade);
            frame = new Shape();
            panel.addChild(frame);
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
        // The button's text and each option's hint (the exe's root.srp_hud_texts), in NAMES' order: the game's
        // tooltip on hover (CLIK's ToolTip, every control's base: its delay, kept on the stage).
        public function texts(label:String, hints:Array):void
        {
            button.label = label;
            size();
            for (var i:int = 0; i < NAMES.length && i < hints.length; i++)
            {
                var control:Object = controls[NAMES[i]];
                control.tooltipRendererName = TIP;
                control.tipData = {name: hints[i]};
            }
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

        // The panel closed (a tab change, the window closed, Esc); whether it was open. The button back as
        // the pointer has it.
        public function close():Boolean
        {
            if (!open)
            {
                return false;
            }
            // a list of its own open: its list is a popup of its own, closed with it
            for (var i:int = 0; i < LISTS; i++)
            {
                var list:DropdownMenu = controls[NAMES[i]] as DropdownMenu;
                if (list.selected)
                {
                    list.close();
                }
            }
            var stage:Stage = panel.stage;
            stage.removeEventListener(MouseEvent.MOUSE_DOWN, onStagePress);
            button.removeEventListener(Event.RENDER, hold);
            button.removeEventListener(Event.ENTER_FRAME, hold);
            panel.parent.removeChild(panel);
            // the button as the pointer leaves it: its state's frame now (a state the button set while it was held
            // is drawn first), then over or up
            button.validateNow();
            button.gotoAndPlay(button.hitTestPoint(stage.mouseX, stage.mouseY, true) ? "over" : "up");
            arrow.scaleY = Math.abs(arrow.scaleY);
            return true;
        }

        // A press of the button (its own click, its pointer always on): the panel opened right under it (a popup
        // over the column), or closed when it is open. While it is open the button is held on its "down" frames
        // (hold).
        private function onPress(event:Event):void
        {
            if (close() || !button.enabled)
            {
                return;
            }
            PopUpManager.show(panel, panelX, button.y + button.height, button.parent);
            // as the stock lists find a press outside them (DropdownMenu.handleStageClick): the stage's
            // MOUSE_DOWN
            panel.stage.addEventListener(MouseEvent.MOUSE_DOWN, onStagePress);
            // after the button's own state changes (its roll-over and roll-out, which it draws on the stage's
            // render), the held frames again
            button.addEventListener(Event.RENDER, hold, false, -1);
            button.addEventListener(Event.ENTER_FRAME, hold);
            hold(null);
            arrow.scaleY = -Math.abs(arrow.scaleY);
            draw();
        }

        // The button held pressed while the panel is open: its "down" frames, whatever state it drew.
        private function hold(event:Event):void
        {
            if (button.currentLabel != "down")
            {
                button.validateNow();
                button.gotoAndPlay("down");
            }
        }

        // A press on the stage: outside the panel and the button, it closes the panel. Where the press lands
        // counts, not only what Scaleform makes its target (it may give a container): the panel's frame (its
        // surface and border) or the button under the point, or a target in either, is no outside press (the
        // button's own click toggles it). While a list of the panel is open, a press is the list's (its own
        // handler closes it).
        private function onStagePress(event:MouseEvent):void
        {
            var target:DisplayObject = event.target as DisplayObject;
            if (frame.hitTestPoint(event.stageX, event.stageY, true) || button.hitTestPoint(event.stageX, event.stageY, true)
                || target != null && (panel.contains(target) || button.contains(target)))
            {
                return;
            }
            for (var i:int = 0; i < LISTS; i++)
            {
                if (DropdownMenu(controls[NAMES[i]]).selected)
                {
                    return;
                }
            }
            close();
        }

        // The panel's left edge in the tab's coordinates.
        private var panelX:Number = 0;
        // The button at the preset buttons' width, wider only for a text that would not fit it beside its arrow;
        // its text in the room left of the arrow (its right margin grown by the arrow's room), the arrow at its
        // right end.
        private function size():void
        {
            if (presetWidth <= 0)
            {
                return;
            }
            var text:Object = button.constraints != null ? button.constraints.getElement("textField") : null;
            if (text != null && arrowRoom == 0)
            {
                arrowRoom = arrowWidth * ARROW_SCALE + 8;
                text.right += arrowRoom;
            }
            button.width = presetWidth;
            button.validateNow();
            var room:Number = button.textField != null ? button.textField.width * button.scaleX : presetWidth;
            var needed:Number = button.textField != null ? button.textField.textWidth + 8 : 0;
            if (needed > room)
            {
                button.width = presetWidth + needed - room;
                button.validateNow();
            }
            placeArrow();
        }

        // The arrow over the button's right end, at its own size, turned as it is.
        private function placeArrow():void
        {
            var flip:Number = arrow.scaleY < 0 ? -1 : 1;
            arrow.scaleX = ARROW_SCALE;
            arrow.scaleY = flip * ARROW_SCALE;
            arrow.x = button.x + button.width - arrowRoom / 2 - 2;
            arrow.y = button.y + button.height / 2;
        }

        // The drop-down's arrow: the Renderer box's own art (DropdownMenuSkinnedLabel's shape: its box, the arrow's
        // dark segment at its right end, the arrow in it), cut to the arrow by a mask of its triangle, centred on
        // the sprite's origin, so it turns up about its middle.
        private function arrowArt():Sprite
        {
            var source:MovieClip = new DropdownMenuSkinnedLabel();
            var art:DisplayObject = source.getChildAt(0);
            source.removeChild(art);
            var box:Rectangle = art.getBounds(art);
            // the arrow: in the middle of the box's last fifth (its segment), a little under the box's middle
            var tip:Rectangle = new Rectangle(box.x + box.width * 0.8724, box.y + box.height * 0.49,
                box.width * 0.0582, box.height * 0.12);
            var cut:Shape = new Shape();
            cut.graphics.beginFill(0);
            cut.graphics.moveTo(tip.left - 0.6, tip.top - 0.4);
            cut.graphics.lineTo(tip.right + 0.6, tip.top - 0.4);
            cut.graphics.lineTo(tip.x + tip.width / 2, tip.bottom + 0.6);
            cut.graphics.lineTo(tip.left - 0.6, tip.top - 0.4);
            cut.graphics.endFill();
            var middle:Point = new Point(tip.x + tip.width / 2, tip.y + tip.height / 2);
            arrowWidth = tip.width + 1.2;
            art.x = cut.x = -middle.x;
            art.y = cut.y = -middle.y;
            var holder:Sprite = new Sprite();
            holder.name = "_srp_vulkan_arrow";
            holder.addChild(art);
            holder.addChild(cut);
            art.mask = cut;
            holder.mouseEnabled = holder.mouseChildren = false;
            return holder;
        }

        // The button at the column's left edge (the Renderer list's), `y` its row, at the tab's preset buttons'
        // width (`preset`: Optimal); the panel right under it, out by MARGIN on both sides of the column, its
        // rows on the column's grid: each list at the Renderer list's place and width (its label at the
        // labels' x, its box at the boxes'), each checkbox at the column's (`rows[1]`, Fullscreen's) x. Its five
        // rows on one step, the lists' (`step`, Monitor's), the padding the same over the first and under the
        // last. `rows`: the column's rows under it, which the open panel lies over: it ends in a gap between
        // two of them, none showing half below its edge; the step grows as much as that takes.
        public function layout(renderer:DisplayObject, y:Number, step:Number, rows:Array, preset:DisplayObject):void
        {
            presetWidth = preset.width;
            size();
            button.x = renderer.x;
            button.y = y + (renderer.height - button.height) / 2;
            placeArrow();
            panelX = renderer.x - MARGIN;
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
            var width:Number = renderer.width + 2 * MARGIN;
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
            panelWidth = width;
            panelHeight = height;
            if (open)
            {
                draw();
            }
        }

        // The panel's outline inset by `inset`: its rectangle and the button's above it, out by TAB round it,
        // one piece (a tab that drops down: no edge between them).
        private function outline(g:Graphics, inset:Number):void
        {
            var w:Number = panelWidth, h:Number = panelHeight, i:Number = inset;
            var right:Number = button.x - panelX + button.width + TAB;
            var top:Number = -button.height - TAB;
            g.moveTo(i, top + i);
            g.lineTo(right - i, top + i);
            g.lineTo(right - i, i);
            g.lineTo(w - i, i);
            g.lineTo(w - i, h - i);
            g.lineTo(i, h - i);
            g.lineTo(i, top + i);
        }

        // The panel's frame, surface and shadow at its size, from where it lies on the window. The frame: the
        // Settings window's border round the panel and its button (the tab), the band under the button left out,
        // so the button opens into the panel. The surface: the window's own background art (the window's first
        // child, SrvSettingsWndow's shape, fills itself with that bitmap over its bounds) where it lies, opaque,
        // so the rows under it never show and a press on it is the panel's; the tab's inside is left open, the
        // button showing through it. The shadow: the outline's, outside it only (knocked out), on the tab under.
        private function draw():void
        {
            var g:Graphics = frame.graphics;
            g.clear();
            var bands:Array = [EDGE_LIGHT].concat(EDGE_DARK);
            for (var i:int = 0; i < bands.length; i++)
            {
                g.beginFill(bands[i]);
                outline(g, i);
                outline(g, i + 1);
                g.endFill();
            }
            var inset:Number = bands.length;
            var window:DisplayObject = content.parent != null ? content.parent.getChildAt(0) : null;
            var art:BitmapData = null;
            var place:Matrix = null;
            if (window != null && frame.stage != null)
            {
                art = new SrpSettingsGround();
                var spread:Rectangle = window.getBounds(window);
                // the art in the window's shape, then on the stage, then in the panel
                place = new Matrix(spread.width / art.width, 0, 0, spread.height / art.height, spread.x, spread.y);
                place.concat(window.transform.concatenatedMatrix);
                var back:Matrix = frame.transform.concatenatedMatrix;
                back.invert();
                place.concat(back);
            }
            surface(g, art, place);
            g.drawRect(inset, inset, panelWidth - 2 * inset, panelHeight - 2 * inset);
            g.endFill();
            // the bands under the button
            surface(g, art, place);
            g.drawRect(inset, 0, button.x - panelX + button.width + TAB - 2 * inset, inset);
            g.endFill();
            var sg:Graphics = shadow.graphics;
            sg.clear();
            sg.beginFill(0x000000);
            outline(sg, 0);
            sg.endFill();
            shadow.filters = [new DropShadowFilter(6, 90, 0x000000, 0.9, 16, 16, 1, 2, false, true)];
        }

        // The panel's surface fill: the window's art, or (off the stage) the border's darkest.
        private static function surface(g:Graphics, art:BitmapData, place:Matrix):void
        {
            if (art != null)
            {
                g.beginBitmapFill(art, place, false, true);
            }
            else
            {
                g.beginFill(EDGE_DARK[EDGE_DARK.length - 1]);
            }
        }

        // Shown on the Graphics frame once ready; disabled (no panel) while `enabled` is false.
        public function show(visible:Boolean, enabled:Boolean):void
        {
            button.visible = visible && ready;
            button.enabled = enabled;
            // greyed with the button (its text's dimming)
            arrow.alpha = enabled ? 1 : 0.45;
            arrow.visible = button.visible;
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

