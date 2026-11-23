package
{
    import flash.display.DisplayObject;
    import flash.display.Loader;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.KeyboardEvent;
    import flash.events.MouseEvent;
    import flash.events.UncaughtErrorEvent;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;
    import flash.text.TextField;
    import flash.ui.Keyboard;

    // The Controls' action selector (survarium/menu/selector/ActionSelector) in the rebuilt main menu, over
    // the Settings, the exe and the Controls window stood in for: the options' real rows with made-up
    // keys (check.sh's data: a language's name, the exe's texts, its rows), opened for a key the way the
    // window opens it. Checks the rows (every offered action once, by group, the fixed chat keys left
    // out), the highlight, Up/Down/Page keys, Enter, Esc (no further than the selector: the Settings stay),
    // a click, Unbind, the search, no match, a free key, the warnings (an action moved from its key, the
    // key's action of the same kind left without one, a lobby action taking nothing from the match),
    // scrolling and the backdrop and the close button; then every language's texts fitting. Prints
    // "SELECTOR ..." lines, "SELECTOR SHOT <name>" for check.sh's screenshots (each state held a while),
    // and "SELECTOR RESULT pass" or "SELECTOR RESULT fail: ...", then "SELECTOR END".
    public class ActionSelectorHarness extends Sprite
    {
        private static const SPOT:uint = 0x22, JUMP:uint = 0x04, SHOP:uint = 0x42, CROUCH_TOGGLE:uint = 0x06;
        private static const CHARACTER:uint = 0x40, SEND_MESSAGE:uint = 0x45, SEND_TO:uint = 0x46;
        private static const HOLD:int = 90;

        private var loader:Loader = new Loader();
        private var frames:int = 0;
        private var languages:Array;
        // What the host was told: each call, ["pick", id], ["unbind"] or ["cancel"].
        private var told:Array = [];
        private var asked:int = 0;
        private var failed:String = null;
        private var step:int = 0;
        private var wait:int = 0;

        public function ActionSelectorHarness()
        {
            loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
            loader.contentLoaderInfo.addEventListener("ioError", function(e:Event):void {
                trace("SELECTOR RESULT fail: " + e);
            });
            loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
                function(e:UncaughtErrorEvent):void { e.preventDefault(); });
            // the game's scene stood in for: a dark backdrop under the menu
            graphics.beginFill(0x2A2F33);
            graphics.drawRect(0, 0, 4000, 3000);
            graphics.endFill();
            addChild(loader);
            loader.load(new URLRequest("main_menu.swf"), new LoaderContext(false, new ApplicationDomain()));
        }

        private function loaded(e:Event):void
        {
            addEventListener(Event.ENTER_FRAME, tick);
        }

        private function require(value:Boolean, message:String):void
        {
            if (!value) throw new Error(message);
        }

        private function get menu():* { return loader.content; }
        private function get selector():* { return menu.key_action_selector; }

        private function cls(name:String):Class
        {
            return loader.contentLoaderInfo.applicationDomain.getDefinition(name) as Class;
        }

        private function language(name:String):Array
        {
            for each (var l:Array in languages) if (l[0] == name) return l;
            throw new Error("no language " + name);
        }

        // The Controls window's call: the key, the rows, and the three answers.
        private function open(lang:Array, key:String):void
        {
            menu.srp_controls_texts.apply(null, lang[1]);
            selector.open(key, rows(lang),
                function(id:uint):void { told.push(["pick", id]); },
                function():void { told.push(["unbind"]); },
                function():void { told.push(["cancel"]); });
        }

        // root.set_keybindings' objects, as the Settings keep them.
        private function rows(lang:Array):Array
        {
            var out:Array = [];
            for each (var r:Array in lang[2])
                out.push({action_id: r[0], label: r[1], group_id: r[2], type: r[3], key: r[4]});
            return out;
        }

        private function key(code:uint):void
        {
            var target:* = menu.stage.focus != null ? menu.stage.focus : menu.stage;
            target.dispatchEvent(new KeyboardEvent(KeyboardEvent.KEY_DOWN, true, false, 0, code));
            target.dispatchEvent(new KeyboardEvent(KeyboardEvent.KEY_UP, true, false, 0, code));
        }

        private function type(text:String):void
        {
            selector.search.text = text;
            selector.search.dispatchEvent(new Event(Event.CHANGE, true));
        }

        private function actionRows():Array
        {
            var ActionRow:Class = cls("survarium.menu.selector.ActionRow");
            return selector.rows.filter(function(r:*, ...rest):Boolean { return r is ActionRow; });
        }

        private function rowOf(id:uint):*
        {
            for each (var r:* in actionRows()) if (r.action.id == id) return r;
            return null;
        }

        private function hover(row:*):void
        {
            row.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_MOVE, true));
        }

        private function warning():String
        {
            var w:TextField = selector.warning;
            return w != null && w.parent != null ? w.text : "";
        }

        private function format(text:String, ...values):String
        {
            for (var i:int = 0; i < values.length; i++) text = text.split("%" + (i + 1) + "$s").join(values[i]);
            return text;
        }

        private function labelOf(lang:Array, id:uint):String
        {
            for each (var r:Array in lang[2]) if (r[0] == id) return r[1];
            return null;
        }

        private function tick(e:Event):void
        {
            ++frames;
            if (wait > 0) { --wait; return; }
            if (frames < 10) return;
            try
            {
                run();
            }
            catch (error:Error)
            {
                failed = error.message;
                trace("SELECTOR RESULT fail: " + error.message + "\n" + error.getStackTrace());
                trace("SELECTOR END");
                removeEventListener(Event.ENTER_FRAME, tick);
            }
        }

        private function shot(name:String):void
        {
            trace("SELECTOR SHOT " + name);
            wait = HOLD;
        }

        private function run():void
        {
            switch (step++)
            {
                case 0:
                    languages = JSON.parse(loaderInfo.parameters.data) as Array;
                    Object(cls("SrpExe")).standIn = function(name:String, value:String):void
                    {
                        if (name == "srp_controls_texts") { ++asked; menu.srp_controls_texts.apply(null, language("english")[1]); }
                    };
                    menu.set_labels([{name: "gameplay_options_type", label: "Gameplay"}, {name: "controllers_options_type", label: "Controls"},
                        {name: "video_options_type", label: "Graphics"}, {name: "sound_options_type", label: "Sound"},
                        {name: "settings_label", label: "Settings"}, {name: "button_apply", label: "Apply"},
                        {name: "button_exit", label: "Exit"}, {name: "button_cancel", label: "Cancel"}]);
                    menu.set_settings(0, []);
                    menu.show_settings(true);
                    // the menu's layout centres the Settings at a stage resize
                    menu.stage.dispatchEvent(new Event(Event.RESIZE));
                    // the first open asks the exe for the texts, once
                    selector.open("Q", rows(language("english")), function(id:uint):void {}, function():void {}, function():void {});
                    require(asked == 1, "the first open asks the exe for its texts: " + asked);
                    selector.close();
                    selector.open("Q", rows(language("english")), function(id:uint):void {}, function():void {}, function():void {});
                    require(asked == 1, "the texts are asked once: " + asked);
                    selector.close();
                    require(!selector.isOpen && !selector.visible, "close() closes it");
                    return;
                case 1:
                    showWarning(language("english"));
                    shot("en");
                    return;
                case 2:
                    behaviour(language("english"));
                    showWarning(language("russian"));
                    shot("ru");
                    return;
                case 3:
                    open(language("english"), "C");
                    type("gren");
                    shot("search");
                    return;
                case 4:
                    selector.close();
                    var overflow:int = 0;
                    for each (var lang:Array in languages) overflow += fits(lang);
                    require(overflow == 0, overflow + " texts do not fit");
                    trace("SELECTOR RESULT pass");
                    trace("SELECTOR END");
                    removeEventListener(Event.ENTER_FRAME, tick);
                    return;
            }
        }

        // Editing Q (Enemy spotted's), Jump highlighted: it moves from Space and spot loses Q.
        private function showWarning(lang:Array):void
        {
            open(lang, "Q");
            // Up from the key's action to Jump, as a player would
            var jump:* = rowOf(JUMP);
            for (var i:int = 0; i < 60 && selector.highlighted != jump; i++) key(Keyboard.UP);
            require(selector.highlighted == jump, lang[0] + ": Up reaches Jump");
            inView(jump);
            var expected:String = format(lang[1][4], labelOf(lang, JUMP), "Space") + " " + format(lang[1][5], labelOf(lang, SPOT));
            require(warning() == expected, lang[0] + ": the warning says " + warning() + ", not " + expected);
        }

        private function behaviour(lang:Array):void
        {
            var ActionRow:Class = cls("survarium.menu.selector.ActionRow");
            var UnbindRow:Class = cls("survarium.menu.selector.UnbindRow");
            var GroupHeading:Class = cls("survarium.menu.selector.GroupHeading");
            var ActionGroup:Object = cls("survarium.constants.ActionGroup");
            selector.close();
            told = [];
            open(lang, "Q");
            var list:Array = selector.rows;
            require(list[0] is UnbindRow, "a bound key's first row is Unbind");
            // every row with a key button once, by group in ActionGroup's order, the table's order in a group
            var ids:Array = [];
            var groups:Array = [];
            for each (var r:* in list)
            {
                if (r is ActionRow) ids.push(r.action.id);
                if (r is GroupHeading) groups.push(r);
            }
            var offered:int = 0;
            for each (var row:Array in lang[2])
            {
                if (row[3] == 0) { require(ids.indexOf(row[0]) == -1, "a fixed key's action is not offered: " + row[0]); continue; }
                require(ids.indexOf(row[0]) == ids.lastIndexOf(row[0]) && ids.indexOf(row[0]) != -1, "offered once: " + row[0]);
                ++offered;
            }
            require(ids.length == offered && offered == 59, "59 actions offered: " + ids.length);
            require(groups.length == 5, "five groups: " + groups.length);
            var last:int = -1;
            for each (r in actionRows())
            {
                require(r.action.group.index >= last, "the groups in order");
                last = r.action.group.index;
            }
            require(ActionGroup.of(0x4D) == ActionGroup.ITEMS && ActionGroup.of(0x37) == ActionGroup.TALK
                && ActionGroup.of(0x26) == ActionGroup.FIGHT && ActionGroup.of(0x0B) == ActionGroup.MOVEMENT
                && ActionGroup.of(0x19) == ActionGroup.INTERFACE, "ActionGroup.of");
            // the key's action highlighted at the open, and in view
            require(selector.highlighted == rowOf(SPOT), "the key's action is highlighted at the open");
            inView(selector.highlighted);
            var now:* = selector.panel.getChildByName("_now");
            require(now != null && now.numChildren == 1 && now.getChildAt(0).text == labelOf(lang, SPOT), "the header names the key's action");
            require(selector.panel.getChildByName("_key") != null, "the header shows the key");
            require(warning() == "", "its own action warns of nothing: " + warning());
            // a lobby action takes nothing from the match: only its move
            hover(rowOf(SHOP));
            require(warning() == format(lang[1][4], labelOf(lang, SHOP), "P"), "Shop only moves: " + warning());
            // Down, Up
            var choices:Array = selector.rows.filter(function(r:*, ...rest):Boolean { return r.choice; });
            hover(rowOf(SPOT));
            var at:int = choices.indexOf(selector.highlighted);
            key(Keyboard.DOWN);
            require(selector.highlighted == choices[at + 1], "Down: the next choice");
            key(Keyboard.UP);
            key(Keyboard.UP);
            require(selector.highlighted == choices[at - 1], "Up: the one before");
            // Page Down to the end, the last row in view
            for (var i:int = 0; i < 12; i++) key(Keyboard.PAGE_DOWN);
            require(selector.highlighted == choices[choices.length - 1], "Page Down ends at the last choice");
            require(selector.scroll > 0, "the list scrolled");
            inView(selector.highlighted);
            for (i = 0; i < 12; i++) key(Keyboard.PAGE_UP);
            require(selector.highlighted == choices[0] && selector.scroll == 0, "Page Up back to Unbind at the top");
            // the wheel
            selector.panel.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_WHEEL, true, false, 0, 0, null, false, false, false, false, -3));
            require(selector.scroll > 0, "the wheel scrolls");
            // Enter picks the highlighted, once, and goes no further (the Settings stay)
            hover(rowOf(JUMP));
            key(Keyboard.ENTER);
            require(!selector.isOpen && told.length == 1 && told[0][0] == "pick" && told[0][1] == JUMP, "Enter picks Jump: " + told);
            require(menu.settings_window.visible, "the Settings stay");
            // Esc cancels, and goes no further
            told = [];
            open(lang, "Q");
            key(Keyboard.ESCAPE);
            require(!selector.isOpen && told.length == 1 && told[0][0] == "cancel", "Esc cancels: " + told);
            require(menu.settings_window.visible, "Esc closes the selector only");
            // a click on a row picks it
            told = [];
            open(lang, "Q");
            rowOf(CROUCH_TOGGLE).dispatchEvent(new MouseEvent(MouseEvent.CLICK, true));
            require(told.length == 1 && told[0][1] == CROUCH_TOGGLE, "a click picks: " + told);
            // Unbind
            told = [];
            open(lang, "Q");
            hover(selector.rows[0]);
            require(warning() == format(lang[1][5], labelOf(lang, SPOT)), "Unbind's warning: " + warning());
            selector.rows[0].dispatchEvent(new MouseEvent(MouseEvent.CLICK, true));
            require(told.length == 1 && told[0][0] == "unbind", "Unbind: " + told);
            // the search
            told = [];
            open(lang, "Q");
            type("gren");
            var found:Array = actionRows();
            require(found.length == 3, "three grenades: " + found.length);
            for each (r in found) require(String(r.action.label).toLowerCase().indexOf("gren") != -1, "a match: " + r.action.label);
            require(!(selector.rows[0] is UnbindRow), "no Unbind in the search's rows");
            require(selector.highlighted == found[0], "the first match highlighted");
            // a key's name finds its action
            type("mouse 1");
            require(actionRows().length == 1 && actionRows()[0].action.id == 0x26, "a key's name: " + actionRows().length);
            type("zzzz");
            require(selector.rows.length == 0 && selector.highlighted == null, "no match: no rows");
            key(Keyboard.ENTER);
            require(selector.isOpen && told.length == 0, "Enter with nothing to pick does nothing");
            type("");
            require(actionRows().length == 59, "the search cleared: every action");
            // the backdrop and the close button cancel
            selector.getChildAt(0).dispatchEvent(new MouseEvent(MouseEvent.CLICK, true));
            require(!selector.isOpen && told.length == 1 && told[0][0] == "cancel", "a click outside cancels: " + told);
            told = [];
            open(lang, "Q");
            selector.panel.getChildByName("_close").dispatchEvent(new MouseEvent(MouseEvent.CLICK, true));
            require(!selector.isOpen && told.length == 1 && told[0][0] == "cancel", "the close button cancels: " + told);
            // a free key: no Unbind, the header says free; a pick of an unbound action warns of nothing
            told = [];
            open(lang, "J");
            require(!(selector.rows[0] is UnbindRow), "a free key has no Unbind");
            now = selector.panel.getChildByName("_now");
            require(now.getChildAt(0).text == lang[1][3], "the header says free: " + now.getChildAt(0).text);
            hover(rowOf(0x4A));
            require(warning() == "", "an unbound action onto a free key warns of nothing: " + warning());
            // a key with a lobby and a match action: both in the header; a match pick takes only the match's
            open(lang, "C");
            now = selector.panel.getChildByName("_now");
            require(now.numChildren == 2, "C's two actions in the header: " + now.numChildren);
            hover(rowOf(JUMP));
            require(warning() == format(lang[1][4], labelOf(lang, JUMP), "Space") + " " + format(lang[1][5], labelOf(lang, CROUCH_TOGGLE)),
                "Jump onto C takes it from crouch toggle only: " + warning());
            selector.close();
            require(told.length == 0, "close() tells nothing: " + told);
            trace("SELECTOR behaviour pass");
        }

        private function inView(row:*):void
        {
            var top:Number = row.y - selector.scroll;
            require(top >= 0 && top + row.rowHeight <= selector.list.mask.height + 0.5, "the row is in view (" + top + ")");
        }

        // Every text of `lang` whole on its place: each row's label, the groups, the title, the hints, the
        // footer's warnings in two lines.
        private function fits(lang:Array):int
        {
            var ActionRow:Class = cls("survarium.menu.selector.ActionRow");
            var GroupHeading:Class = cls("survarium.menu.selector.GroupHeading");
            var bad:int = 0;
            open(lang, "Q");
            require(selector.search.text == "" && actionRows().length == 59, "an open starts with no search");
            for each (var r:* in selector.rows)
            {
                if (r is ActionRow && r.label.text != r.action.label)
                {
                    trace("SELECTOR OVERFLOW " + lang[0] + " row: " + r.action.label + " shows " + r.label.text);
                    ++bad;
                }
                if (r.width > r.rowWidth + 0.5)
                {
                    trace("SELECTOR OVERFLOW " + lang[0] + " row wider than the list: " + r.width);
                    ++bad;
                }
            }
            for (var i:int = 0; i < selector.panel.numChildren; i++)
            {
                var child:DisplayObject = selector.panel.getChildAt(i);
                if (child is TextField && TextField(child).text.indexOf("…") != -1)
                {
                    trace("SELECTOR OVERFLOW " + lang[0] + " cut: " + TextField(child).text);
                    ++bad;
                }
            }
            // the longest warning: Jump moved and spot's key taken, under the footer's preview, in it
            hover(rowOf(JUMP));
            var w:TextField = selector.warning;
            if (w.numLines > 2 || w.y + w.height > selector.hints.y)
            {
                trace("SELECTOR OVERFLOW " + lang[0] + " warning: " + w.numLines + " lines, to " + (w.y + w.height));
                ++bad;
            }
            // the preview's label beside the hints, uncut for every action
            for each (r in actionRows())
            {
                hover(r);
                var pick:* = selector.footer.getChildByName("_pick");
                for (i = 0; i < pick.numChildren; i++)
                {
                    if (pick.getChildAt(i) is TextField && pick.getChildAt(i).text.indexOf("…") != -1)
                    {
                        trace("SELECTOR OVERFLOW " + lang[0] + " footer: " + pick.getChildAt(i).text);
                        ++bad;
                    }
                }
            }
            trace("SELECTOR " + lang[0] + " fits: " + (bad == 0 ? "yes" : bad + " not"));
            selector.close();
            return bad;
        }
    }
}
