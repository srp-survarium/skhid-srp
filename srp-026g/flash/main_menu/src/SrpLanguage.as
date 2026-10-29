package
{
    import flash.display.MovieClip;
    import flash.external.ExternalInterface;
    import flash.text.TextField;
    import flash.text.TextFormat;
    import flash.text.TextFormatAlign;
    import scaleform.clik.data.DataProvider;

    // The gameplay tab's Language choice (exe patch 79,
    // docs/notes/language-option.md): its label, the list of every registered
    // language by its own name, and the note. Settings owns it and counts a
    // pick as a pending change of the tab: Apply saves it (save), Cancel or
    // leaving without Apply puts the saved one back (revert). The exe keeps
    // the choice in user.cfg and answers ExternalInterface.call("srp_language"
    // [, name]) with root.srp_language(label, note, saved, running, name, own
    // name, ...), which Settings passes to answer. The note shows while the
    // saved language is not the running one. The controls are script-added
    // children of the tab content, which survive frame changes: place shows
    // them on the gameplay frame only.
    public class SrpLanguage
    {
        private static const GAMEPLAY_FRAME:int = 1;
        // The bottom panel of the gameplay tab, where the seeding options were
        // (settingsTabs_15 hides them): centred on its left half (the Lobby
        // choice has the right, SrpLobbyScene), the label over the list as the
        // "Limit upload speed:" label was over its stepper.
        private static const CENTER_X:Number = 138;
        private static const LABEL_Y:Number = 289;
        private static const MENU_Y:Number = 315;
        private static const NOTE_Y:Number = 348;
        private static const MENU_WIDTH:Number = 184;
        private static const TEXT_WIDTH:Number = 300;

        public var menu:DropdownMenuSkinned;
        private var content:MovieClip;
        private var label:TextField;
        private var note:TextField;
        // The resource names in the menu's order (none until the exe answers),
        // the saved and the running one.
        private var names:Array = [];
        private var saved:String = "";
        private var running:String = "";

        public function SrpLanguage(content:MovieClip)
        {
            this.content = content;
            var format:TextFormat = textFormat(content);
            label = text(format, LABEL_Y, 24);
            content.addChild(label);
            menu = new DropdownMenuSkinned();
            menu.name = "_srp_language";
            menu.autoSize = "none";
            menu.dropdown = "OptionScrollingList";
            menu.itemRenderer = "OptionsListRenderer";
            menu.menuDirection = "up";
            menu.menuMargin = 0;
            menu.inspectableMenuOffset = {"top": 0, "right": 0, "bottom": 4, "left": 0};
            menu.inspectableMenuPadding = {"top": 5, "right": 1, "bottom": 5, "left": 1};
            menu.menuRowsFixed = false;
            menu.menuWidth = MENU_WIDTH;
            menu.menuWrapping = "normal";
            menu.scrollBar = "";
            menu.focusable = true;
            menu.x = CENTER_X - MENU_WIDTH / 2;
            menu.y = MENU_Y;
            menu.width = MENU_WIDTH;
            // Scaleform leaves a script-created control fully transparent.
            menu.alpha = 1;
            content.addChild(menu);
            note = text(format, NOTE_Y, 40);
            note.wordWrap = true;
            note.multiline = true;
            note.alpha = 0.7;
            content.addChild(note);
            show();
        }

        // On every bind of a tab (create_content): shown on the gameplay
        // frame, where the exe is asked for the list.
        public function place():void
        {
            show();
            if (content.currentFrame == GAMEPLAY_FRAME)
            {
                ask(null);
            }
        }

        // The exe's answer: label, note, saved, running, then name and own
        // name pairs. The saved language is selected.
        public function answer(values:Array):void
        {
            if (values.length < 6)
            {
                return;
            }
            label.text = values[0];
            note.text = values[1];
            saved = values[2];
            running = values[3];
            names = [];
            var labels:Array = [];
            for (var i:int = 4; i + 1 < values.length; i += 2)
            {
                names.push(values[i]);
                labels.push(values[i + 1]);
            }
            menu.dataProvider = new DataProvider(labels);
            menu.menuRowCount = labels.length;
            revert();
            show();
        }

        // The selected language is not the saved one: a change for Apply.
        public function get pending():Boolean
        {
            return menu.selectedIndex >= 0 && menu.selectedIndex < names.length && names[menu.selectedIndex] != saved;
        }

        // Apply: the selected language becomes the saved one (the exe writes
        // it to user.cfg and answers).
        public function save():void
        {
            if (!pending)
            {
                return;
            }
            saved = names[menu.selectedIndex];
            show();
            ask(saved);
        }

        // Cancel: the saved language is selected again.
        public function revert():void
        {
            menu.selectedIndex = names.indexOf(saved);
        }

        private function show():void
        {
            label.visible = menu.visible = content.currentFrame == GAMEPLAY_FRAME && names.length > 0;
            note.visible = menu.visible && saved != running;
        }

        // The tab's option label style, centred: the "Limit upload speed:"
        // stepper's label (the stepper is hidden; this choice took its place).
        // The Lobby choice's note takes it too.
        public static function textFormat(content:MovieClip):TextFormat
        {
            var stepper:OptionStepperCyclic2 = content.getChildByName("_upload_speed") as OptionStepperCyclic2;
            var format:TextFormat = stepper != null && stepper.textField != null
                ? stepper.textField.defaultTextFormat
                : new TextFormat("$CalibriBold", 15, 0x9C9E9D);
            format.align = TextFormatAlign.CENTER;
            return format;
        }

        private static function text(format:TextFormat, y:Number, height:Number):TextField
        {
            var field:TextField = new TextField();
            field.embedFonts = true;
            field.selectable = false;
            field.mouseEnabled = false;
            field.defaultTextFormat = format;
            field.x = CENTER_X - TEXT_WIDTH / 2;
            field.y = y;
            field.width = TEXT_WIDTH;
            field.height = height;
            return field;
        }

        // The exe answers at once (root.srp_language). Outside the game (a
        // test player) there is no exe to call: nothing happens.
        private static function ask(name:String):void
        {
            try
            {
                if (name == null)
                {
                    ExternalInterface.call("srp_language");
                }
                else
                {
                    ExternalInterface.call("srp_language", name);
                }
            }
            catch (error:Error)
            {
            }
        }
    }
}
