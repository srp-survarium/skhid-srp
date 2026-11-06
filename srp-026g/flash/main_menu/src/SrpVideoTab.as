package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import survarium.menu.Settings;

    // srp's rows of the Graphics tab's left column: the Renderer list and its
    // Vulkan options' row under Monitor (Settings' SrpRendererChoice; finish
    // places them). The field of view is the Gameplay tab's (SrpFieldOfView):
    // the video options' own option 7 names no variable, and Settings binds
    // no control to it.
    //
    // To make room, the rows under Monitor move down two of its steps (the
    // Renderer's and the Vulkan options' rows) and the note's room (kept
    // whether it shows or not: the rows never move with it). The preset panel
    // stays where it was with the FOV slider under gamma (that row's room is
    // the Vulkan options' now), its two buttons side by side in it.
    public class SrpVideoTab
    {
        public static const VIDEO_FRAME:int = 3;
        // The preset panel's top under the timeline's: a slider's row under
        // gamma, the Renderer's row and the note's room.
        private static const ROW:Number = 50;
        // Between the two preset buttons, side by side.
        private static const GAP:Number = 8;

        // The timeline recreates frame 3 children on each visit; move each
        // set once.
        private static var moved:DisplayObject;
        // Monitor's step to the row under it, read from the set before it moved.
        private static var step:Number = 0;
        // How far the rows under the Renderer list move: the Vulkan options'
        // row, its own and the note's room.
        private static function get shift():Number
        {
            return 2 * step + SrpRendererChoice.NOTE_ROOM;
        }

        // `renderer`: the Renderer list and the Vulkan options (Settings shows
        // them on this frame once the exe has answered).
        public static function finish(settings:Settings, renderer:SrpRendererChoice):void
        {
            var content:MovieClip = settings.settings_content;
            if (content == null || content.currentFrame != VIDEO_FRAME)
            {
                return;
            }
            var tabs:Object = content;
            var gamma:DisplayObject = tabs.slider_gamma;
            var monitor:DisplayObject = tabs._select_monitor;
            var resolution:DisplayObject = tabs._fscr_resolution;
            var optimal:DisplayObject = tabs.button_optimal_video;
            var standard:DisplayObject = tabs.button_default_video;
            var panel:DisplayObject = tabs.bottomContainer;
            if (gamma == null || monitor == null || resolution == null || optimal == null || standard == null || panel == null)
            {
                return;
            }
            var rows:Array = [resolution, tabs._fullscreen, tabs._vsync, tabs._antialias, tabs._filtering, gamma];
            if (optimal != moved)
            {
                moved = optimal;
                step = resolution.y - monitor.y;
                for each (var row:DisplayObject in rows)
                {
                    if (row != null)
                    {
                        row.y += shift;
                    }
                }
                var band:Number = ROW + step + SrpRendererChoice.NOTE_ROOM;
                panel.y += band;
                panel.height -= band;
                var width:Number = (optimal.width - GAP) / 2;
                optimal.width = standard.width = width;
                standard.x = optimal.x + width + GAP;
                optimal.y = standard.y = panel.y + (panel.height - optimal.height) / 2;
            }
            renderer.layout(monitor, monitor.y + step, step, rows);
        }
    }
}
