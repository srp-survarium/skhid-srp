package
{
    import flash.display.MovieClip;
    import flash.events.Event;
    import flash.external.ExternalInterface;
    import scaleform.clik.events.IndexEvent;

    // The controls tab's "Separate vertical sensitivity" and "Vertical
    // sensitivity" (exe patch 110, docs/notes/mouse-pitch.md), under the stock
    // Mouse sensitivity slider. The exe keeps both in user.cfg
    // (srp_mouse_split, srp_mouse_vertical): it answers
    // ExternalInterface.call(SPLIT[, "1" or "0"]) with
    // root.srp_check_option(SPLIT, label, saved) and
    // ExternalInterface.call(VERTICAL[, hundredths]) with
    // root.srp_slider_option(VERTICAL, label, saved hundredths).
    //
    // Off, the vertical slider is greyed and shows the main slider's value as
    // it moves; on, it is the player's own, starting from the value it showed.
    // Settings counts a change of either as a pending change of the tab:
    // Apply saves them (save), Cancel puts the saved state back (revert).
    public class SrpMouseVertical
    {
        public static const SPLIT:String = "srp_mouse_split";
        public static const VERTICAL:String = "srp_mouse_vertical";
        private static const FRAME:int = SrpCheckOption.CONTROLS_FRAME;
        // The greyed slider's opacity, as a disabled stock control's.
        private static const GREYED:Number = 0.45;

        public var check:SrpCheckOption;
        public var slider:SrvOptionSlider;
        private var content:MovieClip;
        // The main slider this one follows while off (the tab's, recreated at
        // each visit of its frame).
        private var main:SrvOptionSlider;
        // The saved vertical value in hundredths; -1 until the exe answers.
        private var saved:int = -1;

        public function SrpMouseVertical(content:MovieClip)
        {
            this.content = content;
            check = new SrpCheckOption(content, SPLIT, 0, 0, FRAME);
            check.box.addEventListener(Event.SELECT, onSplit);
            slider = new SrvOptionSlider();
            slider.name = "_" + VERTICAL;
            slider.visible = false;
            content.addChild(slider);
        }

        // On every bind of a tab (create_content, before the tab's settings
        // are bound): on top of the tab, the exe asked on the controls frame.
        public function place():void
        {
            check.place();
            content.setChildIndex(slider, content.numChildren - 1);
            shown();
            if (content.currentFrame == FRAME)
            {
                ask(null);
            }
        }

        // After the tab's settings are bound: the main slider's range and
        // value taken, followed while off.
        public function bind():void
        {
            var stock:SrvOptionSlider = content.currentFrame == FRAME ? Object(content)._slider_mouse_sens as SrvOptionSlider : null;
            if (stock != main)
            {
                if (main != null)
                {
                    main.removeEventListener(IndexEvent.INDEX_CHANGE, onMain);
                }
                main = stock;
                if (main != null)
                {
                    main.addEventListener(IndexEvent.INDEX_CHANGE, onMain);
                }
            }
            if (main == null)
            {
                shown();
                return;
            }
            slider.dataProvider = {minimum: main.slider.minimum, maximum: main.slider.maximum,
                snapInterval: main.slider.snapInterval};
            follow();
            shown();
        }

        // The exe's answer to SPLIT (root.srp_check_option, after the name).
        public function answerSplit(values:Array):void
        {
            check.answer(values);
            revert();
        }

        // The exe's answer to VERTICAL (root.srp_slider_option): its name, label
        // and saved hundredths.
        public function answerVertical(values:Array):void
        {
            if (values.length < 3 || values[0] != VERTICAL || !(int(values[2]) >= 1))
            {
                return;
            }
            slider.label = values[1];
            saved = int(values[2]);
            revert();
            shown();
        }

        // The checkbox or the vertical value is not the saved one.
        public function get pending():Boolean
        {
            return check.pending || verticalPending;
        }

        public function get verticalPending():Boolean
        {
            return saved > 0 && split && hundredths(slider.selectedIndex) != saved;
        }

        // Apply: the vertical value (when it is the player's) and the checkbox.
        public function save():void
        {
            if (verticalPending)
            {
                saved = hundredths(slider.selectedIndex);
                ask(String(saved));
            }
            check.save();
        }

        // Cancel: the saved state back; off, the slider follows the main one.
        public function revert():void
        {
            check.revert();
            if (split && saved > 0)
            {
                show(saved / 100);
            }
            follow();
            shown();
        }

        private function get split():Boolean
        {
            return check.box.selected;
        }

        // Off: the main slider's value, greyed and inactive; on: its own.
        private function follow():void
        {
            var on:Boolean = split;
            if (!on && main != null)
            {
                show(main.selectedIndex);
            }
            slider.slider.enabled = on;
            slider.mouseChildren = on;
            slider.alpha = on ? 1 : GREYED;
        }

        private function onMain(event:Event):void
        {
            if (!split)
            {
                show(main.selectedIndex);
            }
        }

        // A value shown at once: the thumb drawn now (a disabled slider draws no later).
        private function show(value:Number):void
        {
            var on:Boolean = slider.slider.enabled;
            slider.slider.enabled = true;
            slider.selectedIndex = value;
            slider.slider.validateNow();
            slider.slider.enabled = on;
        }

        // A toggle: on continues from the value shown, off follows again.
        private function onSplit(event:Event):void
        {
            follow();
        }

        // Shown on the controls frame once both answers came, and its main
        // slider is there.
        private function shown():void
        {
            slider.visible = content.currentFrame == FRAME && saved > 0 && check.box.visible;
        }

        private static function hundredths(value:Number):int
        {
            return Math.round(value * 100);
        }

        // Outside the game (a test player) there is no exe to call.
        private function ask(value:String):void
        {
            try
            {
                if (value == null)
                {
                    ExternalInterface.call(VERTICAL);
                }
                else
                {
                    ExternalInterface.call(VERTICAL, value);
                }
            }
            catch (error:Error)
            {
            }
        }
    }
}
