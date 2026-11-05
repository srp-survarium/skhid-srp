package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import flash.external.ExternalInterface;
    import flash.text.TextField;
    import scaleform.clik.data.DataProvider;
    import scaleform.clik.utils.Padding;

    // The Graphics tab's "Renderer" list (docs/notes/dxvk-option.md): DirectX 11
    // or Vulkan/DXVK, built as the tab's Monitor list (a list with its label
    // on its left). It is exe patch 83's `srp_dxvk`, kept in user.cfg, which the
    // game reads at its next start. Settings owns it: a pick is a pending
    // change of the tab (Apply saves it, Cancel or leaving without Apply puts
    // the saved one back). The exe answers ExternalInterface.call("srp_dxvk"[,
    // "1" or "0"]) with root.srp_check_option(name, label, saved, running), and the two
    // choices' texts with the HUD group's (root.srp_hud_texts). The list is a
    // script-added child of the tab content, shown on the Graphics frame only;
    // SrpVideoTab places it under Monitor.
    public class SrpRendererChoice
    {
        public static const NAME:String = "srp_dxvk";
        private static const VIDEO_FRAME:int = 3;
        private static const DIRECTX:int = 0;
        private static const DXVK:int = 1;

        public var menu:DropdownMenuSkinnedLabel;
        private var content:MovieClip;
        // The saved choice, "1" (DXVK) or "0"; "" until the exe answers.
        private var saved:String = "";
        // The renderer this process runs, "1" (DXVK loaded at its start) or
        // "0", as the exe answers it.
        private var running:String = "";
        // "Applies after restart." (the Language choice's), under the list
        // while the saved choice is not the running one, in the room
        // SrpVideoTab keeps for it (the rows under it never move).
        private var note:TextField;
        // The note's room under the list.
        public static const NOTE_ROOM:Number = 20;

        public function SrpRendererChoice(content:MovieClip)
        {
            this.content = content;
            // the Monitor list's settings (settingsTabs_15)
            menu = new DropdownMenuSkinnedLabel();
            menu.name = "_srp_renderer";
            menu.autoSize = "none";
            menu.dropdown = "OptionScrollingList";
            menu.itemRenderer = "OptionsListRenderer";
            menu.menuDirection = "down";
            menu.menuMargin = 0;
            // Set directly: CLIK's inspectableMenu* setters only take a value
            // from the Flash editor, so from code they do nothing and the list
            // opened under the label, not under its box.
            menu.menuOffset = new Padding(4, 0, 0, 162);
            menu.menuPadding = new Padding(5, 1, 5, 1);
            menu.menuRowCount = 2;
            menu.menuRowsFixed = false;
            menu.menuWidth = 184;
            menu.menuWrapping = "normal";
            menu.scrollBar = "";
            menu.focusable = true;
            // Scaleform leaves a script-created control fully transparent.
            menu.alpha = 1;
            menu.visible = false;
            content.addChild(menu);
            note = SrpLanguage.text(SrpLanguage.noteFormat(content), 280, NOTE_ROOM);
            note.name = "_srp_renderer_note";
            note.alpha = 0.7;
            note.visible = false;
            content.addChild(note);
        }

        // The two choices' texts and the note's (the exe's root.srp_hud_texts).
        public function texts(directx:String, dxvk:String, restart:String):void
        {
            var index:int = menu.selectedIndex;
            menu.dataProvider = new DataProvider([directx, dxvk]);
            menu.selectedIndex = index;
            note.text = restart;
            show();
        }

        // On every bind of a tab (create_content): on top, shown on the
        // Graphics frame, where the exe is asked for the saved choice and the
        // texts.
        public function place():void
        {
            content.setChildIndex(note, content.numChildren - 1);
            content.setChildIndex(menu, content.numChildren - 1);
            show();
            if (content.currentFrame == VIDEO_FRAME)
            {
                call(NAME, null);
                call("srp_hud_texts", null);
            }
        }

        // At the place and size of the tab's Monitor list, a row under it;
        // the note under it at its label's left edge, in the column's width.
        public function layout(monitor:DisplayObject, y:Number):void
        {
            menu.x = monitor.x;
            menu.y = y;
            menu.width = monitor.width;
            note.x = menu.x + (menu.textFieldLabel != null ? menu.textFieldLabel.x : 0);
            note.y = menu.y + menu.height;
            note.width = monitor.x + monitor.width - note.x;
        }

        // The exe's answer after the name: the label, the saved choice and the
        // running one.
        public function answer(values:Array):void
        {
            if (values.length < 3 || (values[1] != "1" && values[1] != "0") || (values[2] != "1" && values[2] != "0"))
            {
                return;
            }
            menu.label = values[0];
            saved = values[1];
            running = values[2];
            revert();
            show();
        }

        // The choice is not the saved one: a change for Apply.
        public function get pending():Boolean
        {
            return saved != "" && state() != saved;
        }

        // Apply: the choice becomes the saved one (the exe writes it to
        // user.cfg and answers).
        public function save():void
        {
            if (!pending)
            {
                return;
            }
            saved = state();
            show();
            call(NAME, saved);
        }

        // Cancel: the saved choice.
        public function revert():void
        {
            menu.selectedIndex = saved == "1" ? DXVK : DIRECTX;
        }

        private function state():String
        {
            return menu.selectedIndex == DXVK ? "1" : "0";
        }

        private function show():void
        {
            menu.visible = content.currentFrame == VIDEO_FRAME && saved != "" && menu.dataProvider != null
                && menu.dataProvider.length == 2;
            note.visible = menu.visible && saved != running && note.text != "";
        }

        // The exe answers at once. Outside the game (a test player) there is no
        // exe to call: nothing happens.
        private static function call(method:String, state:String):void
        {
            try
            {
                if (state == null)
                {
                    ExternalInterface.call(method);
                }
                else
                {
                    ExternalInterface.call(method, state);
                }
            }
            catch (error:Error)
            {
            }
        }
    }
}
