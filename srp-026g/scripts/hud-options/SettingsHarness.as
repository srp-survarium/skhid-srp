package
{
    import flash.display.DisplayObject;
    import flash.display.Loader;
    import flash.display.MovieClip;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.UncaughtErrorEvent;
    import flash.geom.Rectangle;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;
    import flash.text.Font;
    import flash.text.TextField;

    // The Settings' gameplay tab with its "HUD:" group (exe patch 83,
    // docs/notes/hide-hit-markers-scores.md) in the rebuilt main menu: the
    // exe's answers (root.srp_check_option, root.srp_hud_texts), the tab's two
    // columns (Friends and Language; HUD: the crosshair list, Hide hit markers,
    // Hide player scores, Hide quests, Static minimap, the minimap position
    // list), their edges, spacing and fit in every dictionary; Hide quests as
    // the stock quest progress inverted; the crosshair list's Dynamic and
    // Static as the stock static crosshair, Hidden as patch 83's; each pending
    // until Apply, Cancel dropping it; the other tabs; the glyphs. Ends on the
    // gameplay tab, then with Hide player scores checked (Apply enabled), then
    // the crosshair list open, for check.sh's screenshots. Prints "SETTINGS ..." lines and "SETTINGS RESULT
    // pass" or "SETTINGS RESULT fail: ...". No game or server.
    public class SettingsHarness extends Sprite
    {
        private static const NAMES:Array = ["srp_hide_hit_markers", "srp_hide_scores"];

        private static const STOCK_LABELS:Array = ["Invitations from friends only", "Notify when friends are online",
            "Enable message censorship", "Accept private messages from friends only", "Hide chat in match", "Hide spam",
            "", "Static crosshair", "Fix minimap", "", "Show quest progress in match", "Transparency"];

        private var loader:Loader = new Loader();
        private var frames:int = 0;

        public function SettingsHarness()
        {
            loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
            loader.contentLoaderInfo.addEventListener("ioError", function(e:Event):void {
                trace("SETTINGS RESULT fail: " + e);
            });
            // The menu calls the exe (ExternalInterface), which a test player lacks.
            loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
                function(e:UncaughtErrorEvent):void { e.preventDefault(); });
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

        // A click of Apply or Cancel. Outside the game the exe calls after it
        // may throw; what the buttons did before them is what is checked.
        private function click(button:*):void
        {
            var buttonEvent:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
            try
            {
                button.dispatchEvent(new buttonEvent("buttonClick"));
            }
            catch (error:Error)
            {
            }
        }

        private function tick(e:Event):void
        {
            if (++frames < 10) return;
            removeEventListener(Event.ENTER_FRAME, tick);
            try
            {
                check();
                trace("SETTINGS RESULT pass");
            }
            catch (error:Error)
            {
                trace("SETTINGS RESULT fail: " + error.message);
            }
            frames = 0;
            var settings:* = Object(loader.content).settings_window;
            addEventListener(Event.ENTER_FRAME, function(e:Event):void {
                ++frames;
                if (frames == 20)
                {
                    if (settings.btn_accept.enabled) trace("SETTINGS RESULT fail: Apply enabled before the screenshot");
                    trace("SETTINGS SHOT settings");
                }
                if (frames == 80)
                {
                    settings.settings_content.getChildByName("_srp_hide_scores").selected = true;
                    if (!settings.btn_accept.enabled) trace("SETTINGS RESULT fail: the toggle did not enable Apply");
                }
                if (frames == 100) trace("SETTINGS SHOT checked");
                if (frames == 110) settings.settings_content.getChildByName("_srp_crosshair").open();
                if (frames == 130) trace("SETTINGS SHOT open");
            });
        }

        // A visible control's or text's top and bottom in the tab content: a checkbox's box and drawn
        // label (its label's field runs on below), a text's drawn lines, a list's or slider's own height.
        private function span(item:*):Array
        {
            if (item is TextField)
                return [item.y, item.y + TextField(item).textHeight + 4];
            if (item.hasOwnProperty("textField") && item.textField != null && item.hasOwnProperty("selected"))
                return [item.y, item.y + Math.max(item.textField.y + item.textField.textHeight + 4, 26)];
            var bounds:Rectangle = item.getBounds(item.parent);
            return [bounds.top, bounds.bottom];
        }

        // Equal places (x and y are kept in twips, 1/20 px).
        private static function near(a:Number, b:Number):Boolean
        {
            return Math.abs(a - b) < 0.1;
        }

        // The timeline's own places of the gameplay tab, from a new instance of its class (Settings has
        // moved the shown one's).
        private function stockOf(content:MovieClip):Object
        {
            var fresh:* = new (Object(content).constructor as Class)();
            fresh.gotoAndStop(1);
            return {gap: fresh.s_quest_propgress.y - fresh._setter_static_minimap_position.y,
                step: fresh._setter_message.y - fresh._setter_invite.y,
                leftFirst: fresh._setter_invite.y - fresh.s_friends.y,
                rightFirst: fresh._setter_static_cross.y - fresh.s_cross.y,
                dropdown: fresh._setter_static_minimap_position.y - fresh._setter_static_minimap.y,
                dropdownX: fresh._setter_static_minimap_position.x - fresh._setter_static_minimap.x,
                rightHeading: fresh.s_cross.x, rightControl: fresh._setter_static_cross.x};
        }

        // The gameplay tab's two columns: on the left "Friends:", then Language (its note under the list
        // while it shows); on the right "HUD:": the crosshair list, Hide hit markers, Hide player scores,
        // Hide quests (the stock quest progress checkbox), Static minimap (the stock fixed minimap
        // checkbox), the minimap position list. Not shown: the stock "Crosshair:" and "Quest progress:"
        // headings, the static crosshair checkbox, the quest progress transparency slider. Each heading
        // at its column's heading x, the controls at its controls' x (a list one further, as the
        // timeline has them), spaced as the tab's own; nothing overlapping in a column, everything above
        // the window's bottom; every dictionary's texts inside their column.
        private function checkLayout(content:MovieClip, boxes:Array, texts:Array, stock:Object, when:String):void
        {
            var c:* = content;
            var heading:TextField = content.getChildByName("_srp_check_group") as TextField;
            var menu:* = content.getChildByName("_srp_language");
            var cross:* = content.getChildByName("_srp_crosshair");
            var language:TextField = null, note:TextField = null;
            for (var i:int = 0; i < content.numChildren; ++i)
            {
                var t:TextField = content.getChildAt(i) as TextField;
                if (t != null && t.visible && t.text == texts[0][3]) language = t;
                if (t != null && t.text == "Applies after restart.") note = t;
            }
            require(language != null && note != null && cross != null, when + ": the Language heading, its note and the crosshair list there");
            var left:Array = [c.s_friends, c._setter_invite, c._setter_message, c._set_censure, c._only_friends, c._no_in_batle,
                c._hide_spam, language, menu];
            if (note.visible) left.push(note);
            var right:Array = [heading, cross, boxes[0], boxes[1], c._setter_static_show_quests, c._setter_static_minimap,
                c._setter_static_minimap_position];
            var bottom:Number = c.bottomContainer.getBounds(content).bottom;
            for each (var column:Array in [left, right])
            {
                for (i = 0; i < column.length; ++i)
                {
                    require(column[i] != null && column[i].visible, when + ": item " + i + " of a column shown");
                    var here:Array = span(column[i]);
                    require(here[1] <= bottom, when + ": " + column[i].name + " above the window's bottom (" + bottom + "): " + here[1]);
                    if (i + 1 < column.length)
                    {
                        var next:Array = span(column[i + 1]);
                        require(here[1] <= next[0] + 1, when + ": " + column[i].name + " (" + here + ") over " + column[i + 1].name + " (" + next + ")");
                    }
                }
            }
            require(!c.s_cross.visible && !c._setter_static_cross.visible && !c.s_quest_propgress.visible && !c._slider_quest_alpha.visible,
                when + ": no \"Crosshair:\" or \"Quest progress:\" heading, no static crosshair checkbox, no transparency slider");
            // alignment
            require(near(language.x, c.s_friends.x) && near(menu.x, c._setter_invite.x + stock.dropdownX),
                when + ": Language under Friends' edges (its list one further)");
            require(near(heading.x, stock.rightHeading) && near(heading.y, c.s_friends.y), when + ": the HUD heading where Crosshair's was");
            for each (var control:* in [boxes[0], boxes[1], c._setter_static_show_quests, c._setter_static_minimap])
            {
                require(near(control.x, stock.rightControl), when + ": " + control.name + " at the right controls' x");
            }
            require(near(cross.x, stock.rightControl + stock.dropdownX) && near(c._setter_static_minimap_position.x, stock.rightControl + stock.dropdownX),
                when + ": the right column's lists one further");
            // spacing: as the tab's own
            require(near(language.y - c._hide_spam.y, stock.gap) && near(menu.y - language.y, stock.leftFirst),
                when + ": Language the tab's gap under Friends, its list as Friends' first checkbox");
            require(near(cross.y - heading.y, stock.rightFirst), when + ": the crosshair list under the heading as Crosshair's first control was");
            require(near(boxes[0].y - cross.y, stock.step) && near(boxes[1].y - boxes[0].y, stock.step)
                && near(c._setter_static_show_quests.y - boxes[1].y, stock.step) && near(c._setter_static_minimap.y - c._setter_static_show_quests.y, stock.step)
                && near(c._setter_static_minimap_position.y - c._setter_static_minimap.y, stock.dropdown),
                when + ": the HUD rows a step apart, the minimap list as the timeline has it");
            require(!c.bottomContainer.visible, when + ": no bottom panel across the columns");
            // every dictionary's texts inside their columns
            var leftEnd:Number = stock.rightHeading - 10, rightEnd:Number = stock.rightHeading + c.s_friends.width;
            var labelled:Array = [[boxes[0], 0], [boxes[1], 1], [c._setter_static_show_quests, 5], [c._setter_static_minimap, 9]];
            for each (var row:Array in texts)
            {
                for each (var pair:Array in labelled)
                {
                    var shown:String = pair[0].label;
                    pair[0].label = row[pair[1]];
                    pair[0].validateNow();
                    var end:Number = pair[0].x + pair[0].textField.x + pair[0].textField.textWidth;
                    pair[0].label = shown;
                    pair[0].validateNow();
                    require(end < rightEnd, when + ": " + row[pair[1]] + " ends in the right column (" + rightEnd + "): " + end);
                }
                var was:String = heading.text;
                heading.text = row[2];
                require(heading.x + heading.textWidth < rightEnd, when + ": " + row[2] + " ends in the right column");
                heading.text = was;
                was = language.text;
                language.text = row[3];
                require(language.x + language.textWidth < leftEnd, when + ": " + row[3] + " ends in the left column");
                language.text = was;
                if (note.visible)
                {
                    was = note.text;
                    note.text = row[4];
                    require(note.y + note.textHeight + 4 <= bottom, when + ": the note (" + row[4] + ") above the window's bottom: "
                        + (note.y + note.textHeight + 4));
                    require(note.x + note.textWidth < leftEnd, when + ": the note (" + row[4] + ") ends in the left column");
                    note.text = was;
                }
            }
            trace("SETTINGS layout " + when + ": left Friends " + c.s_friends.y + ", Language " + language.y + " (list " + menu.y
                + (note.visible ? ", note " + note.y : "") + ", to " + span(left[left.length - 1])[1] + "); right HUD " + heading.y
                + " (crosshair " + cross.y + ", " + boxes[0].y + ", " + boxes[1].y + ", quests " + c._setter_static_show_quests.y
                + ", minimap " + c._setter_static_minimap.y + ", position " + c._setter_static_minimap_position.y + ", to "
                + span(right[right.length - 1])[1] + "); bottom " + bottom);
        }

        // A player's pick in a list: the list open, the choice, closed (a CLIK dropdown tells its change
        // only while open).
        private static function pick(list:*, index:int):void
        {
            list.open();
            list.selectedIndex = index;
            list.close();
        }

        // The crosshair list's choice and the stock static crosshair checkbox behind it.
        private function crosshair(content:MovieClip):Array
        {
            var cross:* = content.getChildByName("_srp_crosshair");
            return [cross.selectedIndex, Object(content)._setter_static_cross.selected];
        }

        private function check():void
        {
            // [[hit markers' label, scores' label, the group's heading, "Language:", its restart note, "Hide quests",
            //   the crosshair list's three choices, "Static minimap"], ...] for every dictionary
            var texts:Array = JSON.parse(loaderInfo.parameters.texts) as Array;
            var root:* = loader.content;
            var settings:* = root.settings_window;
            var content:MovieClip = settings.settings_content;
            var c:* = content;
            var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
            // the tab's own headings as the exe names them (st_options_friends, ...)
            try
            {
                root.set_labels([{name: "gameplay_options_type", label: "Gameplay"}, {name: "controllers_options_type", label: "Controls"},
                    {name: "settings_label", label: "Settings"}, {name: "button_apply", label: "Apply"},
                    {name: "button_exit", label: "Exit"}, {name: "button_cancel", label: "Cancel"},
                    {name: "s_friends", label: "Friends:"}, {name: "s_cross", label: "Crosshair:"},
                    {name: "s_quest_propgress", label: "Quest progress:"}]);
            }
            catch (error:Error)
            {
                // the tab bar's change calls the exe, which a test player lacks
            }
            // the exe's settings: the stock English labels, each with a setter that records what the
            // menu gives the exe (and gives it back, as the game's do)
            var given:Array = [];
            var settingsList:Array = [];
            var values:Array = [];
            for (var i:int = 0; i < 16; ++i)
            {
                settingsList.push({label: STOCK_LABELS[i] || "option " + i, setter: setterOf(i, given)});
                values.push({id: i, value: 0});
            }
            // the stock defaults: quest progress shown (10), the transparency at 1 (11); the crosshair static (7)
            values[10].value = 1;
            values[11].value = 1;
            values[7].value = 1;
            // the minimap position list's choices (st_minimap_off, _bottom, _top), the lower left corner
            settingsList[9].dataProvider = ["Minimap off", "Lower left corner", "Upper left corner"];
            values[9].value = 1;
            root.set_settings(0, settingsList);
            root.set_values(0, values);
            root.srp_language(texts[0][3], "Applies after restart.", "english", "english", "english", "English",
                "ukrainian", "Українська");
            root.show_settings(true);

            var boxes:Array = [];
            for each (var name:String in NAMES)
            {
                var box:* = content.getChildByName("_" + name);
                require(box != null && !box.visible, name + ": the checkbox waits for the exe's answer");
                boxes.push(box);
            }
            var cross:* = content.getChildByName("_srp_crosshair");
            require(cross != null && !cross.visible, "the crosshair list waits for the exe's answers");
            for (i = 0; i < NAMES.length; ++i)
            {
                root.srp_check_option(NAMES[i], texts[0][i], "0");
                require(boxes[i].visible && boxes[i].alpha == 1, NAMES[i] + ": on the gameplay tab");
                require(boxes[i].label == texts[0][i], NAMES[i] + ": its label is the exe's: " + boxes[i].label);
                require(!boxes[i].selected, NAMES[i] + ": the saved state (off) is shown");
            }
            root.srp_check_option("srp_hide_crosshair", texts[0][8], "0");
            require(!cross.visible, "the crosshair list waits for its texts");
            root.srp_hud_texts(texts[0][2], texts[0][5], texts[0][6], texts[0][7], texts[0][8], texts[0][9]);
            require(!settings.btn_accept.enabled, "nothing to apply after the answers");
            var heading:TextField = content.getChildByName("_srp_check_group") as TextField;
            require(heading != null && heading.visible && heading.text == texts[0][2], "the group's heading is the exe's: " + (heading ? heading.text : null));
            require(cross.visible && cross.dataProvider.length == 3 && cross.dataProvider[0] == texts[0][6]
                && cross.dataProvider[1] == texts[0][7] && cross.dataProvider[2] == texts[0][8], "the crosshair list: the three choices");
            require(c._setter_static_show_quests.label == texts[0][5] && c._setter_static_minimap.label == texts[0][9],
                "the stock quest progress and minimap checkboxes labelled Hide quests and Static minimap");
            root.srp_check_option("srp_no_such", "x", "1");
            require(!boxes[0].selected && !boxes[1].selected && !settings.btn_accept.enabled, "another name changes nothing");

            checkLayout(content, boxes, texts, stockOf(content), "at rest");
            // the restart note shown (a saved language not the running one)
            root.srp_language(texts[0][3], "Applies after restart.", "ukrainian", "english", "english", "English",
                "ukrainian", "Українська");
            checkLayout(content, boxes, texts, stockOf(content), "the restart note shown");
            root.srp_language(texts[0][3], "Applies after restart.", "english", "english", "english", "English",
                "ukrainian", "Українська");
            checkLayout(content, boxes, texts, stockOf(content), "the note hidden again");
            var invite:DisplayObject = content["_setter_invite"];
            for each (box in boxes)
            {
                require(Math.abs(box.width - invite.width) < 1 && Math.abs(box.height - invite.height) < 1,
                    box.name + ": the tab's checkbox at its size: " + box.width + "x" + box.height + " (" + invite.width + "x" + invite.height + ")");
            }
            // every dictionary's three choices inside the list
            var choices:Object = cross.dataProvider;
            for each (var row:Array in texts)
            {
                cross.dataProvider = new (domain.getDefinition("scaleform.clik.data.DataProvider") as Class)([row[6], row[7], row[8]]);
                for (i = 0; i < 3; ++i)
                {
                    cross.selectedIndex = i;
                    cross.validateNow();
                    // the field is counter-scaled: its width is in the list's units, its text in the screen's
                    require(cross.textField.textWidth + 4 < cross.textField.width * cross.scaleX, row[6 + i] + " fits the crosshair list ("
                        + cross.textField.textWidth + " of " + cross.textField.width * cross.scaleX + ")");
                }
            }
            root.srp_hud_texts(texts[0][2], texts[0][5], texts[0][6], texts[0][7], texts[0][8], texts[0][9]);
            root.set_values(0, values);
            root.srp_check_option("srp_hide_crosshair", texts[0][8], "0");
            require(!settings.btn_accept.enabled, "nothing pending after the texts' check");

            // Hide quests: the stock quest progress inverted
            var quests:* = c._setter_static_show_quests;
            require(!quests.selected, "the stock default (quest progress shown) is Hide quests unchecked");
            values[10].value = 0;
            root.set_values(0, values);
            require(quests.selected && !settings.btn_accept.enabled, "quest progress off (0) is Hide quests checked");
            values[10].value = 1;
            root.set_values(0, values);
            require(!quests.selected, "and on (1) unchecked again");
            given.length = 0;
            quests.selected = true;
            require(given.length == 1 && given[0][0] == 10 && given[0][1] == false && quests.selected && settings.btn_accept.enabled,
                "checking Hide quests gives the exe quest progress off (" + given + "), Apply enabled");
            quests.selected = false;
            require(given[given.length - 1][1] == true && !settings.btn_accept.enabled, "unchecking it gives it on again, nothing pending");
            quests.selected = true;
            click(settings.btn_cancel);
            root.set_values(0, values);
            require(!quests.selected && !settings.btn_accept.enabled, "Cancel: the stock value (shown) back, Hide quests unchecked");
            quests.selected = true;
            click(settings.btn_accept);
            require(quests.selected && !settings.btn_accept.enabled, "Apply keeps Hide quests checked");
            values[10].value = 0;
            root.set_values(0, values);
            require(quests.selected && !settings.btn_accept.enabled, "the exe's value after Apply (off) is Hide quests checked");
            trace("SETTINGS Hide quests: the stock quest progress inverted through the setter, Apply and Cancel");

            // the crosshair list: Dynamic and Static are the stock checkbox, Hidden is patch 83's
            var stockBox:* = c._setter_static_cross;
            require(crosshair(content).join() == "1,true", "the stock static crosshair is Static: " + crosshair(content));
            values[7].value = 0;
            root.set_values(0, values);
            require(crosshair(content).join() == "0,false" && !settings.btn_accept.enabled, "the stock dynamic crosshair is Dynamic");
            values[7].value = 1;
            root.set_values(0, values);
            given.length = 0;
            pick(cross, 0);
            require(crosshair(content).join() == "0,false" && given.length == 1 && given[0].join() == "7,false" && settings.btn_accept.enabled,
                "picking Dynamic unchecks the stock checkbox, given to the exe (" + given + "), Apply enabled");
            click(settings.btn_cancel);
            root.set_values(0, values);
            require(crosshair(content).join() == "1,true" && !settings.btn_accept.enabled, "Cancel: Static back");
            given.length = 0;
            pick(cross, 2);
            require(crosshair(content).join() == "2,true" && given.length == 0 && settings.btn_accept.enabled,
                "picking Hidden leaves the stock checkbox (Static), Apply enabled");
            click(settings.btn_accept);
            require(crosshair(content).join() == "2,true" && !settings.btn_accept.enabled, "Apply saves Hidden");
            root.srp_check_option("srp_hide_crosshair", texts[0][8], "1");
            root.set_values(0, values);
            require(crosshair(content).join() == "2,true" && !settings.btn_accept.enabled, "the exe's answers keep Hidden");
            pick(cross, 0);
            require(crosshair(content).join() == "0,false" && settings.btn_accept.enabled, "from Hidden, Dynamic: the stock checkbox unchecked, pending");
            click(settings.btn_cancel);
            root.set_values(0, values);
            require(crosshair(content).join() == "2,true" && !settings.btn_accept.enabled, "Cancel: Hidden back, the stock value Static kept");
            pick(cross, 1);
            require(crosshair(content).join() == "1,true" && settings.btn_accept.enabled, "from Hidden, Static: pending (Hidden dropped)");
            click(settings.btn_accept);
            root.srp_check_option("srp_hide_crosshair", texts[0][8], "0");
            require(crosshair(content).join() == "1,true" && !settings.btn_accept.enabled, "Apply: Static, Hidden off");
            values[7].value = 0;
            root.set_values(0, values);
            root.srp_check_option("srp_hide_crosshair", texts[0][8], "1");
            require(crosshair(content).join() == "2,false", "saved Hidden over a dynamic stock crosshair: Hidden");
            root.srp_check_option("srp_hide_crosshair", texts[0][8], "0");
            require(crosshair(content).join() == "0,false" && !settings.btn_accept.enabled, "Hidden off again: back to Dynamic, the stock value it kept");
            values[7].value = 1;
            root.set_values(0, values);
            // a transient: it opens on top and a press outside closes it
            cross.open();
            require(cross.selected && content.stage.getObjectsUnderPoint(new (domain.getDefinition("flash.geom.Point") as Class)(cross.localToGlobal(new (domain.getDefinition("flash.geom.Point") as Class)(5, cross.height + 15)).x, cross.localToGlobal(new (domain.getDefinition("flash.geom.Point") as Class)(5, cross.height + 15)).y)).length > 0, "the crosshair list opens (its list under it)");
            stage.dispatchEvent(new (domain.getDefinition("flash.events.MouseEvent") as Class)("mouseDown", true, false, 5, 5));
            if (cross.selected)
            {
                content.stage.dispatchEvent(new (domain.getDefinition("flash.events.MouseEvent") as Class)("mouseDown", true, false, 5, 5));
            }
            require(!cross.selected, "a press outside closes the crosshair list");
            trace("SETTINGS crosshair list: Dynamic/Static the stock checkbox, Hidden patch 83's, Apply and Cancel, closes outside");

            for (i = 0; i < boxes.length; ++i)
            {
                box = boxes[i];
                var other:* = boxes[1 - i];
                box.selected = true;
                require(settings.btn_accept.enabled && settings.btn_cancel.label == "Cancel", NAMES[i] + ": a toggle enables Apply and Cancel");
                box.selected = false;
                require(!settings.btn_accept.enabled && settings.btn_cancel.label == "Exit", NAMES[i] + ": toggling back: nothing to apply");
                box.selected = true;
                click(settings.btn_cancel);
                require(!box.selected && !settings.btn_accept.enabled, NAMES[i] + ": Cancel puts the saved state back");
                box.selected = true;
                click(settings.btn_accept);
                require(box.selected && !settings.btn_accept.enabled && !other.selected, NAMES[i] + ": Apply saves the toggle, the other unchanged");
                box.selected = false;
                require(settings.btn_accept.enabled, NAMES[i] + ": unchecking a saved on is a change");
                click(settings.btn_cancel);
                require(box.selected, NAMES[i] + ": Cancel: the saved on again");
                root.srp_check_option(NAMES[i], texts[0][i], "0");
                require(!box.selected && !settings.btn_accept.enabled, NAMES[i] + ": the exe's answer sets the saved state");
            }
            trace("SETTINGS a toggle is pending until Apply; Cancel drops it; each box on its own");

            values[10].value = 1;
            content.gotoAndStop(2);
            root.set_settings(0, settingsList);
            root.set_values(0, values);
            require(!boxes[0].visible && !boxes[1].visible && !cross.visible, "not on another tab");
            content.gotoAndStop(1);
            root.set_settings(0, settingsList);
            root.set_values(0, values);
            require(boxes[0].visible && boxes[1].visible && cross.visible, "back on the gameplay tab");
            require(c._setter_static_show_quests.label == texts[0][5] && !c._setter_static_show_quests.selected
                && crosshair(content).join() == "1,true", "the new tab content: Hide quests and the crosshair list as before");
            var panelIndex:int = content.getChildIndex(content.bottomContainer);
            require(content.getChildIndex(boxes[0]) > panelIndex && content.getChildIndex(cross) > panelIndex,
                "the HUD controls over the bottom panel after a tab change");
            checkLayout(content, boxes, texts, stockOf(content), "after a tab change");
            trace("SETTINGS tabs pass");

            var fontName:String = boxes[0].textField.getTextFormat().font;
            var font:Font = null;
            for each (var fontClass:String in ["CalibriBold", "FontTB", "Calibri", "FontT"])
            {
                if (!domain.hasDefinition(fontClass)) continue;
                var candidate:Font = new (domain.getDefinition(fontClass) as Class)() as Font;
                if (candidate.fontName == fontName) font = candidate;
            }
            if (font == null)
            {
                trace("SETTINGS the label's font " + fontName + " is not a class of the menu: glyphs not checked");
                return;
            }
            var missing:String = "";
            for each (row in texts)
            {
                for each (var text:String in row)
                {
                    for (i = 0; i < text.length; ++i)
                    {
                        var letter:String = text.charAt(i);
                        if (letter != " " && !font.hasGlyphs(letter) && missing.indexOf(letter) < 0) missing += letter;
                    }
                }
            }
            require(missing == "", fontName + " lacks " + missing);
            trace("SETTINGS " + fontName + " has the glyphs of every dictionary's texts");
        }

        // The exe's setter of option `i`: records [i, value] and gives the value back.
        private static function setterOf(i:int, given:Array):Function
        {
            return function(value:*):* {
                given.push([i, value]);
                return value;
            };
        }
    }
}
