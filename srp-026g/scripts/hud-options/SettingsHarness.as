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

        private function check():void
        {
            // [[hit markers' label, scores' label], ...] for every dictionary
            var texts:Array = JSON.parse(loaderInfo.parameters.texts) as Array;
            var root:* = loader.content;
            var settings:* = root.settings_window;
            var content:MovieClip = settings.settings_content;
            var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
            root.set_labels([{name: "gameplay_options_type", label: "Gameplay"}, {name: "controllers_options_type", label: "Controls"},
                {name: "settings_label", label: "Settings"}, {name: "button_apply", label: "Apply"},
                {name: "button_exit", label: "Exit"}, {name: "button_cancel", label: "Cancel"}]);
            var settingsList:Array = [];
            var values:Array = [];
            for (var i:int = 0; i < 16; ++i)
            {
                settingsList.push({label: "option " + i});
                // every option's value, as the exe binds them (the quests' alpha slider, 11, at 1)
                values.push({id: i, value: i == 11 ? 1 : 0});
            }
            root.set_settings(0, settingsList);
            root.set_values(0, values);
            root.srp_language("Language:", "Applies after restart.", "english", "english", "english", "English",
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
                root.srp_check_option(NAMES[i], texts[0][i], "0");
                require(boxes[i].visible && boxes[i].alpha == 1, NAMES[i] + ": on the gameplay tab");
                require(boxes[i].label == texts[0][i], NAMES[i] + ": its label is the exe's: " + boxes[i].label);
                require(!boxes[i].selected, NAMES[i] + ": the saved state (off) is shown");
            }
            require(!settings.btn_accept.enabled, "nothing to apply after the answers");
            root.srp_check_option("srp_no_such", "x", "1");
            require(!boxes[0].selected && !boxes[1].selected && !settings.btn_accept.enabled, "another name changes nothing");

            var panel:Rectangle = content.bottomContainer.getBounds(content);
            var menu:DisplayObject = content.getChildByName("_srp_language");
            var invite:DisplayObject = content["_setter_invite"];
            for (i = 0; i < boxes.length; ++i)
            {
                box = boxes[i];
                // the box and its text (its label's field runs on past the text, as the tab's own)
                var bounds:Rectangle = box.getBounds(content);
                require(panel.contains(box.x, box.y) && panel.contains(box.x, box.y + box.height),
                    NAMES[i] + ": in the bottom panel " + panel + ": " + bounds);
                require(box.x > menu.x + menu.width + 10, NAMES[i] + ": right of the Language list (" + (menu.x + menu.width) + "): " + box.x);
                require(Math.abs(box.width - invite.width) < 1 && Math.abs(box.height - invite.height) < 1,
                    NAMES[i] + ": the tab's checkbox at its size: " + box.width + "x" + box.height + " (" + invite.width + "x" + invite.height + ")");
                // The longest dictionary label, drawn: it ends inside the panel.
                var shown:String = box.label;
                var longest:Number = 0;
                var longestText:String = "";
                for each (var pair:Array in texts)
                {
                    box.label = pair[i];
                    box.validateNow();
                    if (box.textField.textWidth > longest)
                    {
                        longest = box.textField.textWidth;
                        longestText = pair[i];
                    }
                }
                box.label = shown;
                box.validateNow();
                var textEnd:Number = box.x + box.textField.x + longest;
                require(textEnd < panel.right - 4, NAMES[i] + ": the longest label (" + longestText + ") ends in the panel ("
                    + panel.right + "): " + textEnd);
                trace("SETTINGS " + NAMES[i] + " " + box.label + " at " + bounds + ", longest label " + longestText
                    + " to " + int(textEnd) + "; Language list " + menu.x + ".." + (menu.x + menu.width) + "; panel " + panel);
            }
            // one under the other, spaced as the left column's first two
            var step:Number = content["_setter_message"].y - invite.y;
            require(boxes[0].x == boxes[1].x && Math.abs(boxes[1].y - boxes[0].y - step) < 1,
                "one under the other, " + step + " apart: " + boxes[0].y + ", " + boxes[1].y);

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
            require(content.getChildIndex(menu) > panelIndex, "the Language list over the bottom panel after a tab change");
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
            for each (pair in texts)
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
