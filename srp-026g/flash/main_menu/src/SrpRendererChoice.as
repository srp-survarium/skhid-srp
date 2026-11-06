package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import flash.text.TextField;
    import scaleform.clik.data.DataProvider;
    import scaleform.clik.events.ListEvent;
    import scaleform.clik.utils.Padding;

    // The Graphics tab's "Renderer" list (docs/notes/dxvk-option.md): DirectX 11
    // or Vulkan/DXVK, built as the tab's Monitor list (a list with its label
    // on its left), and the "Vulkan options..." button under it (SrpVulkanOptions),
    // greyed while the list shows DirectX 11. The list is exe patch 83's
    // `srp_dxvk`, kept in user.cfg, which the game reads at its next start, with
    // the Vulkan options. Settings owns them as one change of the tab: a pick or
    // an option changed is pending (Apply saves it, Cancel or leaving without
    // Apply puts the saved one back). The exe answers
    // ExternalInterface.call("srp_dxvk"[, "1" or "0"]) with
    // root.srp_check_option(name, label, saved, running), and the two choices'
    // texts with the HUD group's (root.srp_hud_texts). Script-added children of
    // the tab content, shown on the Graphics frame only; SrpVideoTab places them
    // under Monitor.
    public class SrpRendererChoice
    {
        public static const NAME:String = "srp_dxvk";
        private static const VIDEO_FRAME:int = 3;
        private static const DIRECTX:int = 0;
        private static const DXVK:int = 1;

        public var menu:DropdownMenuSkinnedLabel;
        public var vulkan:SrpVulkanOptions;
        private var content:MovieClip;
        // The saved choice, "1" (DXVK) or "0"; "" until the exe answers.
        private var saved:String = "";
        // The renderer this process runs, "1" (DXVK loaded at its start) or
        // "0", as the exe answers it.
        private var running:String = "";
        // "Applies after restart." (the Language choice's), under the Vulkan
        // options' button while a saved value (the choice or an option) is not the
        // one this process runs, in the room SrpVideoTab keeps for it (the rows
        // under it never move).
        private var note:TextField;
        // The note's room under the rows.
        public static const NOTE_ROOM:Number = 20;

        public function SrpRendererChoice(content:MovieClip)
        {
            this.content = content;
            menu = new DropdownMenuSkinnedLabel();
            menu.name = "_srp_renderer";
            style(menu);
            menu.menuRowCount = 2;
            // Scaleform leaves a script-created control fully transparent.
            menu.alpha = 1;
            menu.visible = false;
            content.addChild(menu);
            // the button is disabled and enabled again as the list's pick changes, before Apply
            menu.addEventListener(ListEvent.INDEX_CHANGE, onPick);
            vulkan = new SrpVulkanOptions(content);
            note = SrpLanguage.text(SrpLanguage.noteFormat(content), 280, NOTE_ROOM);
            note.name = "_srp_renderer_note";
            note.alpha = 0.7;
            note.visible = false;
            content.addChild(note);
        }

        // Where a list's open list starts from its left edge: under its box, at the Monitor list's width.
        public static const MENU_LEFT:Number = 162;

        // The Monitor list's settings (settingsTabs_15), for a list built as it is.
        public static function style(list:DropdownMenuSkinnedLabel):void
        {
            list.autoSize = "none";
            list.dropdown = "OptionScrollingList";
            list.itemRenderer = "OptionsListRenderer";
            list.menuDirection = "down";
            list.menuMargin = 0;
            // Set directly: CLIK's inspectableMenu* setters only take a value
            // from the Flash editor, so from code they do nothing and the list
            // opened under the label, not under its box.
            list.menuOffset = new Padding(4, 0, 0, MENU_LEFT);
            list.menuPadding = new Padding(5, 1, 5, 1);
            list.menuRowsFixed = false;
            list.menuWidth = 184;
            list.menuWrapping = "normal";
            list.scrollBar = "";
            list.focusable = true;
        }

        // A change of the list or of an option, for Settings (pending).
        public function listen(handler:Function):void
        {
            menu.addEventListener(ListEvent.INDEX_CHANGE, handler);
            vulkan.listen(handler);
        }

        // The texts of the root.srp_hud_texts: the two choices, the note, the
        // Vulkan options' button.
        public function texts(directx:String, dxvk:String, restart:String, options:String):void
        {
            var index:int = menu.selectedIndex;
            menu.dataProvider = new DataProvider([directx, dxvk]);
            menu.selectedIndex = index;
            note.text = restart;
            vulkan.texts(options);
            show();
        }

        // On every bind of a tab (create_content): on top, shown on the
        // Graphics frame, where the exe is asked for the saved choice, the
        // options and the texts.
        public function place():void
        {
            content.setChildIndex(note, content.numChildren - 1);
            content.setChildIndex(vulkan.button, content.numChildren - 1);
            content.setChildIndex(menu, content.numChildren - 1);
            show();
            if (content.currentFrame == VIDEO_FRAME)
            {
                SrpExe.call(NAME, null);
                vulkan.ask();
                SrpExe.call("srp_hud_texts", null);
            }
        }

        // At the place and size of the tab's Monitor list, a row under it; the
        // Vulkan options' button a row under that, the panel's rows on one step
        // over the column's `rows`, the button at the preset buttons' width (`preset`); the note
        // under them at the labels' left edge, in the column's width.
        public function layout(monitor:DisplayObject, y:Number, step:Number, rows:Array, preset:DisplayObject):void
        {
            menu.x = monitor.x;
            menu.y = y;
            menu.width = monitor.width;
            vulkan.layout(menu, y + step, step, rows, preset);
            note.x = menu.x + (menu.textFieldLabel != null ? menu.textFieldLabel.x : 0);
            note.y = y + step + menu.height;
            note.width = monitor.x + monitor.width - note.x;
        }

        // The exe's answer to the list or an option: the name, then the label,
        // the saved value and the running one (an option's choices after).
        public function answer(values:Array):void
        {
            if (values[0] != NAME)
            {
                vulkan.answer(values);
                show();
                return;
            }
            if (values.length < 4 || (values[2] != "1" && values[2] != "0") || (values[3] != "1" && values[3] != "0"))
            {
                return;
            }
            menu.label = values[1];
            saved = values[2];
            running = values[3];
            menu.selectedIndex = saved == "1" ? DXVK : DIRECTX;
            show();
        }

        // The answer's name is the list's or an option's.
        public static function named(name:String):Boolean
        {
            return name == NAME || SrpVulkanOptions.named(name);
        }

        // The choice is not the saved one, or an option is not: a change for Apply.
        public function get pending():Boolean
        {
            return saved != "" && state() != saved || vulkan.pending;
        }

        // Apply: the choice and the options become the saved ones (the exe
        // writes them to user.cfg and answers).
        public function save():void
        {
            vulkan.save();
            if (saved != "" && state() != saved)
            {
                saved = state();
                SrpExe.call(NAME, saved);
            }
            show();
        }

        // Cancel: the saved choice and options.
        public function revert():void
        {
            menu.selectedIndex = saved == "1" ? DXVK : DIRECTX;
            vulkan.revert();
            show();
        }

        // The Vulkan options' panel closed (Esc, a tab change, the window
        // hidden); whether it was open.
        public function close():Boolean
        {
            return vulkan.close();
        }

        private function onPick(event:ListEvent):void
        {
            show();
        }

        private function state():String
        {
            return menu.selectedIndex == DXVK ? "1" : "0";
        }

        private function show():void
        {
            menu.visible = content.currentFrame == VIDEO_FRAME && saved != "" && menu.dataProvider != null
                && menu.dataProvider.length == 2;
            vulkan.show(menu.visible, menu.selectedIndex == DXVK);
            note.visible = menu.visible && (saved != running || vulkan.restart) && note.text != "";
        }
    }
}
