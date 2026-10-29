package
{
    import flash.display.MovieClip;
    import flash.external.ExternalInterface;
    import flash.text.TextField;
    import flash.text.TextFormat;
    import scaleform.clik.data.DataProvider;
    import scaleform.clik.events.ListEvent;
    import survarium.menu.Settings;

    // The gameplay tab's Language choice (exe patch 79,
    // docs/notes/language-option.md). The exe keeps the choice in user.cfg
    // and answers ExternalInterface.call("srp_language"[, name]) with
    // root.srp_language(label, note, saved, running, name, own name, ...):
    // every registered language, by its own name. A pick is saved at once;
    // the note shows while the saved language is not the running one.
    // create_content calls apply first, on every tab: the controls live on
    // the gameplay frame only (script-added children survive frame changes).
    public class SrpLanguage
    {
        private static const GAMEPLAY_FRAME:int = 1;
        // Right of the seed settings, in the bottom panel.
        private static const X:Number = 420;
        private static const LABEL_Y:Number = 280;
        private static const MENU_Y:Number = 306;
        private static const NOTE_Y:Number = 340;
        private static const WIDTH:Number = 184;

        private static var heading:TextField;
        private static var menu:DropdownMenuSkinned;
        private static var note:TextField;
        // The resource names in the menu's order, the saved and the running one.
        private static var names:Array = [];
        private static var saved:String = "";
        private static var running:String = "";

        public static function apply(settings:Settings):void
        {
            var content:MovieClip = settings.settings_content;
            if (content == null)
            {
                return;
            }
            make(content);
            var shown:Boolean = content.currentFrame == GAMEPLAY_FRAME && names.length > 0;
            heading.visible = menu.visible = shown;
            note.visible = shown && saved != running;
            if (content.currentFrame == GAMEPLAY_FRAME)
            {
                ask(null);
            }
        }

        // The exe's answer: label, note, saved, running, then name and own name pairs.
        public static function answer(settings:Settings, values:Array):void
        {
            var content:MovieClip = settings.settings_content;
            if (content == null || values.length < 6)
            {
                return;
            }
            make(content);
            heading.text = values[0];
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
            menu.selectedIndex = names.indexOf(saved);
            var shown:Boolean = content.currentFrame == GAMEPLAY_FRAME;
            heading.visible = menu.visible = shown;
            note.visible = shown && saved != running;
        }

        private static function make(content:MovieClip):void
        {
            if (menu != null && menu.parent == content)
            {
                return;
            }
            var format:TextFormat = labelFormat(content);
            heading = text(format, X, LABEL_Y, WIDTH, 22);
            content.addChild(heading);
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
            menu.menuWidth = WIDTH;
            menu.menuWrapping = "normal";
            menu.scrollBar = "";
            menu.focusable = true;
            menu.x = X;
            menu.y = MENU_Y;
            menu.width = WIDTH;
            // Scaleform leaves a script-created control fully transparent.
            menu.alpha = 1;
            menu.addEventListener(ListEvent.INDEX_CHANGE, picked);
            content.addChild(menu);
            note = text(format, X, NOTE_Y, WIDTH, 40);
            note.wordWrap = true;
            note.multiline = true;
            note.alpha = 0.7;
            content.addChild(note);
            heading.visible = menu.visible = note.visible = false;
        }

        // The heading style of the gameplay tab (its "Quest progress").
        private static function labelFormat(content:MovieClip):TextFormat
        {
            var model:TextField = content.getChildByName("s_quest_propgress") as TextField;
            return model != null ? model.defaultTextFormat : new TextFormat("$CalibriBold", 14, 0xFFFFFF);
        }

        private static function text(format:TextFormat, x:Number, y:Number, width:Number, height:Number):TextField
        {
            var field:TextField = new TextField();
            field.embedFonts = true;
            field.selectable = false;
            field.mouseEnabled = false;
            field.defaultTextFormat = format;
            field.x = x;
            field.y = y;
            field.width = width;
            field.height = height;
            return field;
        }

        private static function picked(event:ListEvent):void
        {
            var name:String = names[menu.selectedIndex];
            if (name != null && name != saved)
            {
                ask(name);
            }
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
