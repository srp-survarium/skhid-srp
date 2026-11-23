package
{
    import flash.display.DisplayObject;
    import flash.display.Loader;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.events.UncaughtErrorEvent;
    import flash.geom.Rectangle;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;
    import flash.text.TextField;

    // The Settings' Controls tab and its "Key bindings" window (SrpKeysWindow) in the rebuilt main menu, with
    // check.sh's rows (the exe's: hide_ui_026g.asm's `controls` in its order, their default keys) and every
    // dictionary's texts: no button until the exe's texts are there, then at the foot of the left column
    // beside Default settings; the window over the screen, inside it with its margin: every row once, in
    // its group (the match's, the lobby's, the chat's) in the exe's order, each of a kind; whole lists (no
    // scrolling) side by side, none over another, each key box showing its key; a press of a row's key box
    // is the stock rebinding (start_bind_key, the menu told to show its "press a key" message), the exe's
    // answer (root.set_keybind) shown in the window and on the tab, pending for Apply; a kind's chip
    // dimming the other kinds' rows; Esc, the close button and a press beside the window closing it, the
    // Settings staying; every dictionary's title and chips in the title's band, its labels on their rows.
    // Prints "KEYS ..." lines, "KEYS SHOT <name>" for check.sh's screenshots (each state held a second),
    // "KEYS RESULT pass" or "KEYS RESULT fail: ..." and "KEYS END". No game or server.
    public class KeysWindowHarness extends Sprite
    {
        private static const WINDOW:String = "_srp_keys_window";
        // check.sh's texts: the window's (title, five kinds, three headings), then the rows' labels.
        private static const TITLE:int = 0, KINDS:int = 1, HEADINGS:int = 6, LABELS:int = 9;

        private var loader:Loader = new Loader();
        private var frames:int = 0;
        private var rows:Array;
        private var texts:Array;
        private var failed:Boolean = false;
        // The steps: [frames to wait before it, function].
        private var steps:Array;
        private var last:int = 0;

        public function KeysWindowHarness()
        {
            loader.contentLoaderInfo.addEventListener(Event.INIT, function(e:Event):void {
                addEventListener(Event.ENTER_FRAME, tick);
            });
            loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
                function(e:UncaughtErrorEvent):void { e.preventDefault(); });
            // what the menu stands over in the game (the lobby's scene): a plain dark ground
            graphics.beginFill(0x2C2E30);
            graphics.drawRect(0, 0, 4096, 4096);
            graphics.endFill();
            addChild(loader);
            loader.load(new URLRequest("main_menu.swf"), new LoaderContext(false, new ApplicationDomain()));
            steps = [[10, setUp], [10, shot("tab")], [10, tabWithoutTexts], [30, withTexts], [10, shot("tab-button")],
                [30, openWindow], [5, checkWindow], [1, shot("window")], [30, pressKey], [5, shot("binding")],
                [30, answerKey], [5, shot("rebound")], [30, focusKind], [5, shot("focus")], [30, unfocus],
                [5, closings], [5, languages], [5, reopen], [10, end]];
        }

        private function get menu():* { return loader.content; }
        private function get settings():* { return menu.settings_window; }
        private function get content():* { return settings.settings_content; }
        private function get button():* { return content.getChildByName("_srp_keys_button"); }
        private function get window():* { return settings.getChildByName(WINDOW); }
        private function get panel():* { return window.getChildByName(WINDOW + "_panel"); }
        private function get body():* { return panel.getChildByName(WINDOW + "_body"); }
        private function get chips():* { return panel.getChildByName(WINDOW + "_chips"); }

        private function domainClass(name:String):Class
        {
            return loader.contentLoaderInfo.applicationDomain.getDefinition(name) as Class;
        }

        private function require(value:Boolean, message:String):void
        {
            if (!value) throw new Error(message);
        }

        private function click(target:*):void
        {
            target.dispatchEvent(new (domainClass("scaleform.clik.events.ButtonEvent"))("buttonClick"));
        }

        private function shot(name:String):Function
        {
            return function():void { trace("KEYS SHOT " + name); };
        }

        private function tick(e:Event):void
        {
            ++frames;
            if (steps.length == 0 || frames < last + steps[0][0]) return;
            var step:Array = steps.shift();
            last = frames;
            try
            {
                step[1]();
            }
            catch (error:Error)
            {
                failed = true;
                trace("KEYS RESULT fail: " + error.message);
                steps = [[1, end]];
            }
        }

        private function end():void
        {
            if (!failed) trace("KEYS RESULT pass");
            trace("KEYS END");
        }

        // The Settings as the exe opens them on the Controls tab (labels, rows, the tab's settings), without
        // the HUD group's texts yet.
        private function setUp():void
        {
            rows = JSON.parse(loaderInfo.parameters.rows) as Array;
            texts = JSON.parse(loaderInfo.parameters.texts) as Array;
            var english:Array = texts[0];
            menu.set_labels([{name: "gameplay_options_type", label: "Gameplay"}, {name: "controllers_options_type", label: "Controls"},
                {name: "video_options_type", label: "Graphics"}, {name: "sound_options_type", label: "Audio"},
                {name: "settings_label", label: "Settings"}, {name: "button_apply", label: "Apply"},
                {name: "button_exit", label: "Exit"}, {name: "button_cancel", label: "Cancel"}, {name: "button_ok", label: "OK"},
                {name: "s_game", label: english[HEADINGS]}, {name: "s_lobby", label: english[HEADINGS + 1]},
                {name: "s_chat_keys", label: english[HEADINGS + 2]},
                {name: "button_default_controls", label: "Default settings"}, {name: "bind_text", label: "Press a key for"}]);
            menu.set_keybindings(rows);
            menu.show_settings(true);
            controlsTab();
            // centred as the menu's layout has them in the game (loaded here, the menu's constructor finds no
            // stage and makes no layout)
            settings.x = Math.round((stage.stageWidth - 650) / 2);
            settings.y = Math.round((stage.stageHeight - 565) / 2);
        }

        // The Controls tab as the exe binds it.
        private function controlsTab():void
        {
            settings.selectIndex(1);
            menu.set_settings(1, [{label: "Invert mouse"}, {label: "Mouse sensitivity", dataProvider: {minimum: 0.01, maximum: 10, snapInterval: 0.01}},
                {label: "Default settings"}, {label: ""}, {label: ""}, {label: ""}]);
            menu.set_values(1, [{id: 0, value: 0}, {id: 1, value: 1.25}]);
            menu.srp_check_option("srp_mouse_vertical_locked", "Lock vertical sensitivity", "1");
            menu.srp_slider_option("srp_mouse_vertical", "Vertical sensitivity", "100");
        }

        // root.srp_hud_texts with a dictionary's texts of the window (the HUD group's own left empty).
        private function hudTexts(row:Array):void
        {
            var values:Array = ["HUD:"];
            while (values.length < 15) values.push("");
            menu.srp_hud_texts.apply(null, values.concat(row.slice(TITLE, KINDS + 5)));
        }

        private function tabWithoutTexts():void
        {
            var c:* = content;
            require(c.currentFrame == 2, "the Controls tab is frame 2: " + c.currentFrame);
            require(button != null && !button.visible, "no Key bindings button before the exe's texts");
            trace("KEYS tab: the game's list shows " + c.keysGame.height / c.keysGame.rowHeight + " of its " + c.keysGame.dataProvider.length
                + " rows; " + rows.length + " rows in all");
        }

        private function withTexts():void
        {
            hudTexts(texts[0]);
            var c:* = content;
            var b:* = button;
            require(b.visible && b.alpha == 1 && b.label == texts[0][TITLE], "the button shows the exe's text: " + b.label);
            var defaults:* = c.button_default_controls;
            require(b.y == defaults.y && b.width == defaults.width && b.x + b.width < defaults.x,
                "the button beside Default settings, in the left column: " + b.x + "," + b.y + " " + b.width);
            require(c.keysLobby.y + c.keysLobby.height <= b.y, "the button under the lobby's keys: " + (c.keysLobby.y + c.keysLobby.height)
                + ", " + b.y);
            settings.selectIndex(0);
            menu.set_settings(0, []);
            require(!b.visible, "no button on the Gameplay tab");
            controlsTab();
            require(b.visible, "the button again on the Controls tab");
        }

        private function openWindow():void
        {
            click(button);
            require(window.open && window.visible, "the button opens the window");
        }

        // The window's lists, in their order.
        private function windowLists():Array
        {
            var out:Array = [];
            for (var i:int = 0; i < body.numChildren; i++)
            {
                var child:* = body.getChildAt(i);
                if (child.hasOwnProperty("dataProvider")) out.push(child);
            }
            return out;
        }

        private function checkWindow():void
        {
            var lists:Array = windowLists();
            var shown:Array = [];
            var list:*;
            var i:int;
            for each (list in lists)
            {
                for (i = 0; i < list.dataProvider.length; i++) shown.push(list.dataProvider[i]);
            }
            // the groups in the window's order (the match's, the lobby's, the chat's), each in the exe's order
            var expected:Array = [];
            var row:Object;
            for each (var group:Function in [function(r:Object):Boolean { return r.group_id != 0 && r.group_id != 1; },
                function(r:Object):Boolean { return r.group_id == 0; }, function(r:Object):Boolean { return r.group_id == 1; }])
            {
                for each (row in rows) if (group(row)) expected.push(row);
            }
            require(shown.length == rows.length, "every row in the window: " + shown.length + " of " + rows.length);
            for (var k:int = 0; k < shown.length; k++)
            {
                require(shown[k] === expected[k], "row " + k + ": " + shown[k].label + ", not " + expected[k].label);
            }
            for each (row in rows)
            {
                require(kindOf(row.action_id) != -1, "the action " + row.action_id + " (" + row.label + ") has a kind (SrpKeysWindow.KINDS)");
            }
            var screen:Rectangle = new Rectangle(0, 0, stage.stageWidth, stage.stageHeight);
            screen.inflate(-20, -20);
            var board:Rectangle = panel.getBounds(stage);
            require(screen.containsRect(board), "the window inside the screen: " + board + " in " + screen);
            var boxes:Array = [];
            for each (list in lists)
            {
                list.validateNow();
                require(Math.abs(list.height - list.dataProvider.length * list.rowHeight) < 0.1, list.name + " is whole rows, no scrolling");
                var at:Rectangle = list.getBounds(stage);
                at = new Rectangle(at.x, at.y, list.width * panel.scaleX, list.height * panel.scaleY);
                require(board.containsRect(at), list.name + " inside the window: " + at + " in " + board);
                for each (var other:Rectangle in boxes)
                {
                    var meet:Rectangle = other.intersection(at);
                    require(meet.width < 1 || meet.height < 1, list.name + " over another list: " + at + ", " + other);
                }
                boxes.push(at);
                for (i = 0; i < list.dataProvider.length; i++)
                {
                    var r:* = list.getRendererAt(i);
                    require(r != null && r.data === list.dataProvider[i], list.name + ": row " + i + " drawn");
                    require(r.btn_key.textField.text == String(r.data.key), list.name + ": " + r.data.label + "'s key box shows "
                        + r.btn_key.textField.text + ", not " + r.data.key);
                    require(r.textField.textWidth + 4 <= r.textField.width + 0.5, r.data.label + " fits its row");
                }
            }
            var title:TextField = panel.getChildByName(WINDOW + "_title");
            require(title.text == String(texts[0][TITLE]).toUpperCase(), "the title is the exe's text: " + title.text);
            trace("KEYS window: " + lists.length + " lists, " + board.width + "x" + board.height + " at " + board.x + "," + board.y
                + " (scale " + panel.scaleX + "); every row shown at once");
        }

        // The kind of an action (SrpKeysWindow.KINDS' index), or -1.
        private function kindOf(action:int):int
        {
            var kinds:Array = Object(domainClass("SrpKeysWindow")).KINDS;
            for (var k:int = 0; k < kinds.length; k++)
            {
                if (kinds[k].ids.indexOf(action) != -1) return k;
            }
            return -1;
        }

        // The window's renderer of the action's row.
        private function rowOf(action:int):*
        {
            for each (var list:* in windowLists())
            {
                for (var i:int = 0; i < list.dataProvider.length; i++)
                {
                    if (list.dataProvider[i].action_id == action) return list.getRendererAt(i);
                }
            }
            return null;
        }

        // A press of Jump's key box: the stock row asks the exe for a key (start_bind_key) and tells the menu,
        // which shows its "press a key" message (MainMenu.onStartKeybind, over everything: its messages' layer
        // is the stage's top). Loaded by this harness the menu's constructor finds no stage and makes no
        // message layer, so the message itself is not shown here: the menu hearing the row is what is checked.
        private function pressKey():void
        {
            var jump:* = rowOf(4);
            var heard:Array = [];
            var listener:Function = function(e:Event):void { heard.push(e.target); };
            menu.addEventListener("startBindKey", listener, true);
            jump.btn_key.dispatchEvent(new MouseEvent(MouseEvent.CLICK));
            menu.removeEventListener("startBindKey", listener, true);
            require(jump.btn_key.currentLabel == "down_k", "a press of Jump's key box waits for a key: " + jump.btn_key.currentLabel);
            require(heard.length == 1 && heard[0] == jump, "the menu hears the row's start of a binding (its message)");
        }

        // The exe's answer to a key pressed (root.set_keybind, then root.end_keybind).
        private function answerKey():void
        {
            menu.set_keybind(4, "J", false);
            menu.end_keybind();
            var jump:* = rowOf(4);
            jump.validateNow();
            jump.btn_key.validateNow();
            require(jump.data.key == "J" && jump.btn_key.label == "J", "the window shows the exe's key: " + jump.btn_key.label);
            var tab:Object = null;
            for (var i:int = 0; i < content.keysGame.dataProvider.length; i++)
            {
                if (content.keysGame.dataProvider[i].action_id == 4) tab = content.keysGame.dataProvider[i];
            }
            require(tab != null && tab.key == "J", "the tab's row has it too");
            require(settings.btn_accept.enabled, "the change is pending for Apply");
        }

        private function focusKind():void
        {
            chips.getChildAt(0).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
            for each (var list:* in windowLists())
            {
                for (var i:int = 0; i < list.dataProvider.length; i++)
                {
                    var r:* = list.getRendererAt(i);
                    var mine:Boolean = kindOf(r.data.action_id) == 0;
                    require(mine ? r.alpha == 1 : r.alpha < 0.5, r.data.label + (mine ? " shown" : " dimmed") + " over Movement's chip");
                }
            }
        }

        private function unfocus():void
        {
            chips.getChildAt(0).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
            for each (var list:* in windowLists())
            {
                for (var i:int = 0; i < list.dataProvider.length; i++)
                {
                    require(list.getRendererAt(i).alpha == 1, "every row shown again");
                }
            }
        }

        // Esc, the close button, a press beside the window: each closes it, the Settings staying.
        private function closings():void
        {
            // Esc: MainMenu.onKeyDownK asks the Settings to close a panel first, the Settings themselves after
            require(settings.closePanel() && !window.open && settings.visible, "Esc closes the window, the Settings stay");
            require(!settings.closePanel(), "a second Esc is the Settings'");
            click(button);
            click(panel.getChildByName(WINDOW + "_close"));
            require(!window.open && settings.visible, "the close button closes it");
            click(button);
            window.getChildAt(0).dispatchEvent(new MouseEvent(MouseEvent.CLICK));
            require(!window.open && settings.visible, "a press beside it closes it");
            click(button);
            panel.dispatchEvent(new MouseEvent(MouseEvent.CLICK, true));
            require(window.open, "a press on the window keeps it");
        }

        // Every dictionary: the title and the chips on the title's band, apart; every row's label on its row.
        private function languages():void
        {
            var worst:String = "";
            var worstSize:int = 99;
            for each (var row:Array in texts)
            {
                hudTexts(row);
                window.rows(rows.map(function(r:Object, i:int, a:Array):Object {
                    var o:Object = {};
                    for (var f:String in r) o[f] = r[f];
                    o.label = row[LABELS + i];
                    return o;
                }));
                window.dispatchEvent(new Event(Event.ENTER_FRAME));
                var title:TextField = panel.getChildByName(WINDOW + "_title");
                var close:* = panel.getChildByName(WINDOW + "_close");
                require(title.x + title.width + 16 <= chips.x && chips.x + chips.width <= close.x,
                    row[TITLE] + ": the title, the chips and the close button apart: " + title.width + ", " + chips.x + "-" + (chips.x + chips.width));
                for each (var list:* in windowLists())
                {
                    for (var i:int = 0; i < list.dataProvider.length; i++)
                    {
                        var r:* = list.getRendererAt(i);
                        require(r.textField.text == r.data.label, r.data.label + " drawn: " + r.textField.text);
                        require(r.textField.textWidth + 4 <= r.textField.width + 0.5, r.data.label + " fits its row ("
                            + r.textField.textWidth + " of " + r.textField.width + ")");
                        var size:int = int(r.textField.getTextFormat().size);
                        if (size < worstSize)
                        {
                            worstSize = size;
                            worst = r.data.label;
                        }
                    }
                }
            }
            trace("KEYS every dictionary's title, chips and labels fit; the smallest label " + worstSize + " pt (" + worst + ")");
        }

        // The English texts and rows again, for the last state.
        private function reopen():void
        {
            hudTexts(texts[0]);
            window.rows(rows);
        }
    }
}
