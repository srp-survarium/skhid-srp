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

    // The Settings' Lobby choice (exe patch 81) in the rebuilt main menu: the
    // exe's answer as root.srp_lobby_scene gives it (and root.srp_language
    // the Language's, its neighbour), the arrow selector over the four
    // lobbies, the saved one selected, the note, the two columns of the
    // bottom panel, a pick as a pending change that Apply saves and Cancel
    // drops, the other tabs, and the embedded font's glyphs for every
    // language's texts (check.sh passes them, from the dictionaries). Then it
    // leaves the tab on screen for check.sh's screenshot. No game or server.
    public class LobbySceneHarness extends Sprite
    {
        private var loader:Loader = new Loader();
        private var frames:int = 0;

        public function LobbySceneHarness()
        {
            loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
            loader.contentLoaderInfo.addEventListener("ioError", function(e:Event):void {
                trace("LOBBY RESULT fail: " + e);
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

        private function event(name:String, type:String):Event
        {
            var kind:Class = loader.contentLoaderInfo.applicationDomain.getDefinition(name) as Class;
            return new kind(type) as Event;
        }

        // A click of an arrow: the stepper moves at its next draw (here at once).
        private function arrow(stepper:*, next:Boolean):void
        {
            (next ? stepper.nextBtn : stepper.prevBtn).dispatchEvent(event("scaleform.clik.events.ButtonEvent", "buttonClick"));
            stepper.validateNow();
        }

        // A click of Apply or Cancel. Outside the game the exe calls after it
        // may throw; what the buttons did before them is what is checked.
        private function click(button:*):void
        {
            try
            {
                button.dispatchEvent(event("scaleform.clik.events.ButtonEvent", "buttonClick"));
            }
            catch (error:Error)
            {
            }
        }

        private function tick(e:Event):void
        {
            if (++frames < 10) return;
            removeEventListener(Event.ENTER_FRAME, tick);
            run(check);
            if (failed) return;
            frames = 0;
            addEventListener(Event.ENTER_FRAME, settle);
        }

        private var failed:Boolean = false;

        private function run(step:Function):void
        {
            try
            {
                step();
            }
            catch (error:Error)
            {
                failed = true;
                trace("LOBBY RESULT fail: " + error.message);
            }
        }

        // Some frames on (the stepper takes its index at its own draw):
        // still nothing pending; the screenshot (check.sh, at LOBBY CAPTURE),
        // then the other tabs.
        private function settle(e:Event):void
        {
            ++frames;
            if (frames == 20)
            {
                run(settled);
            }
            if (frames == 20 && !failed)
            {
                trace("LOBBY CAPTURE");
            }
            if (frames < 140 && !failed) return;
            removeEventListener(Event.ENTER_FRAME, settle);
            if (failed) return;
            run(tabs);
            if (!failed) trace("LOBBY RESULT pass");
        }

        private function settled():void
        {
            var settings:* = loader.content["settings_window"];
            var stepper:* = settings.settings_content.getChildByName("_srp_lobby_scene");
            require(!settings.btn_accept.enabled && settings.btn_cancel.label == "Exit" && stepper.selectedIndex == 2,
                "frames later the saved lobby, nothing to apply: Apply " + settings.btn_accept.enabled + ", "
                + settings.btn_cancel.label + ", " + stepper.selectedIndex);
            trace("LOBBY frames later the saved lobby is selected and nothing is pending");
        }

        // A tab change binds the tab's content again (create_content);
        // Settings' own tab change calls the exe first, which a test player
        // lacks, so the frame is changed here and bound again. (Binding the
        // gameplay tab's test options on the other frame marks some of that
        // frame's controls changed a frame later: the test's, not the game's.)
        private function tabs():void
        {
            var root:* = loader.content;
            var settings:* = root.settings_window;
            var content:MovieClip = settings.settings_content;
            var stepper:* = content.getChildByName("_srp_lobby_scene");
            var note:TextField = content.getChildByName("_srp_lobby_scene_note") as TextField;
            var settingsList:Array = [];
            for (var i:int = 0; i < 16; ++i) settingsList.push({label: "option " + i});
            content.gotoAndStop(2);
            root.set_settings(0, settingsList);
            require(!stepper.visible && !note.visible, "nothing of it on another tab");
            content.gotoAndStop(1);
            root.set_settings(0, settingsList);
            require(stepper.visible && note.visible && stepper.selectedIndex == 2, "back on the gameplay tab, the saved lobby selected");
            trace("LOBBY saved pick, note and tabs pass");
        }

        private function find(content:MovieClip, text:String):TextField
        {
            for (var i:int = 0; i < content.numChildren; ++i)
            {
                var child:DisplayObject = content.getChildAt(i);
                if (child is TextField && TextField(child).text == text) return TextField(child);
            }
            return null;
        }

        private function check():void
        {
            // [label, note, Classic, S.T.A.L.K.E.R. 2, Hangar, Camp] of each language; the first is English
            var texts:Array = JSON.parse(loaderInfo.parameters.texts) as Array;
            var language:Array = JSON.parse(loaderInfo.parameters.language) as Array;
            var ids:Array = ["day", "stk2_blockpost", "hangar_046", "camp_069"];
            var root:* = loader.content;
            var settings:* = root.settings_window;
            var content:MovieClip = settings.settings_content;
            var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
            root.set_labels([{name: "gameplay_options_type", label: "Gameplay"}, {name: "controllers_options_type", label: "Controls"},
                {name: "settings_label", label: "Settings"}, {name: "button_apply", label: "Apply"},
                {name: "button_exit", label: "Exit"}, {name: "button_cancel", label: "Cancel"}]);
            var settingsList:Array = [];
            for (var i:int = 0; i < 16; ++i) settingsList.push({label: "option " + i});
            root.set_settings(0, settingsList);
            var english:Array = texts[0];
            var answer:Array = [english[0], english[1], "hangar_046", "day"];
            for (i = 0; i < ids.length; ++i) answer.push(ids[i], english[2 + i]);
            root.srp_language.apply(null, [language[0], language[1], "english", "english", "english", "English", "czech", "Čeština"]);
            root.srp_lobby_scene.apply(null, answer);
            root.show_settings(true);

            var stepper:* = content.getChildByName("_srp_lobby_scene");
            stepper.validateNow();          // the stepper takes its index at its next draw
            require(stepper != null && stepper.visible && stepper.alpha == 1, "the selector is on the gameplay tab");
            require(stepper.dataProvider.length == ids.length, "every lobby listed");
            require(stepper.selectedIndex == 2 && stepper.textField2.text == english[4], "the saved lobby selected: "
                + stepper.selectedIndex + " " + stepper.textField2.text);
            require(stepper.textField.text == english[0], "the selector's label: " + stepper.textField.text);
            var note:TextField = content.getChildByName("_srp_lobby_scene_note") as TextField;
            require(note != null && note.text == english[1] && note.visible, "the note shows while the saved lobby is not the one shown");

            // the two columns of the bottom panel
            var panel:Rectangle = content.bottomContainer.getBounds(content);
            var middle:Number = panel.x + panel.width / 2;
            var menu:* = content.getChildByName("_srp_language");
            var heading:TextField = find(content, language[0]);
            require(menu != null && menu.visible && heading != null && heading.visible, "the Language choice shows");
            for each (var part:DisplayObject in [heading, menu, find(content, language[1]), stepper, note])
            {
                var bounds:Rectangle = part.getBounds(content);
                require(panel.containsRect(bounds), "in the bottom panel " + panel + ": " + part.name + " " + bounds);
            }
            require(menu.getBounds(content).right < middle && heading.getBounds(content).right < middle, "Language in the left half");
            var box:Rectangle = stepper.getBounds(content);
            require(box.left > middle, "Lobby in the right half: " + box);
            require(Math.abs(stepper.y - menu.y) < 1, "on the Language list's row");
            require(Math.abs(box.x + box.width / 2 - (middle + panel.right) / 2) < 6, "the selector centred in the right half: " + box);
            trace("LOBBY selector at " + box + ", note at " + note.getBounds(content) + "; list at " + menu.getBounds(content)
                + "; panel " + panel);

            // the arrows: one step each, stopping at the ends
            require(!settings.btn_accept.enabled, "nothing to apply before a pick");
            arrow(stepper, true);
            require(stepper.selectedIndex == 3 && stepper.textField2.text == english[5], "the right arrow: Camp");
            require(settings.btn_accept.enabled && settings.btn_cancel.label == "Cancel", "a pick enables Apply and Cancel");
            require(note.visible, "the note follows the saved lobby, not a pending pick");
            arrow(stepper, true);
            require(stepper.selectedIndex == 3, "no step past the last");
            arrow(stepper, false);
            require(stepper.selectedIndex == 2 && !settings.btn_accept.enabled && settings.btn_cancel.label == "Exit",
                "back to the saved one: nothing to apply");
            arrow(stepper, false);
            arrow(stepper, false);
            require(stepper.selectedIndex == 0 && stepper.textField2.text == english[2], "the left arrow: Classic");
            arrow(stepper, false);
            require(stepper.selectedIndex == 0, "no step before the first");
            click(settings.btn_cancel);
            stepper.validateNow();
            require(stepper.selectedIndex == 2 && !settings.btn_accept.enabled && settings.btn_cancel.label == "Exit",
                "Cancel puts the saved lobby back");
            arrow(stepper, false);
            require(stepper.selectedIndex == 1 && stepper.textField2.text == english[3], "S.T.A.L.K.E.R. 2");
            arrow(stepper, false);
            click(settings.btn_accept);
            stepper.validateNow();
            require(stepper.selectedIndex == 0 && !settings.btn_accept.enabled && !note.visible,
                "Apply saves the pick (saved and shown are one: no note)");
            trace("LOBBY the arrows step through the four; a pick is pending until Apply; Cancel drops it");

            answer[2] = "camp_069";
            root.srp_lobby_scene.apply(null, answer);
            stepper.validateNow();
            require(note.visible && stepper.selectedIndex == 3, "a saved lobby not shown: selected, with the note");
            answer[2] = "nowhere";
            root.srp_lobby_scene.apply(null, answer);
            stepper.validateNow();
            require(stepper.selectedIndex == 0 && !settings.btn_accept.enabled, "an unknown saved id is the classic, saved");
            answer[2] = "hangar_046";
            root.srp_lobby_scene.apply(null, answer);
            stepper.validateNow();
            require(stepper.selectedIndex == 2 && note.visible && !settings.btn_accept.enabled, "the saved lobby again");
            trace("LOBBY saved pick and note pass");

            var font:Font = null;
            var fontName:String = note.defaultTextFormat.font;
            for each (var name:String in ["CalibriBold", "FontTB"])
            {
                if (!domain.hasDefinition(name)) continue;
                var candidate:Font = new (domain.getDefinition(name) as Class)() as Font;
                if (candidate.fontName == fontName) font = candidate;
            }
            require(font != null, "the note's font " + fontName + " is one the menu embeds");
            var letters:String = "";
            for each (var row:Array in texts) letters += row.join("");
            var missing:String = "";
            for (i = 0; i < letters.length; ++i)
            {
                var letter:String = letters.charAt(i);
                if (letter != " " && !font.hasGlyphs(letter) && missing.indexOf(letter) < 0) missing += letter;
            }
            require(missing == "", fontName + " lacks " + missing);
            trace("LOBBY " + fontName + " has the glyphs of every text");
            // every language's label and lobby names fit the selector's fields
            var labelField:TextField = stepper.textField, valueField:TextField = stepper.textField2;
            var shownLabel:String = labelField.text, shownValue:String = valueField.text;
            for each (row in texts)
            {
                labelField.text = row[0];
                require(labelField.textWidth + 4 <= labelField.width, "the label fits: " + row[0]);
                for (i = 2; i < 6; ++i)
                {
                    valueField.text = row[i];
                    require(valueField.textWidth + 4 <= valueField.width, "the name fits: " + row[i] + " "
                        + valueField.textWidth + " of " + valueField.width);
                }
            }
            labelField.text = shownLabel;
            valueField.text = shownValue;
            trace("LOBBY every language's label and names fit (the value field " + valueField.width + " wide)");
            var shot:Rectangle = content.bottomContainer.getBounds(stage);
            trace("LOBBY SHOT " + Math.round(shot.x) + " " + Math.round(shot.y) + " " + Math.round(shot.width) + " "
                + Math.round(shot.height));
            var window:Rectangle = settings.getBounds(stage);
            trace("LOBBY WINDOW " + Math.round(window.x) + " " + Math.round(window.y) + " " + Math.round(window.width) + " "
                + Math.round(window.height));
        }
    }
}
