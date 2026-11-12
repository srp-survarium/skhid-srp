package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import survarium.menu.Settings;

    // Placement only. Native options_item_int supplies the localized choices,
    // values, preset membership, change notifications and Apply/Cancel.
    public class SrpDistanceDetail
    {
        public static function apply(settings:Settings):void
        {
            var content:MovieClip = settings.settings_content;
            if (content == null) return;
            var picker:OptionStepperCyclic = content["_distance_detail"] as OptionStepperCyclic;
            if (picker == null && content.currentFrame == 3)
            {
                picker = new OptionStepperCyclic();
                picker.name = "_distance_detail";
                picker.enableInitCallback = false;
                content["_distance_detail"] = picker;
                content.addChild(picker);
            }
            if (picker != null) picker.visible = false;
        }

        public static function finish(settings:Settings):void
        {
            var content:MovieClip = settings.settings_content;
            if (content == null || content.currentFrame != 3) return;
            var picker:OptionStepperCyclic = content["_distance_detail"] as OptionStepperCyclic;
            var previous:DisplayObject = content["_motion_blur"] as DisplayObject;
            if (picker == null || previous == null) return;
            // One extra row, using the same arrow picker as the quality rows.
            var rows:Array = ["_texture_q", "_geometry_q", "_lighting_q", "_shadow_q",
                              "_water_q", "_decorations", "_post_process", "_ao_q",
                              "_particples_q", "_motion_blur"];
            var top:Number = content["_texture_q"].y;
            for (var i:int = 0; i < rows.length; i++) content[rows[i]].y = top + i * 30;
            picker.x = previous.x;
            picker.y = previous.y + 30;
            picker.width = previous.width;
            picker.height = previous.height;
            picker.alpha = 1;
            picker.visible = true;
            content.setChildIndex(picker, content.numChildren - 1);
        }
    }
}
