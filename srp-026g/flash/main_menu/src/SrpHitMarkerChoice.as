package
{
    import flash.display.MovieClip;
    import scaleform.clik.data.DataProvider;

    // The "HUD:" group's hit marker list (docs/notes/hide-hit-markers-scores.md):
    // All hit markers, Kill marker only, Hidden hit markers. Exe patch 83's
    // `srp_hide_hit_markers`, kept in user.cfg. Settings owns it: a pick is a
    // pending change of the tab (Apply saves it, Cancel or leaving without
    // Apply puts the saved one back). The exe answers
    // SrpExe.call(NAME[, index]) with root.srp_check_option(name, label,
    // saved index, running index, the three choices' texts); the label is
    // empty (the choices name themselves, as the crosshair list's do). The
    // list is a script-added child of the tab content, shown on the gameplay
    // frame only.
    public class SrpHitMarkerChoice
    {
        public static const NAME:String = "srp_hide_hit_markers";
        private static const GAMEPLAY_FRAME:int = 1;
        private static const CHOICES:int = 3;
        // The tab's own lists' width (the minimap position's).
        private static const MENU_WIDTH:Number = 184;

        public var menu:DropdownMenuSkinned;
        private var content:MovieClip;
        // The saved choice's index; -1 until the exe answers.
        private var saved:int = -1;

        public function SrpHitMarkerChoice(content:MovieClip)
        {
            this.content = content;
            menu = new DropdownMenuSkinned();
            menu.name = "_srp_hit_markers";
            menu.autoSize = "none";
            menu.dropdown = "OptionScrollingList";
            menu.itemRenderer = "OptionsListRenderer";
            menu.menuDirection = "down";
            menu.menuMargin = 0;
            menu.menuRowsFixed = false;
            menu.menuWidth = MENU_WIDTH;
            menu.menuWrapping = "normal";
            menu.scrollBar = "";
            menu.focusable = true;
            menu.width = MENU_WIDTH;
            // Scaleform leaves a script-created control fully transparent.
            menu.alpha = 1;
            menu.visible = false;
            content.addChild(menu);
        }

        // On every bind of a tab (create_content): the list on top, shown on
        // the gameplay frame, where the exe is asked for it.
        public function place():void
        {
            content.setChildIndex(menu, content.numChildren - 1);
            show();
            if (content.currentFrame == GAMEPLAY_FRAME)
            {
                SrpExe.call(NAME, null);
            }
        }

        // The exe's answer after the name: the label, the saved and running
        // indexes, the choices' texts.
        public function answer(values:Array):void
        {
            if (values.length < 3 + CHOICES || !/^[0-9]$/.test(values[1]) || int(values[1]) >= CHOICES)
            {
                return;
            }
            saved = int(values[1]);
            menu.dataProvider = new DataProvider(values.slice(3, 3 + CHOICES));
            menu.menuRowCount = CHOICES;
            revert();
            show();
        }

        // The pick is not the saved one: a change for Apply.
        public function get pending():Boolean
        {
            return saved != -1 && menu.selectedIndex != saved;
        }

        // Apply: the pick becomes the saved one (the exe writes it to user.cfg
        // and answers).
        public function save():void
        {
            if (!pending)
            {
                return;
            }
            saved = menu.selectedIndex;
            SrpExe.call(NAME, String(saved));
        }

        // Cancel: the saved one.
        public function revert():void
        {
            if (saved != -1)
            {
                menu.selectedIndex = saved;
            }
        }

        private function show():void
        {
            menu.visible = content.currentFrame == GAMEPLAY_FRAME && saved != -1;
        }
    }
}
