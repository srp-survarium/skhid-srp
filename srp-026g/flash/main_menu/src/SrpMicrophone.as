package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import flash.events.MouseEvent;
    import flash.text.TextField;
    import flash.text.TextFormat;
    import scaleform.clik.data.DataProvider;
    import scaleform.clik.events.ListEvent;

    // The Sound tab's "Microphone" list (exe patch 66, docs/notes/voice-chat.md, "Microphone"; the
    // owner asked for a microphone list): "Default" (Windows' default recording device) first, then each
    // capture device by its name as Windows shows it, which the voice DLL finds. The exe answers
    // ExternalInterface.call("srp_microphone"[, id]) with root.srp_microphone(label, default, shown, id,
    // name, ...): shown is the saved device's id while it is there, else "" (the default). It is asked as
    // the tab is shown and again as the list is opened, so a device plugged in since is there. A pick
    // takes effect at once (the owner, 2026-11-14): ExternalInterface.call("srp_microphone_use", id) has
    // the voice DLL use it, unsaved, so "Test microphone" and push-to-talk open it from their next press.
    // Settings owns it as a change of the tab, as the Language list: a pick is pending, Apply saves it
    // (the call "srp_microphone" with the device's id, "" for the default), Cancel or leaving without
    // Apply puts the saved one back, and "srp_microphone_use" with no id has the DLL use the saved one
    // again. Its label in the sensitivity slider's look, the list under it (Settings.layoutVoice). Script-added
    // children of the tab content, shown on the Sound frame once the exe has answered (without the voice
    // DLL it never does).
    public class SrpMicrophone
    {
        public static const NAME:String = "srp_microphone";
        // The exe's call of the device used now: a pick's id, or none for the saved one.
        private static const USE:String = "srp_microphone_use";
        private static const SOUND_FRAME:int = 4;
        // Its width until it is laid out (the Settings' lists').
        private static const WIDTH:Number = 184;
        // The most rows the open list shows (within the window under it); more scroll by the wheel.
        private static const ROWS:int = 9;

        public var menu:DropdownMenuSkinned;
        private var content:MovieClip;
        private var label:TextField;
        // The ids in the list's order, the default's "" first (none until the exe answers), and the
        // saved one as the list shows it.
        private var ids:Array = [];
        private var saved:String = "";

        public function SrpMicrophone(content:MovieClip)
        {
            this.content = content;
            label = SrpLanguage.text(new TextFormat("$CalibriBold", 15, 0x9C9E9D), WIDTH, 20);
            label.name = "_srp_microphone_label";
            content.addChild(label);
            menu = new DropdownMenuSkinned();
            menu.name = "_" + NAME;
            menu.autoSize = "none";
            menu.dropdown = "OptionScrollingList";
            menu.itemRenderer = "OptionsListRenderer";
            menu.menuDirection = "down";
            menu.menuMargin = 0;
            menu.menuRowsFixed = false;
            menu.menuWidth = WIDTH;
            menu.menuWrapping = "normal";
            menu.scrollBar = "";
            menu.focusable = true;
            menu.width = WIDTH;
            // Scaleform leaves a script-created control fully transparent.
            menu.alpha = 1;
            // Pressed while closed: the exe is asked again before the click opens it.
            menu.addEventListener(MouseEvent.MOUSE_DOWN, onPress);
            // A pick in the open list (CLIK dispatches it only while the list is open, so not for the
            // selections answer and revert make).
            menu.addEventListener(ListEvent.INDEX_CHANGE, onPick);
            content.addChild(menu);
            show();
        }

        // On every bind of a tab (create_content): on top of the tab, the exe asked on the Sound frame.
        public function place():void
        {
            for each (var control:DisplayObject in [label, menu])
            {
                content.setChildIndex(control, content.numChildren - 1);
            }
            show();
            if (content.currentFrame == SOUND_FRAME)
            {
                SrpExe.call(NAME, null);
            }
        }

        // The exe's answer: label, the default's text, the pick it shows, then id and name pairs. A
        // pick not applied yet stays while its device is listed; else the shown one is selected (a pick
        // whose device is gone: the DLL uses the saved one again).
        public function answer(values:Array):void
        {
            if (values.length < 3)
            {
                return;
            }
            var picked:String = pending ? ids[menu.selectedIndex] : null;
            label.text = values[0];
            saved = values[2];
            ids = [""];
            var names:Array = [values[1]];
            for (var i:int = 3; i + 1 < values.length; i += 2)
            {
                ids.push(values[i]);
                names.push(values[i + 1]);
            }
            menu.dataProvider = new DataProvider(names);
            menu.menuRowCount = Math.min(names.length, ROWS);
            var index:int = picked != null ? ids.indexOf(picked) : -1;
            menu.selectedIndex = index >= 0 ? index : Math.max(0, ids.indexOf(saved));
            show();
            if (picked != null && index < 0)
            {
                SrpExe.call(USE, null);
            }
        }

        // The selected device is not the saved one: a change for Apply.
        public function get pending():Boolean
        {
            return menu.selectedIndex >= 0 && menu.selectedIndex < ids.length && ids[menu.selectedIndex] != saved;
        }

        // Apply: the selected device becomes the saved one (the exe writes it to user.cfg, tells the
        // voice DLL and answers).
        public function save():void
        {
            if (!pending)
            {
                return;
            }
            saved = ids[menu.selectedIndex];
            SrpExe.call(NAME, saved);
        }

        // Cancel (or leaving without Apply): the saved device is selected again, and used again (once
        // the exe has answered: before, nothing was picked).
        public function revert():void
        {
            menu.selectedIndex = Math.max(0, ids.indexOf(saved));
            if (ids.length > 0)
            {
                SrpExe.call(USE, null);
            }
        }

        // The label at (x, top) in the look of `style` (the sensitivity slider's label), the list `drop`
        // under the label's top at x, `width` wide. The list's bottom.
        public function layout(style:TextField, x:Number, top:Number, drop:Number, width:Number):Number
        {
            menu.width = menu.menuWidth = Math.round(width);
            SrpLanguage.restyle(label, style);
            label.width = style.width;
            label.height = style.height;
            label.x = x + style.x;
            label.y = top;
            menu.x = x;
            menu.y = Math.round(top + drop);
            return menu.y + menu.height;
        }

        private function onPress(event:MouseEvent):void
        {
            if (!menu.selected && ids.length > 0)
            {
                SrpExe.call(NAME, null);
            }
        }

        // A pick: the voice DLL uses it at once, unsaved; the saved one picked again is the saved one
        // (the exe's, also while its device is unplugged and the list shows the default).
        private function onPick(event:ListEvent):void
        {
            SrpExe.call(USE, pending ? ids[menu.selectedIndex] : null);
        }

        private function show():void
        {
            label.visible = menu.visible = content.currentFrame == SOUND_FRAME && ids.length > 0;
        }
    }
}
