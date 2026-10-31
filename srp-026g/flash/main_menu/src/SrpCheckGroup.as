package
{
    import flash.display.MovieClip;
    import flash.text.TextField;
    import flash.text.TextFormat;

    // A group of the gameplay tab: a heading, as the tab's own group headings
    // ("Friends:", "Crosshair:"), with checkboxes the exe keeps in user.cfg
    // under it (SrpCheckOption; exe patch 83's "Hide hit markers" and "Hide
    // player scores", docs/notes/hide-hit-markers-scores.md). The exe's answer
    // for each option names the group's heading too (root.srp_check_option(
    // name, label, saved, heading)). Settings places the group (layout) in its
    // column as the tab's groups are placed; the controls are script-added
    // children of the tab content, shown on the gameplay frame only.
    public class SrpCheckGroup
    {
        private static const GAMEPLAY_FRAME:int = 1;

        public var heading:TextField;
        public var options:Array = [];
        private var content:MovieClip;

        // `style`: one of the tab's own group headings, whose look the
        // heading takes.
        public function SrpCheckGroup(content:MovieClip, style:TextField, names:Array)
        {
            this.content = content;
            heading = new TextField();
            heading.embedFonts = style.embedFonts;
            heading.selectable = false;
            heading.mouseEnabled = false;
            var format:TextFormat = style.defaultTextFormat;
            heading.defaultTextFormat = format;
            heading.filters = style.filters;
            heading.width = style.width;
            heading.height = style.height;
            heading.name = "_srp_check_group";
            heading.visible = false;
            content.addChild(heading);
            for each (var name:String in names)
            {
                options.push(new SrpCheckOption(content, name, 0, 0));
            }
        }

        // The option of this name, or null.
        public function option(name:String):SrpCheckOption
        {
            for each (var each:SrpCheckOption in options)
            {
                if (each.name == name)
                {
                    return each;
                }
            }
            return null;
        }

        // On every bind of a tab (create_content): each option placed (on top,
        // the exe asked on the gameplay frame), the heading on top too.
        public function place():void
        {
            content.setChildIndex(heading, content.numChildren - 1);
            for each (var each:SrpCheckOption in options)
            {
                each.place();
            }
            show();
        }

        // The exe's answer for one option: name, label, saved, heading.
        public function answer(values:Array):void
        {
            var each:SrpCheckOption = values.length > 0 ? option(values[0]) : null;
            if (each == null)
            {
                return;
            }
            each.answer(values.slice(1, 3));
            if (values.length > 3)
            {
                heading.text = values[3];
            }
            show();
        }

        // The heading at (x, y), the checkboxes under it at controlX, the
        // first `first` below the heading, each next `step` below the last
        // (the tab's own spacing), the heading in the look of `style` (a
        // heading of the tab). The top of the last checkbox.
        public function layout(style:TextField, x:Number, controlX:Number, y:Number, first:Number, step:Number):Number
        {
            SrpLanguage.restyle(heading, style);
            heading.x = x;
            heading.y = y;
            var top:Number = y + first - step;
            for each (var each:SrpCheckOption in options)
            {
                top += step;
                each.box.x = controlX;
                each.box.y = top;
            }
            return top;
        }

        private function show():void
        {
            heading.visible = content.currentFrame == GAMEPLAY_FRAME && heading.text != "";
        }
    }
}
