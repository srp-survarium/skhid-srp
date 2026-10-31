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

    // The Settings' "Hide hit markers" and "Hide player scores" checkboxes
    // (exe patch 83, docs/notes/hide-hit-markers-scores.md) in the rebuilt main
    // menu: the exe's answers as root.srp_check_option gives them, their place
    // in the bottom panel (right of the Language list, one under the other,
    // every dictionary's label inside the panel), a toggle of each as a
    // pending change that Apply saves and Cancel drops, the other tabs, and
    // the label font's glyphs for every dictionary's text (check.sh passes
    // them). Ends on the gameplay tab, unchecked, then one checked (Apply
    // enabled), for check.sh's screenshots. Prints "SETTINGS ..." lines and
    // "SETTINGS RESULT pass" or "SETTINGS RESULT fail: ...". No game or server.
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

        // The gameplay tab's two columns: on the left "Friends:", "Crosshair:" (the static crosshair),
        // Language; on the right "HUD:": Hide hit markers, Hide player scores, the quest progress
        // checkbox and its slider, the fixed minimap checkbox and the minimap position list, no "Quest
        // progress:" heading. Each heading at its column's heading x, the controls at its controls' x
        // (a list one further, as the timeline has them), spaced as the tab's own; nothing overlapping
        // in a column, everything above the window's bottom; every dictionary's texts inside their
        // column. `stock` is the timeline's places before anything moved.
        private function checkLayout(content:MovieClip, boxes:Array, texts:Array, stock:Object, when:String):void
        {
            var c:* = content;
            var heading:TextField = content.getChildByName("_srp_check_group") as TextField;
            var menu:* = content.getChildByName("_srp_language");
            var language:TextField = null, note:TextField = null;
            for (var i:int = 0; i < content.numChildren; ++i)
            {
                var t:TextField = content.getChildAt(i) as TextField;
                if (t != null && t.visible && t.text == texts[0][3]) language = t;
                if (t != null && t.text == "Applies after restart.") note = t;
            }
            require(language != null && note != null, when + ": the Language heading and note there");
            var left:Array = [c.s_friends, c._setter_invite, c._setter_message, c._set_censure, c._only_friends, c._no_in_batle,
                c._hide_spam, c.s_cross, c._setter_static_cross, language, menu];
            if (note.visible) left.push(note);
            var right:Array = [heading, boxes[0], boxes[1], c._setter_static_show_quests, c._slider_quest_alpha,
                c._setter_static_minimap, c._setter_static_minimap_position];
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
            require(!c.s_quest_propgress.visible, when + ": no \"Quest progress:\" heading");
            // alignment
            require(near(c.s_cross.x, c.s_friends.x) && near(language.x, c.s_friends.x), when + ": the left headings at Friends' x");
            require(near(c._setter_static_cross.x, c._setter_invite.x) && near(menu.x, c._setter_invite.x + stock.dropdownX),
                when + ": the left controls at the friends' x (a list one further)");
            require(near(heading.x, stock.rightHeading) && near(heading.y, c.s_friends.y), when + ": the HUD heading where Crosshair's was");
            for each (var control:* in [boxes[0], boxes[1], c._setter_static_show_quests, c._slider_quest_alpha, c._setter_static_minimap])
            {
                require(near(control.x, stock.rightControl), when + ": " + control.name + " at the right controls' x");
            }
            require(near(c._setter_static_minimap_position.x, stock.rightControl + stock.dropdownX), when + ": the minimap list one further");
            // spacing: as the tab's own
            require(near(c.s_cross.y - c._hide_spam.y, stock.gap) && near(language.y - c._setter_static_cross.y, stock.gap),
                when + ": the tab's gap before Crosshair and Language");
            require(near(c._setter_static_cross.y - c.s_cross.y, stock.rightFirst) && near(menu.y - language.y, stock.rightFirst)
                && near(boxes[0].y - heading.y, stock.rightFirst), when + ": each group's first control as Crosshair's was");
            require(near(boxes[1].y - boxes[0].y, stock.step) && near(c._setter_static_show_quests.y - boxes[1].y, stock.step),
                when + ": the HUD checkboxes a step apart");
            require(near(c._slider_quest_alpha.y - c._setter_static_show_quests.y, stock.slider)
                && near(c._setter_static_minimap.y - c._slider_quest_alpha.y, stock.step)
                && near(c._setter_static_minimap_position.y - c._setter_static_minimap.y, stock.dropdown),
                when + ": the slider and the minimap list at their stock distances");
            require(!c.bottomContainer.visible, when + ": no bottom panel across the columns");
            // every dictionary's texts inside their columns
            var leftEnd:Number = stock.rightHeading - 10, rightEnd:Number = stock.rightHeading + c.s_friends.width;
            for each (var row:Array in texts)
            {
                for (i = 0; i < 2; ++i)
                {
                    var shown:String = boxes[i].label;
                    boxes[i].label = row[i];
                    boxes[i].validateNow();
                    var end:Number = boxes[i].x + boxes[i].textField.x + boxes[i].textField.textWidth;
                    boxes[i].label = shown;
                    boxes[i].validateNow();
                    require(end < rightEnd, when + ": " + row[i] + " ends in the right column (" + rightEnd + "): " + end);
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
                    note.text = was;
                }
            }
            trace("SETTINGS layout " + when + ": left Friends " + c.s_friends.y + ", Crosshair " + c.s_cross.y + ", Language "
                + language.y + " (list " + menu.y + (note.visible ? ", note " + note.y : "") + "); right HUD " + heading.y + " ("
                + boxes[0].y + ", " + boxes[1].y + ", quests " + c._setter_static_show_quests.y + ", slider " + c._slider_quest_alpha.y
                + ", minimap " + c._setter_static_minimap.y + ", position " + c._setter_static_minimap_position.y + ", to "
                + span(c._setter_static_minimap_position)[1] + "); bottom " + bottom);
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
                rightFirst: fresh._setter_static_cross.y - fresh.s_cross.y,
                slider: fresh._slider_quest_alpha.y - fresh._setter_static_show_quests.y,
                dropdown: fresh._setter_static_minimap_position.y - fresh._setter_static_minimap.y,
                dropdownX: fresh._setter_static_minimap_position.x - fresh._setter_static_minimap.x,
                rightHeading: fresh.s_cross.x, rightControl: fresh._setter_static_cross.x};
        }

        private function check():void
        {
            // [[hit markers' label, scores' label, the group's heading, "Language:", its restart note], ...] for every dictionary
            var texts:Array = JSON.parse(loaderInfo.parameters.texts) as Array;
            var root:* = loader.content;
            var settings:* = root.settings_window;
            var content:MovieClip = settings.settings_content;
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
            var settingsList:Array = [];
            var values:Array = [];
            for (var i:int = 0; i < 16; ++i)
            {
                // the stock English texts (the exe's st_*_option keys), for the screenshots
                settingsList.push({label: STOCK_LABELS[i] || "option " + i});
                // every option's value, as the exe binds them (the quests' alpha slider, 11, at 1)
                values.push({id: i, value: i == 11 ? 1 : 0});
            }
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
            for (i = 0; i < NAMES.length; ++i)
            {
                root.srp_check_option(NAMES[i], texts[0][i], "0", texts[0][2]);
                require(boxes[i].visible && boxes[i].alpha == 1, NAMES[i] + ": on the gameplay tab");
                require(boxes[i].label == texts[0][i], NAMES[i] + ": its label is the exe's: " + boxes[i].label);
                require(!boxes[i].selected, NAMES[i] + ": the saved state (off) is shown");
            }
            require(!settings.btn_accept.enabled, "nothing to apply after the answers");
            var heading:TextField = content.getChildByName("_srp_check_group") as TextField;
            require(heading != null && heading.visible && heading.text == texts[0][2], "the group's heading is the exe's: " + (heading ? heading.text : null));
            root.srp_check_option("srp_no_such", "x", "1", "y");
            require(!boxes[0].selected && !boxes[1].selected && !settings.btn_accept.enabled, "another name changes nothing");

            checkLayout(content, boxes, texts, stockOf(content), "at rest");
            // the restart note shown (a saved language not the running one): the groups under it move down
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
                root.srp_check_option(NAMES[i], texts[0][i], "0", texts[0][2]);
                require(!box.selected && !settings.btn_accept.enabled, NAMES[i] + ": the exe's answer sets the saved state");
            }
            boxes[0].selected = true;
            boxes[1].selected = true;
            require(settings.btn_accept.enabled, "both toggled: Apply enabled");
            boxes[0].selected = false;
            require(settings.btn_accept.enabled, "one toggled back: the other still pending");
            boxes[1].selected = false;
            require(!settings.btn_accept.enabled, "both back: nothing to apply");
            trace("SETTINGS a toggle is pending until Apply; Cancel drops it; each box on its own");

            content.gotoAndStop(2);
            root.set_settings(0, settingsList);
            root.set_values(0, values);
            require(!boxes[0].visible && !boxes[1].visible, "not on another tab");
            content.gotoAndStop(1);
            root.set_settings(0, settingsList);
            root.set_values(0, values);
            require(boxes[0].visible && boxes[1].visible, "back on the gameplay tab");
            // The tab's timeline children came back over the script-added ones; the bind puts the
            // boxes and the Language controls on top again (the panel's translucent rect would dim
            // and cover them).
            var panelIndex:int = content.getChildIndex(content.bottomContainer);
            require(content.getChildIndex(boxes[0]) > panelIndex && content.getChildIndex(boxes[1]) > panelIndex,
                "the boxes over the bottom panel after a tab change");

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
            for each (var pair:Array in texts)
            {
                for each (var text:String in pair)
                {
                    for (i = 0; i < text.length; ++i)
                    {
                        var letter:String = text.charAt(i);
                        if (letter != " " && !font.hasGlyphs(letter) && missing.indexOf(letter) < 0) missing += letter;
                    }
                }
            }
            require(missing == "", fontName + " lacks " + missing);
            trace("SETTINGS " + fontName + " has the glyphs of every dictionary's labels");
        }
    }
}
