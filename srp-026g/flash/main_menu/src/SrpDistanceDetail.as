package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import survarium.menu.Settings;

    // Native video option 19: r_use_texture_streaming. Off keeps full
    // texture, model and material detail. Native Apply/Cancel owns the value.
    public class SrpDistanceDetail
    {
        public static function apply(settings:Settings):void
        {
            var content:MovieClip = settings.settings_content;
            if (content == null) return;
            var checkbox:CheckBoxSkinned = content["_distance_detail"] as CheckBoxSkinned;
            if (checkbox == null && content.currentFrame == 3)
            {
                checkbox = new SrpDistanceDetailCheckBox();
                checkbox.name = "_distance_detail";
                content["_distance_detail"] = checkbox;
                content.addChild(checkbox);
            }
            if (checkbox != null) checkbox.visible = false;
        }

        public static function finish(settings:Settings):void
        {
            var content:MovieClip = settings.settings_content;
            if (content == null || content.currentFrame != 3) return;
            var checkbox:CheckBoxSkinned = content["_distance_detail"] as CheckBoxSkinned;
            var previous:DisplayObject = content["_motion_blur"] as DisplayObject;
            if (checkbox == null || previous == null) return;
            // Keep the graphics column inside the existing window. The stock
            // rows leave 8px gaps; 3px gaps make room for a multiline checkbox.
            var rows:Array = ["_texture_q", "_geometry_q", "_lighting_q", "_shadow_q",
                              "_water_q", "_decorations", "_post_process", "_ao_q",
                              "_particples_q", "_motion_blur"];
            var top:Number = content["_texture_q"].y;
            for (var i:int = 0; i < rows.length; i++) content[rows[i]].y = top + i * 30;
            checkbox.x = previous.x;
            checkbox.y = previous.y + previous.height + 7;
            checkbox.width = previous.width;
            checkbox.height = 60;
            checkbox.alpha = 1;
            checkbox.visible = true;
        }
    }
}
