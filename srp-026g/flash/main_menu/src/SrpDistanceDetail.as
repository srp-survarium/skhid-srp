package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import survarium.menu.Settings;

    // Native video option 19 remains r_use_texture_streaming (1 adaptive,
    // 0 maximum). Convert at the Settings boundary; the UI uses increasing
    // quality: Normal = 0, Maximum = 1. Native Apply/Cancel owns the value.
    public class SrpDistanceDetail
    {
        public static function value(tab:uint, id:uint, nativeValue:Number):Number
        {
            return tab == 2 && id == 19 ? 1 - nativeValue : nativeValue;
        }

        public static function setting(nativeSetting:Object):Object
        {
            // One localized native label carries title|normal|maximum.
            var texts:Array = nativeSetting.label.split("|");
            var setter:Function = nativeSetting.setter;
            return {label:texts[0], dataProvider:[texts[1], texts[2]],
                    setter:function(index:int):int { return 1 - int(setter(1 - index)); }};
        }

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
