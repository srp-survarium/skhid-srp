package
{
    import flash.display.MovieClip;
    import flash.display.Shape;
    import flash.events.Event;
    import flash.text.TextField;
    import flash.text.TextFieldAutoSize;
    import scaleform.clik.events.ButtonEvent;

    // The Gameplay tab's "Weapon view" (exe patch 218, docs/notes/weapon-view-settings.md): how the
    // held gun is drawn, apart from the world's field of view (the "World FOV:" slider above it). A
    // button under the World FOV slider, labelled with the group's heading, opens a panel over the
    // left column (the tab has no room for the group in either column) holding
    // - three sliders of the weapon's own field of view, a rifle's degrees (each gun keeps 0.69d0's
    //   own ratio to them): at the hip (40 to 100, 65), over iron sights and collimators (20 to 65,
    //   32.5) and through magnified scopes (16.25 to 32.5, 32.5);
    // - the checkbox "Weapon at the world FOV": the held gun drawn at the world's projection, as before
    //   revision 74; the three sliders are greyed while it is ticked;
    // - the sight distance, 0 to 100 % of the way from 0.69d0's eye to revision 73's, aimed over iron
    //   sights and collimators.
    // The button again, another tab, Apply or Cancel close the panel. The exe keeps each value in
    // user.cfg and answers ExternalInterface.call(name[, value]) with root.srp_slider_option(name,
    // label, saved, least, most, steps a unit) for a slider, root.srp_check_option(name, label, saved)
    // for the checkbox; ExternalInterface.call("srp_weapon_view_texts") with
    // root.srp_weapon_view_texts(heading). Settings owns it: a move or a toggle is a pending change
    // of the tab (Apply saves it, the exe applying it at once, also in a match; Cancel puts the saved
    // values back).
    public class SrpWeaponView
    {
        public static const HIP:String = "srp_weapon_fov";
        public static const SIGHTS:String = "srp_weapon_fov_aimed";
        public static const SCOPES:String = "srp_weapon_fov_scope";
        public static const DISTANCE:String = "srp_sight_distance";
        public static const WORLD:String = "srp_weapon_fov_world";
        public static const TEXTS:String = "srp_weapon_view_texts";
        // In the panel's order: the three angles, then (after the checkbox) the distance.
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
        // The panel's inside margin and its rows' step (a slider's label over its track, as the stock ones).
        private static const PADDING:Number = 12;
        private static const BOX_ROW:Number = 34;
        private static const ROW_GAP:Number = 10;
        // The window's border (SrpVulkanOptions's) and a surface as dark as the window's.
        private static const EDGE:uint = 0x585A5D;
        private static const SURFACE:uint = 0x1B1C1D;

        public var button:SetingsButton;
        public var panel:MovieClip;
        public var heading:TextField;
        public var box:CheckBoxSkinned;
        // The sliders by name.
        public var sliders:Object = {};
        private var surface:Shape;
        private var content:MovieClip;
        // A slider's saved value in its units and its steps a unit, by name; absent until the exe answers.
        private var saved:Object = {};
        private var units:Object = {};
        // The checkbox's saved state, "1" or "0"; "" until the exe answers.
        private var savedWorld:String = "";
        private var open:Boolean = false;

        public function SrpWeaponView(content:MovieClip)
        {
            this.content = content;
            button = new SetingsButton();
            button.name = "_srp_weapon_view";
            button.autoSize = "none";
            button.focusable = true;
            button.toggle = false;
            // Scaleform leaves a script-created control fully transparent.
            button.alpha = 1;
            button.visible = false;
            button.addEventListener(ButtonEvent.CLICK, onPress);
            content.addChild(button);
            panel = new MovieClip();
            panel.name = "_srp_weapon_view_panel";
            panel.visible = false;
            surface = new Shape();
            panel.addChild(surface);
            heading = new TextField();
            heading.name = "_srp_weapon_view_heading";
            heading.selectable = false;
            heading.mouseEnabled = false;
            heading.autoSize = TextFieldAutoSize.LEFT;
            panel.addChild(heading);
            for each (var name:String in SLIDERS)
            {
                var slider:SrvOptionSlider = new SrvOptionSlider();
                slider.name = "_" + name;
                slider.defaultValue = DEFAULTS[name];
                slider.alpha = 1;
                panel.addChild(slider);
                sliders[name] = slider;
            }
            box = new CheckBoxSkinned();
            box.name = "_" + WORLD;
            box.autoSize = "none";
            box.enableInitCallback = false;
            box.focusable = true;
            box.selected = false;
            box.alpha = 1;
            box.addEventListener(Event.SELECT, grey);
            panel.addChild(box);
            content.addChild(panel);
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
            for each (var name:String in SLIDERS)
            {
                out.push(sliders[name]);
            }
            return out;
        }

        // On every bind of a tab (create_content): on top of the tab, closed, the exe asked on the
        // gameplay frame.
        public function place():void
        {
            open = false;
            content.setChildIndex(button, content.numChildren - 1);
            content.setChildIndex(panel, content.numChildren - 1);
            shown();
            if (content.currentFrame == FRAME)
            {
                SrpExe.call(TEXTS, null);
                SrpExe.call(WORLD, null);
                for each (var name:String in SLIDERS)
                {
                    SrpExe.call(name, null);
                }
            }
        }

        // The exe's texts (root.srp_weapon_view_texts): the heading, the button's label too.
        public function answerTexts(values:Array):void
        {
            if (values.length < 1 || String(values[0]) == "")
            {
                return;
            }
            heading.text = String(values[0]) + ":";
            button.label = String(values[0]);
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
            var name:String = values[0];
            var unit:int = int(values[5]);
            var least:int = int(values[3]), most:int = int(values[4]), value:int = int(values[2]);
            if (!(unit > 0) || !(most > least) || value < least || value > most)
            {
                return;
            }
            var slider:SrvOptionSlider = sliders[name];
            var label:String = values[1];
            slider.label = label.length > 0 && label.charAt(label.length - 1) != ":" ? label + ":" : label;
            slider.dataProvider = {minimum: least / unit, maximum: most / unit, snapInterval: SNAP[name]};
            units[name] = unit;
            saved[name] = value / unit;
            show(slider, saved[name]);
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

        // Its heading in the look of `look` (one of the tab's own: "Friends:"); the button at (x, y),
        // `buttonWidth` wide (the column's lists'); the panel over the rectangle from (left, top), `width`
        // wide, to `bottom`.
        public function layout(look:TextField, x:Number, y:Number, buttonWidth:Number, left:Number, top:Number,
            width:Number, bottom:Number):void
        {
            SrpLanguage.restyle(heading, look);
            button.x = x;
            button.y = y;
            button.width = buttonWidth;
            button.validateNow();
            panel.x = left;
            panel.y = top;
            var g:* = surface.graphics;
            g.clear();
            g.lineStyle(1, EDGE);
            g.beginFill(SURFACE, 1);
            g.drawRect(0, 0, width, bottom - top);
            g.endFill();
            heading.x = PADDING;
            heading.y = PADDING;
            var at:Number = heading.y + heading.height + 4;
            for each (var name:String in SLIDERS)
            {
                var slider:SrvOptionSlider = sliders[name];
                if (name == DISTANCE)
                {
                    box.x = PADDING;
                    box.y = at;
                    at += BOX_ROW;
                }
                SrpLanguage.restyle(slider.textField, look);
                slider.x = PADDING;
                slider.y = at - slider.textField.y;
                // the next row under the track, as the stock sliders' rows are spaced
                at = slider.y + slider.slider.y + slider.slider.height + ROW_GAP;
            }
        }

        // A slider or the checkbox is not its saved value.
        public function pending(control:Object):Boolean
        {
            if (control == box)
            {
                return savedWorld != "" && state() != savedWorld;
            }
            for each (var name:String in SLIDERS)
            {
                if (control == sliders[name])
                {
                    return saved[name] != undefined && steps(name) != Math.round(saved[name] * units[name]);
                }
            }
            return false;
        }

        // Apply: every pending value saved (the exe sets it, writes user.cfg and answers); the panel closed.
        public function save():void
        {
            var calls:Array = [];
            if (pending(box))
            {
                savedWorld = state();
                calls.push([WORLD, savedWorld]);
            }
            for each (var name:String in SLIDERS)
            {
                if (pending(sliders[name]))
                {
                    saved[name] = steps(name) / units[name];
                    calls.push([name, String(steps(name))]);
                }
            }
            // each answer reverts what it names to the saved value: all are taken before any call
            for each (var call:Array in calls)
            {
                SrpExe.call(call[0], call[1]);
            }
            close();
        }

        // Cancel: the saved values back, the panel closed.
        public function revert():void
        {
            if (savedWorld != "")
            {
                box.selected = savedWorld == "1";
            }
            for each (var name:String in SLIDERS)
            {
                if (saved[name] != undefined)
                {
                    show(sliders[name], saved[name]);
                }
            }
            grey();
            close();
        }

        public function close():void
        {
            open = false;
            shown();
        }

        // The button: the panel opened (over everything of the tab: the Language list and its heading are
        // placed after this group) or closed.
        private function onPress(event:Event):void
        {
            open = !open;
            if (open)
            {
                content.setChildIndex(panel, content.numChildren - 1);
            }
            shown();
        }

        private function state():String
        {
            return box.selected ? "1" : "0";
        }

        // A slider's value in its steps.
        private function steps(name:String):int
        {
            return Math.round(sliders[name].selectedIndex * units[name]);
        }

        // A value shown at once, the thumb drawn now (a disabled slider draws no later).
        private function show(slider:SrvOptionSlider, value:Number):void
        {
            var on:Boolean = slider.slider.enabled;
            slider.slider.enabled = true;
            slider.selectedIndex = value;
            slider.slider.validateNow();
            slider.slider.enabled = on;
        }

        // The three angles greyed and inactive while the weapon follows the world's projection.
        private function grey(event:Event = null):void
        {
            var on:Boolean = !box.selected;
            for each (var name:String in ANGLES)
            {
                var slider:SrvOptionSlider = sliders[name];
                slider.slider.enabled = on;
                slider.mouseChildren = on;
                slider.alpha = on ? 1 : GREYED;
            }
        }

        // The button on the gameplay frame once the exe has answered; the panel while open and every
        // value is there.
        private function shown():void
        {
            var here:Boolean = content.currentFrame == FRAME;
            var ready:Boolean = savedWorld != "" && button.label != null && button.label != "";
            for each (var name:String in SLIDERS)
            {
                ready = ready && saved[name] != undefined;
            }
            button.visible = here && ready;
            panel.visible = here && ready && open;
        }
    }
}
