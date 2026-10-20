package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import flash.text.TextField;
    import survarium.menu.Settings;

    // The native video options send a slider for index 7 (Settings maps it to
    // settings_content._slider_fov), but the stock timeline never places one.
    // create_content calls apply first, so the slider exists when the options
    // are bound (label, range and value come from the native option), and
    // finish last, after the label is set.
    public class SrpFovSlider
    {
        private static const VIDEO_FRAME:int = 3;
        // Below gamma; the preset panel moves down to make room.
        private static const ROW:Number = 50;
        // The preset panel keeps its bottom edge; its two buttons move down
        // less than the panel top, so both still fit inside it.
        private static const BUTTONS:Number = 26;

        // The timeline recreates frame 3 children on each visit; move each
        // set once.
        private static var moved:DisplayObject;

        public static function apply(settings:Settings):void
        {
            var content:MovieClip = settings.settings_content;
            if (content == null)
            {
                return;
            }
            var tabs:Object = content;
            var slider:SrvOptionSlider = tabs._slider_fov as SrvOptionSlider;
            var video:Boolean = content.currentFrame == VIDEO_FRAME;
            if (slider == null)
            {
                if (!video)
                {
                    return;
                }
                slider = new SrvOptionSlider();
                slider.name = "_slider_fov";
                tabs._slider_fov = slider;
                content.addChild(slider);
            }
            // Hidden until finish shows it: script-added children survive frame
            // changes, and create_content returns early (skipping finish) while
            // the tab's settings have not arrived, when an unplaced slider would
            // sit at 0,0 over the monitor list.
            slider.visible = false;
        }

        public static function finish(settings:Settings):void
        {
            var content:MovieClip = settings.settings_content;
            if (content == null)
            {
                return;
            }
            var tabs:Object = content;
            var slider:SrvOptionSlider = tabs._slider_fov as SrvOptionSlider;
            var gamma:DisplayObject = tabs.slider_gamma;
            if (slider == null || content.currentFrame != VIDEO_FRAME || gamma == null)
            {
                return;
            }
            slider.visible = true;
            // Scaleform leaves the script-created slider fully transparent.
            slider.alpha = 1;
            // The game labels gamma "Gamma:" but FOV without the colon, in
            // every language.
            var label:TextField = slider.textField;
            if (label != null && label.text.length > 0 && label.text.charAt(label.text.length - 1) != ":")
            {
                label.text += ":";
            }
            slider.x = gamma.x;
            slider.y = gamma.y + ROW;
            var optimal:DisplayObject = tabs.button_optimal_video;
            var standard:DisplayObject = tabs.button_default_video;
            var panel:DisplayObject = tabs.bottomContainer;
            if (optimal == null || standard == null || panel == null || optimal == moved)
            {
                return;
            }
            moved = optimal;
            panel.y += ROW;
            panel.height -= ROW;
            optimal.y += BUTTONS;
            standard.y += BUTTONS;
        }
    }
}
