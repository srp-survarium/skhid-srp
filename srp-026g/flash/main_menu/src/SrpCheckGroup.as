package
{
    import flash.display.MovieClip;
    import flash.external.ExternalInterface;
    import flash.text.TextField;
    import flash.text.TextFormat;

    // A group of the gameplay tab: a heading, as the tab's own group headings
    // ("Friends:", "Crosshair:"), with checkboxes the exe keeps in user.cfg
    // in it (SrpCheckOption; exe patch 83's "Hide hit markers" and "Hide
    // player scores", docs/notes/hide-hit-markers-scores.md). The exe gives the
    // heading with the group's other texts (root.srp_hud_texts, asked at each
    // bind of the gameplay tab). Settings places the heading (layout) and the
    // group's controls in its column as the tab's groups are placed; the
    // controls are script-added children of the tab content, shown on the
    // gameplay frame only.
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
            if (content.currentFrame == GAMEPLAY_FRAME)
            {
                try
                {
                    // the group's texts: root.srp_hud_texts (Settings.setHudTexts)
                    ExternalInterface.call("srp_hud_texts");
                }
                catch (error:Error)
                {
                }
            }
        }

        // The exe's answer for one option: name, label, saved.
        public function answer(values:Array):void
        {
            var each:SrpCheckOption = values.length > 0 ? option(values[0]) : null;
            if (each != null)
            {
                each.answer(values.slice(1, 3));
            }
        }

        // The heading's text (the exe's root.srp_hud_texts).
        public function title(text:String):void
        {
            heading.text = text;
            show();
        }

        // The heading at (x, y) in the look of `style` (a heading of the tab);
        // Settings places the group's controls under it.
        public function layout(style:TextField, x:Number, y:Number):void
        {
            SrpLanguage.restyle(heading, style);
            heading.x = x;
            heading.y = y;
        }

        private function show():void
        {
            heading.visible = content.currentFrame == GAMEPLAY_FRAME && heading.text != "";
        }
    }
}
