package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import flash.text.TextField;
    import survarium.menu.Settings;

    // srp's rows of the Graphics tab's left column: the FOV slider under gamma
    // and the Renderer list under Monitor.
    //
    // The native video options send a slider for index 7 (Settings maps it to
    // settings_content._slider_fov), but the stock timeline never places one.
    // create_content calls apply first, so the slider exists when the options
    // are bound (label, range and value come from the native option), and
    // finish last, after the label is set. The Renderer list is Settings'
    // (SrpRendererChoice); finish places it.
    //
    // To make room, the rows under Monitor move down one of its steps and the
    // Renderer note's room (kept whether it shows or not: the rows never move
    // with it), and the preset panel's top moves down by that and the slider's
    // row; its two buttons go side by side in it.
    public class SrpVideoTab
    {
        private static const VIDEO_FRAME:int = 3;
        // Below gamma.
        private static const ROW:Number = 50;
        // Between the two preset buttons, side by side.
        private static const GAP:Number = 8;

        // The timeline recreates frame 3 children on each visit; move each
        // set once.
        private static var moved:DisplayObject;
        // Monitor's step to the row under it, read from the set before it moved.
        private static var step:Number = 0;
        // How far the rows under the Renderer list move: a step and its note's room.
        private static function get shift():Number
        {
            return step + SrpRendererChoice.NOTE_ROOM;
        }

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

        // `renderer`: the Renderer list (Settings shows it on this frame once
        // the exe has answered).
        public static function finish(settings:Settings, renderer:SrpRendererChoice):void
        {
            var content:MovieClip = settings.settings_content;
            if (content == null)
            {
                return;
            }
            var tabs:Object = content;
            var slider:SrvOptionSlider = tabs._slider_fov as SrvOptionSlider;
            var gamma:DisplayObject = tabs.slider_gamma;
            var monitor:DisplayObject = tabs._select_monitor;
            var resolution:DisplayObject = tabs._fscr_resolution;
            if (slider == null || content.currentFrame != VIDEO_FRAME || gamma == null || monitor == null
                || resolution == null)
            {
                return;
            }
            var optimal:DisplayObject = tabs.button_optimal_video;
            var standard:DisplayObject = tabs.button_default_video;
            var panel:DisplayObject = tabs.bottomContainer;
            if (optimal != null && standard != null && panel != null && optimal != moved)
            {
                moved = optimal;
                step = resolution.y - monitor.y;
                for each (var row:DisplayObject in [resolution, tabs._fullscreen, tabs._vsync, tabs._antialias,
                    tabs._filtering, gamma])
                {
                    if (row != null)
                    {
                        row.y += shift;
                    }
                }
                panel.y += ROW + shift;
                panel.height -= ROW + shift;
                var width:Number = (optimal.width - GAP) / 2;
                optimal.width = standard.width = width;
                standard.x = optimal.x + width + GAP;
                optimal.y = standard.y = panel.y + (panel.height - optimal.height) / 2;
            }
            renderer.layout(monitor, monitor.y + step);
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
        }
    }
}
