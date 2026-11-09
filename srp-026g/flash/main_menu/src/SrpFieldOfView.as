package
{
    import flash.display.MovieClip;
    import flash.text.TextField;

    // The Gameplay tab's "Field of view" slider (patch 22's uro_fov, the camera's angle in degrees
    // unaimed, 50 to 100), the stock slider at its size. Exe patch 83
    // keeps it: it answers ExternalInterface.call("uro_fov"[, degrees]) with
    // root.srp_slider_option(name, label, saved degrees, least, most), setting the degrees first
    // (user.cfg at once). The video options' own option 7 names no variable (the original's), so the
    // Graphics tab has no field of view. Its label is a group heading as "Friends:" and "HUD:" are (their
    // look), a group of the right column under the HUD group. Settings owns it: a move is a pending change of the tab
    // (Apply saves it, Cancel or leaving without Apply puts the saved value back). A script-added child
    // of the tab content, shown on the gameplay frame once the exe has answered.
    public class SrpFieldOfView
    {
        public static const NAME:String = "uro_fov";
        private static const FRAME:int = SrpCheckOption.GAMEPLAY_FRAME;
        // The game's own value (uro_fov's in the image), which a double click on the value puts back.
        private static const DEFAULT:Number = 65;

        public var slider:SrvOptionSlider;
        private var content:MovieClip;
        // The saved degrees; -1 until the exe answers.
        private var saved:int = -1;

        public function SrpFieldOfView(content:MovieClip)
        {
            this.content = content;
            slider = new SrvOptionSlider();
            slider.name = "_srp_fov";
            slider.defaultValue = DEFAULT;
            slider.visible = false;
            content.addChild(slider);
        }

        // On every bind of a tab (create_content): on top of the tab, the exe asked on the gameplay
        // frame.
        public function place():void
        {
            content.setChildIndex(slider, content.numChildren - 1);
            shown();
            if (content.currentFrame == FRAME)
            {
                SrpExe.call(NAME, null);
            }
        }

        // The exe's answer (root.srp_slider_option): its name, label, saved degrees and range.
        public function answer(values:Array):void
        {
            if (values.length < 5 || values[0] != NAME || !(int(values[3]) > 0) || !(int(values[4]) >= int(values[3])))
            {
                return;
            }
            // the game labels the option without the colon its other sliders have ("Gamma:")
            var label:String = values[1];
            slider.label = label.length > 0 && label.charAt(label.length - 1) != ":" ? label + ":" : label;
            slider.dataProvider = {minimum: int(values[3]), maximum: int(values[4]), snapInterval: 1};
            saved = int(values[2]);
            revert();
            shown();
        }

        // Its label a group heading in `heading`'s look (one of the tab's own: "Friends:"), at `x`, its
        // top at `top`; the slider under it as the stock one is under its label.
        public function layout(heading:TextField, x:Number, top:Number):void
        {
            SrpLanguage.restyle(slider.textField, heading);
            slider.x = x;
            slider.y = top - slider.textField.y;
        }

        // The slider is not the saved value: a change for Apply.
        public function get pending():Boolean
        {
            return saved > 0 && degrees() != saved;
        }

        // Apply: the value saved (the exe sets uro_fov, writes user.cfg and answers).
        public function save():void
        {
            if (!pending)
            {
                return;
            }
            saved = degrees();
            SrpExe.call(NAME, String(saved));
        }

        // Cancel: the saved value.
        public function revert():void
        {
            if (saved > 0)
            {
                slider.selectedIndex = saved;
            }
        }

        private function degrees():int
        {
            return Math.round(slider.selectedIndex);
        }

        private function shown():void
        {
            slider.visible = content.currentFrame == FRAME && saved > 0;
            // Scaleform leaves a script-created control fully transparent.
            slider.alpha = 1;
        }
    }
}
