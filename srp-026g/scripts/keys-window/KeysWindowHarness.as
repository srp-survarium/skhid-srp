package
{
    import flash.display.DisplayObject;
    import flash.display.DisplayObjectContainer;
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

    // Settings > Controls > Bindings (SrpKeysWindow) in the rebuilt main menu, the exe stood in for (SrpExe.standIn:
    // root.srp_keys from check.sh's binder table and the rows' keys; srp_bind_key giving a key to a row and
    // telling root.set_keybind, a key other match rows had taken from them), with check.sh's rows (the exe's:
    // hide_ui_026g.asm's `controls` in its order, their default keys) and every dictionary's texts:
    // - the Controls tab: its game and chat lists hidden, the lobby's in the right column for reading (no press,
    //   no focus); no Bindings button until the exe's texts are there, then in the left column under the mouse
    //   settings, over Default settings;
    // - the window over the screen at scale 1: the exe's table asked as it opens; every key the rows have drawn
    //   with its rows' actions on it (the keyboard, the mouse, or beside the mouse); the lists: every match row (not
    //   the lobby's or the chat's) once, by kind in the exe's order, each of a kind; nothing over anything else;
    // - a row's press: the stock wait (the menu told with the row, its key box the game's orange), the exe's
    //   answer (root.set_keybind) in the lists and on the keyboard, pending for Apply;
    // - a key's press with a selector: its request (the key, its code and text, its rows, every row, the
    //   kinds), its answer given to the exe (srp_bind_key "action code") and shown; UNBIND taking the key
    //   from its rows (srp_unbind_key "code");
    // - a kind's chip dims the keys and rows of the other kinds; Esc, the close button and a press beside the
    //   window close it, the Settings staying;
    // - every dictionary: the title, the chips, the rows' labels and the keys' texts inside their places.
    // Prints "KEYS ..." lines, "KEYS SHOT <name>" for check.sh's screenshots (each state held a while),
    // "KEYS RESULT pass" or "KEYS RESULT fail: ..." and "KEYS END". No game or server.
    public class KeysWindowHarness extends Sprite
    {
        private static const WINDOW:String = "_srp_keys_window";
        // check.sh's texts: the window's (title, five kinds, three headings), the rows' labels, the binder keys' texts.
        private static const TITLE:int = 0, KINDS:int = 1, HEADINGS:int = 6, LABELS:int = 9;
        private static const CHAT:int = 1, LOBBY:int = 0;

        private var loader:Loader = new Loader();
        private var frames:int = 0;
        private var rows:Array;
        private var texts:Array;
        private var binder:Array;
        // The stand-in exe: each row's key name by action, the dictionary it answers in, its calls.
        private var names:Object;
        private var language:Array;
        private var calls:Array = [];
        private var failed:Boolean = false;
        private var steps:Array;
        private var last:int = 0;
        private var request:Object = null;

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
            steps = [[10, setUp], [10, shot("tab")], [90, tabWithoutTexts], [90, withTexts], [10, shot("tab-button")],
                [90, openWindow], [5, shot("window")], [5, checkWindow], [90, pressRow], [5, shot("binding")],
                [90, answerRow], [3, checkRebound], [1, shot("rebound")], [90, pressKey], [3, checkPicked],
                [10, clearKey], [3, checkCleared],
                [10, focusKind], [5, shot("focus")], [90, unfocus], [5, closings], [5, languages], [5, reopen], [10, end]];
        }

        private function get menu():* { return loader.content; }
        private function get settings():* { return menu.settings_window; }
        private function get content():* { return settings.settings_content; }
        private function get button():* { return content.getChildByName("_srp_keys_button"); }
        private function get window():* { return settings.getChildByName(WINDOW); }
        private function get panel():* { return window.getChildByName(WINDOW + "_panel"); }
        private function get body():* { return panel.getChildByName(WINDOW + "_body"); }
        private function get chips():* { return body.getChildByName(WINDOW + "_chips"); }
        private function windowClass():* { return domainClass("SrpKeysWindow"); }

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

        // The stand-in exe (SrpExe.standIn): srp_keys answers the table; srp_bind_key "action code" gives the key
        // to the row (the match rows that had it lose it), tells root.set_keybind for each row changed, then
        // answers the table.
        private function exe(name:String, value:String):void
        {
            calls.push(name + (value != null ? " " + value : ""));
            if (name == "srp_bind_key")
            {
                var parts:Array = value.split(" ");
                var action:int = int(parts[0]), code:int = int(parts[1]);
                var key:String = null;
                for each (var pair:Array in binder) if (pair[1] == code) key = pair[0];
                if (key == null) return;
                var target:Object = rowOfAction(action);
                for each (var row:Object in rows)
                {
                    if (row != target && row.group_id == target.group_id && names[row.action_id] == key)
                    {
                        names[row.action_id] = "";
                        menu.set_keybind(row.action_id, "", false);
                    }
                }
                names[action] = key;
                menu.set_keybind(action, textOf(key), false);
            }
            if (name == "srp_unbind_key")
            {
                var taken:String = null;
                for each (var held:Array in binder) if (held[1] == int(value)) taken = held[0];
                for each (var holder:Object in rows)
                {
                    if (taken != null && names[holder.action_id] == taken)
                    {
                        names[holder.action_id] = "";
                        menu.set_keybind(holder.action_id, "", false);
                    }
                }
            }
            if (name == "srp_keys" || name == "srp_bind_key" || name == "srp_unbind_key")
            {
                var lines:Array = [];
                for each (var k:Array in binder) lines.push(["K", k[0], k[1], textOf(k[0])].join("\t"));
                for each (var r:Object in rows) lines.push(["A", r.action_id, names[r.action_id]].join("\t"));
                menu.srp_keys(lines.join("\n") + "\n");
            }
        }

        private function textOf(key:String):String
        {
            if (key == "") return "";
            for (var i:int = 0; i < binder.length; i++) if (binder[i][0] == key) return language[LABELS + rows.length + i];
            return key;
        }

        private function rowOfAction(action:int):Object
        {
            for each (var row:Object in rows) if (row.action_id == action) return row;
            return null;
        }

        private function setUp():void
        {
            rows = JSON.parse(loaderInfo.parameters.rows) as Array;
            texts = JSON.parse(loaderInfo.parameters.texts) as Array;
            binder = JSON.parse(loaderInfo.parameters.binder) as Array;
            names = JSON.parse(loaderInfo.parameters.names);
            language = texts[0];
            Object(domainClass("SrpExe")).standIn = exe;
            menu.set_labels([{name: "gameplay_options_type", label: "Gameplay"}, {name: "controllers_options_type", label: "Controls"},
                {name: "video_options_type", label: "Graphics"}, {name: "sound_options_type", label: "Audio"},
                {name: "settings_label", label: "Settings"}, {name: "button_apply", label: "Apply"},
                {name: "button_exit", label: "Exit"}, {name: "button_cancel", label: "Cancel"}, {name: "button_ok", label: "OK"},
                {name: "s_game", label: language[HEADINGS]}, {name: "s_lobby", label: language[HEADINGS + 1]},
                {name: "s_chat_keys", label: language[HEADINGS + 2]},
                {name: "button_default_controls", label: "Default settings"}, {name: "bind_text", label: "Press a key for"}]);
            menu.set_keybindings(rows);
            menu.show_settings(true);
            controlsTab();
            // centred as the menu's layout has them in the game (loaded here, the menu's constructor finds no
            // stage and makes no layout)
            settings.x = Math.round((stage.stageWidth - 650) / 2);
            settings.y = Math.round((stage.stageHeight - 565) / 2);
        }

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
            require(content.currentFrame == 2, "the Controls tab is frame 2: " + content.currentFrame);
            require(button != null && !button.visible, "no Bindings button before the exe's texts");
            trace("KEYS tab: " + rows.length + " rows the exe gives; the tab's lists shown: " + content.keysGame.visible);
        }

        private function withTexts():void
        {
            hudTexts(language);
            var c:* = content;
            var b:* = button;
            require(b.visible && b.alpha == 1 && b.label == language[TITLE], "the button shows the exe's text: " + b.label);
            var defaults:* = c.button_default_controls;
            var invert:* = c._setter_inverse_mouse;
            require(b.x == defaults.x && b.width == defaults.width && b.y + b.height <= defaults.y && b.y > invert.y + 20
                && b.x + b.width <= 297 && defaults.y + defaults.height <= 400,
                "the button in the left column under Invert mouse, over Default settings: " + b.x + "," + b.y + " " + b.width + ", "
                + defaults.y);
            for each (var hidden:* in [c.keysGame, c.keysChat, c.s_chat_keys])
            {
                require(!hidden.visible, hidden.name + " hidden: the match's key bindings are the window's alone");
            }
            // the lobby's keys in the right column, where the game's list was, for reading
            require(c.keysLobby.visible && c.s_lobby.visible && c.keysLobby.x > 297 && c.s_lobby.x > 297
                && c.keysLobby.y == c.s_lobby.y + (c.keysLobby.y - c.s_lobby.y) && c.keysLobby.y > c.s_lobby.y
                && c.keysLobby.dataProvider.length == 5,
                "the lobby's 5 keys under their heading in the right column: " + c.s_lobby.x + "," + c.s_lobby.y + "; "
                + c.keysLobby.x + "," + c.keysLobby.y);
            require(!c.keysLobby.mouseEnabled && !c.keysLobby.mouseChildren && !c.keysLobby.focusable,
                "the lobby's keys take no press nor focus (not bindable)");
            settings.selectIndex(0);
            menu.set_settings(0, []);
            require(!b.visible, "no button on the Gameplay tab");
            controlsTab();
            require(b.visible, "the button again on the Controls tab");
        }

        private function openWindow():void
        {
            calls = [];
            click(button);
            require(window.open && window.visible, "the button opens the window");
            require(calls.indexOf("srp_keys") != -1, "the window asks the exe for its table: " + calls);
        }

        private function children(prefix:String):Array
        {
            var out:Array = [];
            for (var i:int = 0; i < body.numChildren; i++)
            {
                var child:DisplayObject = body.getChildAt(i);
                if (child.name.indexOf(prefix) == 0) out.push(child);
            }
            return out;
        }

        private function texts_of(sprite:DisplayObjectContainer):Array
        {
            var out:Array = [];
            for (var i:int = 0; i < sprite.numChildren; i++)
            {
                var field:TextField = sprite.getChildAt(i) as TextField;
                if (field != null) out.push(field);
            }
            return out;
        }

        private function kindOf(action:int):int
        {
            var kinds:Array = Object(windowClass()).KINDS;
            for (var k:int = 0; k < kinds.length; k++) if (kinds[k].ids.indexOf(action) != -1) return k;
            return -1;
        }

        private function checkWindow():void
        {
            require(panel.scaleX == 1, "the window at its own size (scale 1): " + panel.scaleX);
            var screen:Rectangle = new Rectangle(0, 0, stage.stageWidth, stage.stageHeight);
            var board:Rectangle = panel.getBounds(stage);
            require(screen.containsRect(board), "the window inside the screen: " + board);
            // the lists: every match row (not the lobby's or the chat's), by kind (the exe's order inside)
            var shown:Array = children(WINDOW + "_row_");
            var expected:Array = [];
            var row:Object;
            for (var k:int = 0; k < 5; k++)
            {
                for each (row in rows) if (row.group_id != CHAT && row.group_id != LOBBY && kindOf(row.action_id) == k) expected.push(row);
            }
            require(shown.length == expected.length, "the lists' rows: " + shown.length + " of " + expected.length);
            for (var i:int = 0; i < shown.length; i++)
            {
                require(shown[i].name == WINDOW + "_row_" + expected[i].action_id, "row " + i + ": " + shown[i].name + ", not "
                    + expected[i].label);
            }
            for each (row in rows)
            {
                require(row.group_id == CHAT || row.group_id == LOBBY || kindOf(row.action_id) != -1, "the action "
                    + row.action_id + " (" + row.label + ") has a kind (SrpKeysWindow.KINDS)");
            }
            // every bound row's action on its key
            for each (row in rows)
            {
                if (row.group_id == CHAT || row.group_id == LOBBY || names[row.action_id] == "") continue;
                var key:* = body.getChildByName(WINDOW + "_key_" + names[row.action_id]);
                require(key != null, row.label + "'s key " + names[row.action_id] + " drawn");
                var all:String = texts_of(key).map(function(f:TextField, i:int, a:Array):String { return f.text; }).join("|");
                require(all.indexOf(String(row.label).replace(/\s+$/, "")) != -1, names[row.action_id] + " shows " + row.label + ": " + all);
            }
            checkPlaces("English");
            trace("KEYS window: " + board.width + "x" + board.height + " at " + board.x + "," + board.y + "; "
                + children(WINDOW + "_key_").length + " keys, " + shown.length + " rows");
        }

        // Nothing over anything else: the keys apart, the rows apart, the keys' and the rows' texts inside them.
        private function checkPlaces(language:String):void
        {
            var boxes:Array = [];
            for each (var item:DisplayObject in children(WINDOW + "_key_").concat(children(WINDOW + "_row_")))
            {
                var at:Rectangle = item.getBounds(body);
                at.inflate(-1, -1);
                for each (var other:Rectangle in boxes)
                {
                    var meet:Rectangle = other.intersection(at);
                    require(meet.width < 1 || meet.height < 1, language + ": " + item.name + " over another: " + at + ", " + other
                        + " (" + texts_of(item as DisplayObjectContainer).map(function(f:TextField, i:int, a:Array):String {
                            return f.text + " " + f.getBounds(item); }).join("; ") + ")");
                }
                boxes.push(at);
                var inside:Rectangle = item.getBounds(item);
                inside.inflate(2, 2);
                for each (var field:TextField in texts_of(item as DisplayObjectContainer))
                {
                    var drawn:Rectangle = new Rectangle(field.x, field.y, Math.min(field.width, field.textWidth + 4), Math.min(field.height, field.textHeight + 4));
                    require(inside.containsRect(drawn), language + ": " + item.name + "'s text '" + field.text + "' inside it: "
                        + drawn + " in " + inside);
                }
            }
        }

        // A press of Jump's row: the stock wait.
        private function pressRow():void
        {
            var jump:* = body.getChildByName(WINDOW + "_row_4");
            var heard:Array = [];
            var listener:Function = function(e:Event):void { heard.push(e.target.data); };
            menu.addEventListener("startBindKey", listener, true);
            jump.dispatchEvent(new MouseEvent(MouseEvent.CLICK));
            menu.removeEventListener("startBindKey", listener, true);
            require(heard.length == 1 && heard[0] == rowOfAction(4), "the menu hears the row's start of a binding (its message)");
        }

        // The exe's answer to a key pressed: root.set_keybind, then root.end_keybind; the window asks its table again.
        private function answerRow():void
        {
            names[4] = "kJ";
            menu.set_keybind(4, textOf("kJ"), false);
            menu.end_keybind();
            calls = [];
        }

        private function checkRebound():void
        {
            require(calls.indexOf("srp_keys") != -1, "a key set: the table asked again: " + calls);
            var j:* = body.getChildByName(WINDOW + "_key_kJ");
            require(j != null && texts_of(j).some(function(f:TextField, i:int, a:Array):Boolean { return f.text.indexOf(rowOfAction(4).label) != -1; }),
                "J shows Jump");
            var space:* = body.getChildByName(WINDOW + "_key_kSPACE");
            require(!texts_of(space).some(function(f:TextField, i:int, a:Array):Boolean { return f.text.indexOf(rowOfAction(4).label) != -1; }),
                "Space no longer shows Jump");
            require(settings.btn_accept.enabled, "the change is pending for Apply");
        }

        // A press of the free key Z with a selector: it asks, its answer (Use Medikit, 4Eh) given to the exe.
        private function pressKey():void
        {
            Object(windowClass()).selector = function(asked:Object, answer:Function):void {
                request = asked;
                answer(0x4E);
            };
            calls = [];
            body.getChildByName(WINDOW + "_key_kZ").dispatchEvent(new MouseEvent(MouseEvent.CLICK));
            Object(windowClass()).selector = null;
            require(request != null && request.key == "kZ" && request.code == 0x2C && request.keyName == textOf("kZ")
                && request.actions.length == 0 && request.rows.length == rows.length - 7 && request.kinds.length == 5,
                "the selector asked with Z, its code and text, no rows, the match's rows (not the lobby's or the chat's), the kinds");
            require(calls.indexOf("srp_bind_key 78 44") != -1, "its answer given to the exe: " + calls);
        }

        private function checkPicked():void
        {
            var z:* = body.getChildByName(WINDOW + "_key_kZ");
            require(texts_of(z).some(function(f:TextField, i:int, a:Array):Boolean { return f.text.indexOf(rowOfAction(0x4E).label) != -1; }),
                "Z shows the action the selector picked");
            var row:* = body.getChildByName(WINDOW + "_row_78");
            require(texts_of(row).some(function(f:TextField, i:int, a:Array):Boolean { return f.text == textOf("kZ"); }), "its row shows Z");
        }

        // A press of Z again, the selector answering UNBIND: Z's rows lose it.
        private function clearKey():void
        {
            Object(windowClass()).selector = function(asked:Object, answer:Function):void {
                request = asked;
                answer(Object(windowClass()).UNBIND);
            };
            calls = [];
            body.getChildByName(WINDOW + "_key_kZ").dispatchEvent(new MouseEvent(MouseEvent.CLICK));
            Object(windowClass()).selector = null;
            require(request.actions.length == 1 && request.actions[0] == rowOfAction(0x4E), "the selector asked with Z's row");
            require(calls.indexOf("srp_unbind_key 44") != -1, "UNBIND given to the exe: " + calls);
        }

        private function checkCleared():void
        {
            var z:* = body.getChildByName(WINDOW + "_key_kZ");
            require(!texts_of(z).some(function(f:TextField, i:int, a:Array):Boolean { return f.text.indexOf(rowOfAction(0x4E).label) != -1; }),
                "Z free again");
        }

        private function focusKind():void
        {
            chips.getChildAt(0).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OVER));
            for each (var row:* in children(WINDOW + "_row_"))
            {
                var mine:Boolean = kindOf(int(row.name.substr((WINDOW + "_row_").length))) == 0;
                require(mine ? row.alpha == 1 : row.alpha < 0.5, row.name + (mine ? " shown" : " dimmed") + " over Movement's chip");
            }
            require(body.getChildByName(WINDOW + "_key_kW").alpha == 1 && body.getChildByName(WINDOW + "_key_kR").alpha < 0.5,
                "W (Forward) shown, R (Reload) dimmed");
        }

        private function unfocus():void
        {
            chips.getChildAt(0).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));
            for each (var row:* in children(WINDOW + "_row_")) require(row.alpha == 1, "every row shown again");
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

        // Every dictionary: its texts (the window's, the rows' labels, the keys' texts) in their places.
        private function languages():void
        {
            var english:Array = rows;
            for each (var row:Array in texts)
            {
                language = row;
                hudTexts(row);
                rows = english.map(function(r:Object, i:int, a:Array):Object {
                    var o:Object = {};
                    for (var f:String in r) o[f] = r[f];
                    o.label = row[LABELS + i];
                    o.key = textOf(names[r.action_id]);
                    return o;
                });
                window.rows(rows);
                exe("srp_keys", null);
                var title:TextField = panel.getChildByName(WINDOW + "_title");
                var close:* = panel.getChildByName(WINDOW + "_close");
                require(title.text == row[TITLE] && title.x + title.width < close.x, row[TITLE] + ": the title before the close button");
                var band:Rectangle = chips.getBounds(panel);
                require(band.left >= 0 && band.right <= panel.width, row[TITLE] + ": the chips inside the window");
                checkPlaces(row[TITLE]);
            }
            trace("KEYS every dictionary's title, chips, labels and keys' texts in their places");
            rows = english;
            language = texts[0];
        }

        private function reopen():void
        {
            hudTexts(language);
            window.rows(rows);
            exe("srp_keys", null);
        }
    }
}
