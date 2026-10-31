package
{
    import flash.display.MovieClip;
    import flash.events.Event;
    import flash.external.ExternalInterface;
    import scaleform.clik.controls.Button;
    import scaleform.clik.data.DataProvider;

    // The "HUD:" group's crosshair list (docs/notes/hide-hit-markers-scores.md):
    // Dynamic crosshair, Static crosshair, Hidden crosshair. Dynamic and Static
    // are the stock static crosshair checkbox (`_setter_static_cross`, hidden
    // from the tab; unchecked Dynamic, checked Static): a pick of either sets
    // it, which goes to the exe as the tab's own change. Hidden is exe patch
    // 83's `srp_hide_crosshair`, kept in user.cfg, which leaves the stock value
    // as it is, so Dynamic or Static comes back as it was. Settings owns it:
    // Hidden picked or dropped is a pending change of the tab (Apply saves it,
    // Cancel or leaving without Apply puts the saved one back). The exe answers
    // ExternalInterface.call("srp_hide_crosshair"[, "1" or "0"]) with
    // root.srp_check_option(name, label, saved). The list is a script-added
    // child of the tab content, shown on the gameplay frame only.
    public class SrpCrosshairChoice
    {
        public static const NAME:String = "srp_hide_crosshair";
        private static const GAMEPLAY_FRAME:int = 1;
        private static const DYNAMIC:int = 0;
        private static const STATIC:int = 1;
        private static const HIDDEN:int = 2;
        // The tab's own lists' width (the minimap position's).
        private static const MENU_WIDTH:Number = 184;

        public var menu:DropdownMenuSkinned;
        private var content:MovieClip;
        // The stock checkbox of the tab content now shown (a tab change makes it anew).
        private var stock:Button;
        // The saved Hidden, "1" or "0"; "" until the exe answers.
        private var saved:String = "";

        public function SrpCrosshairChoice(content:MovieClip)
        {
            this.content = content;
            menu = new DropdownMenuSkinned();
            menu.name = "_srp_crosshair";
            menu.autoSize = "none";
            menu.dropdown = "OptionScrollingList";
            menu.itemRenderer = "OptionsListRenderer";
            menu.menuDirection = "down";
            menu.menuMargin = 0;
            menu.inspectableMenuOffset = {"top": 4, "right": 0, "bottom": 0, "left": 0};
            menu.inspectableMenuPadding = {"top": 5, "right": 1, "bottom": 5, "left": 1};
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

        // The three choices' texts (the exe's root.srp_hud_texts).
        public function texts(dynamicText:String, staticText:String, hiddenText:String):void
        {
            menu.dataProvider = new DataProvider([dynamicText, staticText, hiddenText]);
            menu.menuRowCount = 3;
            show();
        }

        // On every bind of a tab (create_content), after the tab's settings are
        // bound: the stock checkbox of this frame followed, the list on top,
        // shown on the gameplay frame, where the exe is asked for Hidden.
        public function place():void
        {
            var box:Button = content.getChildByName("_setter_static_cross") as Button;
            if (box != stock)
            {
                if (stock != null)
                {
                    stock.removeEventListener(Event.SELECT, onStock);
                }
                stock = box;
                if (stock != null)
                {
                    stock.addEventListener(Event.SELECT, onStock);
                    stock.visible = false;
                }
            }
            content.setChildIndex(menu, content.numChildren - 1);
            show();
            if (content.currentFrame == GAMEPLAY_FRAME)
            {
                ask(null);
            }
        }

        // The exe's answer after the name: the label and the saved Hidden.
        public function answer(values:Array):void
        {
            if (values.length < 2 || (values[1] != "1" && values[1] != "0"))
            {
                return;
            }
            saved = values[1];
            revert();
            show();
        }

        // A pick in the list: Dynamic or Static sets the stock checkbox (the
        // tab's own change); Hidden leaves it.
        public function pick():void
        {
            if (stock != null && menu.selectedIndex != HIDDEN && menu.selectedIndex >= 0)
            {
                stock.selected = menu.selectedIndex == STATIC;
            }
        }

        // Hidden is not the saved one: a change for Apply.
        public function get pending():Boolean
        {
            return saved != "" && state() != saved;
        }

        // Apply: Hidden as picked becomes the saved one (the exe writes it to
        // user.cfg and answers).
        public function save():void
        {
            if (!pending)
            {
                return;
            }
            saved = state();
            ask(saved);
        }

        // Cancel: the saved Hidden, else the stock checkbox's state.
        public function revert():void
        {
            menu.selectedIndex = saved == "1" ? HIDDEN : stockChoice();
        }

        private function stockChoice():int
        {
            return stock != null && stock.selected ? STATIC : DYNAMIC;
        }

        // The stock checkbox changed (the exe's values bound, a Cancel): the
        // list follows it unless Hidden is picked.
        private function onStock(event:Event):void
        {
            if (menu.selectedIndex != HIDDEN)
            {
                menu.selectedIndex = stockChoice();
            }
        }

        private function state():String
        {
            return menu.selectedIndex == HIDDEN ? "1" : "0";
        }

        private function show():void
        {
            menu.visible = content.currentFrame == GAMEPLAY_FRAME && saved != "" && menu.dataProvider != null
                && menu.dataProvider.length == 3;
        }

        // The exe answers at once (root.srp_check_option). Outside the game (a
        // test player) there is no exe to call: nothing happens.
        private static function ask(state:String):void
        {
            try
            {
                if (state == null)
                {
                    ExternalInterface.call(NAME);
                }
                else
                {
                    ExternalInterface.call(NAME, state);
                }
            }
            catch (error:Error)
            {
            }
        }
    }
}
