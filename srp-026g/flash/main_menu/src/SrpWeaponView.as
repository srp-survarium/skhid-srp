package
{
    import flash.display.BitmapData;
    import flash.display.Graphics;
    import flash.display.MovieClip;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.geom.Matrix;
    import flash.geom.Point;
    import flash.text.TextField;
    import flash.text.TextFieldAutoSize;
    import flash.text.TextFormat;
    import flash.text.TextFormatAlign;
    import scaleform.clik.controls.Button;
    import scaleform.clik.events.ButtonEvent;
    import scaleform.clik.events.IndexEvent;

    // The Gameplay tab's "Weapon view" (exe patch 219, docs/notes/weapon-view-settings.md): how the
    // held gun is drawn, apart from the world's field of view (the "World FOV:" slider above it). The
    // tab's standard button (SetingsButton, the Optimal and Default buttons' symbol at their width) under
    // the World FOV slider opens a window in the Settings window's own skin, as the Controls tab's Key
    // bindings window (SrpKeysWindow: SrpSettingsGround's frame and title band, the title in its font,
    // its close button, the screen dimmed behind it), holding the tab's own controls in their look (the
    // stock slider with its label in the headings' look and its value, as World FOV; the HUD group's
    // checkbox; a note in the "Applies after restart." look):
    // - three sliders of the weapon's own field of view, a rifle's degrees (each gun keeps 0.69d0's
    //   own ratio to them): at the hip (40 to 100, 65), over iron sights and collimators (20 to 65,
    //   32.5) and through magnified scopes (16.25 to 32.5, 32.5);
    // - the checkbox "Weapon at the world FOV": the held gun drawn at the world's projection, as before
    //   revision 74; the three sliders are greyed while it is ticked;
    // - "Sight distance", -20 to +50 cm: aimed over iron sights and collimators, how much farther the
    //   eye stands from the sight than 0.69d0's place (revision 74's), negative nearer; its value in cm
    //   with its sign, its note ("+ farther, - closer") says which way.
    // Esc, its close button or a press beside it closes it; another tab, Apply or Cancel too. Its values
    // are the Gameplay tab's pending changes (Apply saves them, the exe applying each at once, in a
    // match too; Cancel puts the saved ones back). The exe keeps each value in user.cfg and answers
    // ExternalInterface.call(name[, value]) with root.srp_slider_option(name, label, saved, least, most,
    // steps a unit) for a slider, root.srp_check_option(name, label, saved) for the checkbox;
    // ExternalInterface.call("srp_weapon_view_texts") with root.srp_weapon_view_texts(title, the
    // distance's note). A script-added child of the Settings window.
    public class SrpWeaponView extends Sprite
    {
        public static const NAME:String = "_srp_weapon_view_window";
        public static const BUTTON:String = "_srp_weapon_view";
        public static const HIP:String = "srp_weapon_fov";
        public static const SIGHTS:String = "srp_weapon_fov_aimed";
        public static const SCOPES:String = "srp_weapon_fov_scope";
        public static const DISTANCE:String = "srp_sight_distance";
        public static const WORLD:String = "srp_weapon_fov_world";
        public static const TEXTS:String = "srp_weapon_view_texts";
        // In the window's order: the three angles, then (after the checkbox) the distance.
        public static const SLIDERS:Array = [HIP, SIGHTS, SCOPES, DISTANCE];
        private static const ANGLES:Array = [HIP, SIGHTS, SCOPES];
        private static const FRAME:int = SrpCheckOption.GAMEPLAY_FRAME;
        // The greyed sliders' opacity, as a disabled stock control's (SrpMouseVertical's).
        private static const GREYED:Number = 0.45;
        // Each slider's snap in its own units, and its value a double click puts back (0.69d0's).
        private static const SNAP:Object = {srp_weapon_fov: 1, srp_weapon_fov_aimed: 0.5, srp_weapon_fov_scope: 0.25,
            srp_sight_distance: 1};
        private static const DEFAULTS:Object = {srp_weapon_fov: 65, srp_weapon_fov_aimed: 32.5, srp_weapon_fov_scope: 32.5,
            srp_sight_distance: 0};
        // The room right of a value's text in its field (the game's Scaleform measures wider than Ruffle:
        // "-16 cm" was cut to "-16 cr" in a field Ruffle found wide enough).
        private static const VALUE_MARGIN:Number = 10;
        // The widest values each slider shows, measured once its texts are there.
        private static const SAMPLES:Object = {srp_weapon_fov: ["100", "40"], srp_weapon_fov_aimed: ["32.5", "65", "22.5"],
            srp_weapon_fov_scope: ["16.25", "32.5", "22.75"]};
        // The button: the Graphics tab's Optimal and Default buttons' width, wider only for its text.
        private static const BUTTON_WIDTH:Number = 127;
        // The Settings window's frame (SrpSettingsGround, 650 x 565), as SrpKeysWindow lays it: its border
        // and its title's band kept, its inside stretched; the title and the close button where the Settings
        // window has them.
        private static const FRAME_LEFT:Number = 6, FRAME_RIGHT:Number = 6, FRAME_TOP:Number = 39, FRAME_BOTTOM:Number = 6;
        // The art's bottom: the Settings window's button bar (its dark band, rows 543 to 558) is left out, the
        // inside running on to the frame's own edge, its last 6 rows (the owner: "the bottom dark band is
        // unnecessary").
        private static const ART_INSIDE_END:Number = 543;
        private static const TITLE_Y:Number = 6.75, CLOSE_RIGHT:Number = 34.5, CLOSE_SCALE:Number = 0.756;
        // Inside the frame's sides and over its bottom; the controls' column's width; to the screen's edges.
        private static const SIDE:Number = 22, SPACE:Number = 12, INSIDE:Number = 300, MARGIN:Number = 12;
        // Between the preview's column (on the left, SrpWeaponPreview.WIDTH) and the controls' column.
        private static const COLUMN_GAP:Number = 24;
        // The rows: a slider's next row under its track, the checkbox's row (the HUD group's step).
        private static const ROW_GAP:Number = 12;
        // The preview's tabs: the Settings tab bar's height (its buttons side by side, no room between).
        private static const TAB_HEIGHT:Number = 40;
        // Between two buttons, and between their rows.
        private static const BUTTON_GAP:Number = 8;
        private static const BOX_ROW:Number = 35;

        // The tab's button that opens the window.
        public var button:SetingsButton;
        public var sliders:Object = {};
        public var box:CheckBoxSkinned;
        public var note:TextField;
        // The preview (SrpWeaponPreview) and its three views' tabs (the Settings window's tabs).
        public var preview:SrpWeaponPreview;
        public var tabs:Array = [];
        // The player's World FOV (Settings gives the World FOV slider's value), for the preview's world.
        public var worldFov:Function = null;
        // Under the controls, on a line of its own, "Default settings" (the tab's standard button, the
        // game's own text, the Controls tab's st_options_default_controls): every value back to revision
        // 74's, pending until Apply. Under it Apply and Close, the Settings window's own buttons
        // (`applyFrom` and `closeFrom` give them: their symbols; Apply its label, Close the game's st_close):
        // Apply saves this window's pending values only (the other tabs' stay pending) and keeps it open
        // (`applied` tells Settings, which counts what is pending; disabled while nothing is); Close closes
        // it as its corner's cross does, the changes still pending.
        public var resetButton:SetingsButton;
        public var applyButton:Button;
        public var closeButton2:Button;
        public var applyFrom:Function = null;
        public var closeFrom:Function = null;
        public var applied:Function = null;
        private var defaultText:String = "Default settings";
        private var closeText:String = "Close";
        private var content:MovieClip;
        private var backdrop:Sprite;
        private var panel:Sprite;
        // The window's frame (its bounds are the window's; the preview's pictures past its pane are masked).
        public var frame:Shape;
        private var titleField:TextField;
        private var closeButton:ButtonCloseWindow2;
        private var title:String = "";
        // The dictionary's centimetres (st_srp_unit_cm: "%1$s cm", "%1$s см"), the number in place of %1$s.
        private var unit:String = "%1$s cm";
        // A slider's saved value in its units and its steps a unit, by name; absent until the exe answers.
        private var saved:Object = {};
        private var units:Object = {};
        // The checkbox's saved state, "1" or "0"; "" until the exe answers.
        private var savedWorld:String = "";
        // The look of the tab's headings (its "Friends:"), taken at the gameplay tab's layout.
        private var look:TextField = null;

        public function SrpWeaponView(content:MovieClip)
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
            button.width = BUTTON_WIDTH;
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
            for each (var key:String in SLIDERS)
            {
                var slider:SrvOptionSlider = new SrvOptionSlider();
                slider.name = "_" + key;
                slider.defaultValue = DEFAULTS[key];
                // Scaleform leaves a script-created control fully transparent.
                slider.alpha = 1;
                panel.addChild(slider);
                sliders[key] = slider;
            }
            sliders[DISTANCE].addEventListener(IndexEvent.INDEX_CHANGE, centimetres);
            box = new CheckBoxSkinned();
            box.name = "_" + WORLD;
            box.autoSize = "none";
            box.enableInitCallback = false;
            box.focusable = true;
            box.selected = false;
            box.alpha = 1;
            box.addEventListener(Event.SELECT, grey);
            panel.addChild(box);
            note = new TextField();
            note.name = NAME + "_note";
            note.selectable = note.mouseEnabled = false;
            note.wordWrap = true;
            note.multiline = true;
            note.autoSize = TextFieldAutoSize.LEFT;
            panel.addChild(note);
            resetButton = new SetingsButton();
            resetButton.name = NAME + "_default";
            resetButton.autoSize = "none";
            resetButton.focusable = true;
            resetButton.toggle = false;
            resetButton.addEventListener(ButtonEvent.CLICK, onDefault);
            panel.addChild(resetButton);
            preview = new SrpWeaponPreview();
            panel.addChild(preview);
            for (var view:int = 0; view < 4; view++)
            {
                // the Settings window's own tab (its tab bar's renderer: the lit label and the orange line under
                // the one selected)
                var tab:InventoryTabButton = new InventoryTabButton();
                tab.name = NAME + "_tab" + view;
                tab.autoSize = "none";
                tab.toggle = true;
                tab.focusable = true;
                tab.addEventListener(ButtonEvent.CLICK, onTab);
                panel.addChild(tab);
                tabs.push(tab);
            }
            // the slider being moved picks the view it shows; every move draws it again
            sliders[SIGHTS].addEventListener(IndexEvent.INDEX_CHANGE, function(e:Event):void { pick(-1); });
            sliders[SCOPES].addEventListener(IndexEvent.INDEX_CHANGE, function(e:Event):void { pick(SrpWeaponPreview.SCOPE); });
            sliders[DISTANCE].addEventListener(IndexEvent.INDEX_CHANGE, function(e:Event):void { pick(-1); });
            sliders[HIP].addEventListener(IndexEvent.INDEX_CHANGE, function(e:Event):void { pick(SrpWeaponPreview.HIP); });
            box.addEventListener(Event.SELECT, refresh);
        }

        // The view shown: `view`, or (-1) one of the sights' (the one shown, or the iron sights after the scope).
        private function pick(view:int):void
        {
            if (view < 0)
            {
                view = preview.shown == SrpWeaponPreview.SCOPE || preview.shown == SrpWeaponPreview.HIP
                    ? SrpWeaponPreview.IRON : preview.shown;
            }
            preview.shown = view;
            refresh();
        }

        private function onTab(event:ButtonEvent):void
        {
            pick(tabs.indexOf(event.currentTarget));
        }

        // The preview of the values the window shows (pending ones too), and its tabs. Drawn only while the
        // window is open (the exe's answers and the World FOV slider move its values while it is closed):
        // its pictures are loaded only then.
        public function refresh(event:Event = null):void
        {
            for (var view:int = 0; view < tabs.length; view++)
            {
                tabs[view].selected = view == preview.shown;
            }
            var uro:Number = worldFov != null ? Number(worldFov()) : 65;
            if (open)
            {
                preview.draw(sliders[HIP].selectedIndex, sliders[SIGHTS].selectedIndex, sliders[SCOPES].selectedIndex,
                    box.selected, sliders[DISTANCE].selectedIndex, uro > 0 ? uro : 65);
            }
            if (applyButton != null)
            {
                applyButton.enabled = changed();
            }
        }

        // Whether any of this window's values is pending.
        public function changed():Boolean
        {
            for each (var control:Object in controls)
            {
                if (pending(control))
                {
                    return true;
                }
            }
            return false;
        }

        private function onApply(event:ButtonEvent):void
        {
            send();
            if (applied != null)
            {
                applied();
            }
            refresh();
        }

        // Whether `name` is one of this group's sliders.
        public static function owns(name:String):Boolean
        {
            return SLIDERS.indexOf(name) >= 0;
        }

        // The controls Settings counts changes of.
        public function get controls():Array
        {
            var out:Array = [box];
            for each (var key:String in SLIDERS)
            {
                out.push(sliders[key]);
            }
            return out;
        }

        public function get open():Boolean
        {
            return visible;
        }

        // On every bind of a tab (create_content): closed, the button on top of the tab, the exe asked
        // on the gameplay frame.
        public function place():void
        {
            close();
            content.setChildIndex(button, content.numChildren - 1);
            shown();
            if (content.currentFrame == FRAME)
            {
                SrpExe.call(TEXTS, null);
                SrpExe.call(WORLD, null);
                for each (var key:String in SLIDERS)
                {
                    SrpExe.call(key, null);
                }
            }
        }

        // The exe's texts (root.srp_weapon_view_texts): the title (the button's label too) and the
        // distance's note.
        public function answerTexts(values:Array):void
        {
            if (values.length < 2 || String(values[0]) == "")
            {
                return;
            }
            title = String(values[0]);
            button.label = title;
            note.text = String(values[1]);
            if (values.length >= 3 && String(values[2]).indexOf("%1$s") >= 0)
            {
                unit = String(values[2]);
            }
            // the preview's three views' names
            for (var view:int = 0; view < tabs.length && 3 + view < values.length; view++)
            {
                tabs[view].label = String(values[3 + view]);
            }
            // the buttons' texts: "Default settings", "Close"
            if (values.length >= 9)
            {
                defaultText = String(values[7]);
                closeText = String(values[8]);
            }
            shown();
        }

        // The exe's answer for a slider (root.srp_slider_option): its name, label, saved, least, most
        // and steps a unit.
        public function answerSlider(values:Array):void
        {
            if (values.length < 6 || !owns(values[0]))
            {
                return;
            }
            var key:String = values[0];
            var unit:int = int(values[5]);
            var least:int = int(values[3]), most:int = int(values[4]), value:int = int(values[2]);
            if (!(unit > 0) || !(most > least) || value < least || value > most)
            {
                return;
            }
            var slider:SrvOptionSlider = sliders[key];
            var label:String = values[1];
            slider.label = label.length > 0 && label.charAt(label.length - 1) != ":" ? label + ":" : label;
            slider.dataProvider = {minimum: least / unit, maximum: most / unit, snapInterval: SNAP[key]};
            units[key] = unit;
            saved[key] = value / unit;
            show(slider, saved[key]);
            grey();
            shown();
        }

        // The exe's answer for the checkbox (root.srp_check_option, after the name): its label and
        // saved state.
        public function answerCheck(values:Array):void
        {
            if (values.length < 2 || (values[1] != "1" && values[1] != "0"))
            {
                return;
            }
            box.label = values[0];
            savedWorld = values[1];
            box.selected = savedWorld == "1";
            grey();
            shown();
        }

        // The gameplay tab's layout: the button at (x, y), as wide as the tab's preset buttons (wider for its
        // text); `look` the tab's headings' (its "Friends:"), which the sliders' labels take as World FOV's.
        public function layout(look:TextField, x:Number, y:Number):void
        {
            this.look = look;
            button.x = Math.round(x);
            button.y = Math.round(y);
            button.width = BUTTON_WIDTH;
            button.validateNow();
            if (button.textField != null && button.textField.textWidth + 16 > button.textField.width * button.scaleX)
            {
                button.width = BUTTON_WIDTH + button.textField.textWidth + 16 - button.textField.width * button.scaleX;
                button.validateNow();
            }
            if (open)
            {
                build();
            }
        }

        // A slider or the checkbox is not its saved value.
        public function pending(control:Object):Boolean
        {
            if (control == box)
            {
                return savedWorld != "" && state() != savedWorld;
            }
            for each (var key:String in SLIDERS)
            {
                if (control == sliders[key])
                {
                    return saved[key] != undefined && steps(key) != Math.round(saved[key] * units[key]);
                }
            }
            return false;
        }

        // Apply: every pending value saved (the exe sets it, writes user.cfg and answers); closed.
        public function save():void
        {
            send();
            close();
        }

        // Every pending value saved, the window left as it is.
        private function send():void
        {
            var calls:Array = [];
            if (pending(box))
            {
                savedWorld = state();
                calls.push([WORLD, savedWorld]);
            }
            for each (var key:String in SLIDERS)
            {
                if (pending(sliders[key]))
                {
                    saved[key] = steps(key) / units[key];
                    calls.push([key, String(steps(key))]);
                }
            }
            // each answer reverts what it names to the saved value: all are taken before any call
            for each (var call:Array in calls)
            {
                SrpExe.call(call[0], call[1]);
            }
        }

        // Cancel: the saved values back; closed.
        public function revert():void
        {
            if (savedWorld != "")
            {
                box.selected = savedWorld == "1";
            }
            for each (var key:String in SLIDERS)
            {
                if (saved[key] != undefined)
                {
                    show(sliders[key], saved[key]);
                }
            }
            grey();
            close();
        }

        // Esc, the close button, a press beside it, another tab, Apply, Cancel; whether it was open. Its
        // values stay the tab's pending changes.
        public function close():Boolean
        {
            if (!open)
            {
                return false;
            }
            visible = false;
            preview.release();
            if (stage != null)
            {
                stage.removeEventListener(Event.RESIZE, onResize);
            }
            return true;
        }

        private function onOpen(event:ButtonEvent):void
        {
            if (stage == null || !ready)
            {
                return;
            }
            parent.setChildIndex(this, parent.numChildren - 1);
            visible = true;
            stage.addEventListener(Event.RESIZE, onResize);
            build();
        }

        // Default: the checkbox off and every slider at revision 74's value, each a move of its own (pending
        // until Apply, as the player's), the preview drawn again.
        private function onDefault(event:ButtonEvent):void
        {
            if (box.selected)
            {
                box.selected = false;
            }
            for each (var key:String in SLIDERS)
            {
                var slider:SrvOptionSlider = sliders[key];
                if (slider.selectedIndex != DEFAULTS[key])
                {
                    slider.slider.value = DEFAULTS[key];
                }
            }
            refresh();
        }

        // A copy of one of the Settings window's buttons (its symbol), in this window.
        private function made(source:Object, name:String, click:Function):Button
        {
            var Kind:Class = source.constructor as Class;
            var button:Button = new Kind() as Button;
            button.name = name;
            button.addEventListener(ButtonEvent.CLICK, click);
            panel.addChild(button);
            return button;
        }

        // A button at `width`, wider only for its text (16 px to spare, as the tab's button).
        private static function fit(button:Button, width:Number):void
        {
            button.width = width;
            button.validateNow();
            if (button.textField != null && button.textField.textWidth + 16 > button.textField.width * button.scaleX)
            {
                button.width = width + button.textField.textWidth + 16 - button.textField.width * button.scaleX;
                button.validateNow();
            }
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

        // The window at the screen's centre, the screen dimmed behind it (SrpKeysWindow's): its rows in the
        // tab's look, top to bottom, its frame around them.
        private function build():void
        {
            var from:Point = parent.globalToLocal(new Point(0, 0));
            var to:Point = parent.globalToLocal(new Point(stage.stageWidth, stage.stageHeight));
            var screenWidth:Number = to.x - from.x, screenHeight:Number = to.y - from.y;
            var g:Graphics = backdrop.graphics;
            g.clear();
            g.beginFill(0x000000, 0.6);
            g.drawRect(from.x, from.y, screenWidth, screenHeight);
            g.endFill();
            var left:Number = FRAME_LEFT + SIDE;
            var top:Number = FRAME_TOP + SPACE;
            var at:Number = top;
            // the preview's column: the tabs side by side at their own width (the Settings window's tab bar:
            // no room between), centred over the preview under them
            var tabsWidth:Number = 0;
            for (var view:int = 0; view < tabs.length; view++)
            {
                tabs[view].validateNow();
                tabsWidth += tabs[view].width;
            }
            var tabX:Number = left + Math.round((SrpWeaponPreview.WIDTH - tabsWidth) / 2);
            for (view = 0; view < tabs.length; view++)
            {
                tabs[view].x = tabX;
                tabs[view].y = at;
                tabX += tabs[view].width;
            }
            at += TAB_HEIGHT + 4;
            preview.x = left;
            preview.y = at;
            var previewBottom:Number = at + SrpWeaponPreview.HEIGHT;
            // the controls' column, right of it, its first label's text at the picture's top (the owner: "text
            // can be on the level of the image")
            left += SrpWeaponPreview.WIDTH + COLUMN_GAP;
            // (a text field's text starts 2 px under its top)
            at = preview.y - 2;
            for each (var key:String in SLIDERS)
            {
                var slider:SrvOptionSlider = sliders[key];
                if (key == DISTANCE)
                {
                    box.x = left + 6;
                    box.y = at;
                    at += BOX_ROW;
                }
                if (look != null)
                {
                    SrpLanguage.restyle(slider.textField, look);
                }
                slider.x = left;
                slider.y = at - slider.textField.y;
                at = slider.y + slider.slider.y + slider.slider.height + ROW_GAP;
            }
            fitValues();
            centimetres();
            refresh();
            note.defaultTextFormat = SrpLanguage.noteFormat(content);
            note.setTextFormat(note.defaultTextFormat);
            note.embedFonts = true;
            note.x = left;
            note.width = INSIDE;
            note.y = at - ROW_GAP + 2;
            at = note.y + note.textHeight + 4 + ROW_GAP;
            // "Default settings" under them, on a line of its own, as wide as the Graphics tab's buttons (wider
            // only for its text)
            resetButton.label = defaultText;
            resetButton.alpha = 1;
            resetButton.x = left;
            resetButton.y = at;
            fit(resetButton, BUTTON_WIDTH);
            at = resetButton.y + resetButton.height + BUTTON_GAP;
            // Apply and Close under it, side by side: the Settings window's own
            if (applyButton == null && applyFrom != null && applyFrom() != null)
            {
                applyButton = made(applyFrom(), NAME + "_apply", onApply);
            }
            if (closeButton2 == null && closeFrom != null && closeFrom() != null)
            {
                closeButton2 = made(closeFrom(), NAME + "_close_button", onClose);
            }
            var rowX:Number = left;
            var rowBottom:Number = at;
            if (applyButton != null)
            {
                applyButton.label = (applyFrom() as Button).label;
                applyButton.alpha = 1;
                applyButton.x = rowX;
                applyButton.y = at;
                fit(applyButton, (applyFrom() as Button).width);
                applyButton.enabled = changed();
                rowX += applyButton.width + BUTTON_GAP;
                rowBottom = Math.max(rowBottom, applyButton.y + applyButton.height);
            }
            if (closeButton2 != null)
            {
                closeButton2.label = closeText;
                closeButton2.alpha = 1;
                closeButton2.x = rowX;
                closeButton2.y = at;
                fit(closeButton2, (closeFrom() as Button).width);
                rowBottom = Math.max(rowBottom, closeButton2.y + closeButton2.height);
            }
            at = rowBottom;
            at = Math.max(at, previewBottom);
            var w:Number = FRAME_LEFT + FRAME_RIGHT + 2 * SIDE + SrpWeaponPreview.WIDTH + COLUMN_GAP + INSIDE;
            var h:Number = at + SPACE + FRAME_BOTTOM;
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

        // SrpSettingsGround in nine parts (SrpKeysWindow's): the corners and the title band as they are.
        private function drawFrame(w:Number, h:Number):void
        {
            var art:BitmapData = new SrpSettingsGround();
            var g:Graphics = frame.graphics;
            g.clear();
            var xs:Array = [0, FRAME_LEFT, art.width - FRAME_RIGHT, art.width];
            // each part's source rows: the title band, the inside to the button bar, the frame's bottom edge
            var ys:Array = [[0, FRAME_TOP], [FRAME_TOP, ART_INSIDE_END], [art.height - FRAME_BOTTOM, art.height]];
            var tx:Array = [0, FRAME_LEFT, w - FRAME_RIGHT, w];
            var ty:Array = [0, FRAME_TOP, h - FRAME_BOTTOM, h];
            for (var i:int = 0; i < 3; i++)
            {
                for (var j:int = 0; j < 3; j++)
                {
                    var sx:Number = (tx[i + 1] - tx[i]) / (xs[i + 1] - xs[i]);
                    var sy:Number = (ty[j + 1] - ty[j]) / (ys[j][1] - ys[j][0]);
                    var m:Matrix = new Matrix(sx, 0, 0, sy, tx[i] - xs[i] * sx, ty[j] - ys[j][0] * sy);
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

        // A text in the Settings' own font (its title's), on one line (SrpKeysWindow's).
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

        private function state():String
        {
            return box.selected ? "1" : "0";
        }

        // A slider's value in its steps.
        private function steps(key:String):int
        {
            return Math.round(sliders[key].selectedIndex * units[key]);
        }

        // A value shown at once, the thumb drawn now (a disabled slider draws no later).
        private function show(slider:SrvOptionSlider, value:Number):void
        {
            var on:Boolean = slider.slider.enabled;
            slider.slider.enabled = true;
            slider.selectedIndex = value;
            slider.slider.validateNow();
            slider.slider.enabled = on;
            if (slider == sliders[DISTANCE])
            {
                centimetres();
            }
        }

        // The distance's value in the dictionary's centimetres with its sign ("+10 cm", "0 cm", "-5 cm"; a
        // plain hyphen, as the stock sliders show a number), where the slider shows its number.
        private function centimetres(event:Event = null):void
        {
            var slider:SrvOptionSlider = sliders[DISTANCE];
            if (slider.textFieldValue != null)
            {
                slider.textFieldValue.text = withUnit(Math.round(slider.selectedIndex));
            }
        }

        public function withUnit(value:int):String
        {
            return unit.split("%1$s").join((value > 0 ? "+" : "") + value);
        }

        // The values each slider shows, the widest of them: its own SAMPLES, the distance's ends in the
        // dictionary's unit.
        public function samples(key:String):Array
        {
            return key == DISTANCE ? [withUnit(-20), withUnit(50), withUnit(-10)] : SAMPLES[key];
        }

        // Each value field one line, as wide as the widest value it shows and VALUE_MARGIN more (never
        // narrower than the stock field), growing to its right.
        private function fitValues():void
        {
            for each (var key:String in SLIDERS)
            {
                var field:TextField = sliders[key].textFieldValue;
                if (field == null)
                {
                    continue;
                }
                var shown:String = field.text;
                field.wordWrap = false;
                var widest:Number = 0;
                for each (var sample:String in samples(key))
                {
                    field.text = sample;
                    widest = Math.max(widest, field.textWidth);
                }
                field.text = shown;
                if (field.width < widest + VALUE_MARGIN)
                {
                    field.width = widest + VALUE_MARGIN;
                }
            }
        }

        // The three angles greyed and inactive while the weapon follows the world's projection.
        private function grey(event:Event = null):void
        {
            var on:Boolean = !box.selected;
            for each (var key:String in ANGLES)
            {
                var slider:SrvOptionSlider = sliders[key];
                slider.slider.enabled = on;
                slider.mouseChildren = on;
                slider.alpha = on ? 1 : GREYED;
            }
        }

        // Every answer of the exe is there.
        private function get ready():Boolean
        {
            var all:Boolean = savedWorld != "" && title != "";
            for each (var key:String in SLIDERS)
            {
                all = all && saved[key] != undefined;
            }
            return all;
        }

        // The button on the gameplay frame once the exe has answered.
        private function shown():void
        {
            button.visible = content.currentFrame == FRAME && ready;
            // Scaleform leaves a script-created control fully transparent.
            button.alpha = 1;
        }
    }
}
