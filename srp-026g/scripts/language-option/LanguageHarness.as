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
    import flash.text.TextFormat;

    // The Settings' Language choice (exe patch 79) in the rebuilt main menu:
    // the exe's answer as root.srp_language gives it, the list, the saved
    // pick, the note, its place in the bottom panel (the seeding and
    // statistics options hidden), a pick as a pending change that Apply saves
    // and Cancel drops, the other tabs, and the embedded font's glyphs for
    // every language's own name and texts (check.sh passes them, from the
    // registry and the dictionaries). No game or server.
    public class LanguageHarness extends Sprite
    {
        private var loader:Loader = new Loader();
        private var frames:int = 0;

        public function LanguageHarness()
        {
            loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
            loader.contentLoaderInfo.addEventListener("ioError", function(e:Event):void {
                trace("LANGUAGE RESULT fail: " + e);
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

        // A player's pick: the list dispatches its change only while open.
        private function pick(menu:*, index:int):void
        {
            var listEvent:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.events.ListEvent") as Class;
            menu.selectedIndex = index;
            menu.dispatchEvent(new listEvent("listIndexChange", true, false, index));
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
                trace("LANGUAGE RESULT pass");
            }
            catch (error:Error)
            {
                trace("LANGUAGE RESULT fail: " + error.message);
            }
        }

        private function check():void
        {
            var languages:Array = JSON.parse(loaderInfo.parameters.languages) as Array;
            var texts:Array = JSON.parse(loaderInfo.parameters.texts) as Array;
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
            var answer:Array = [texts[0][0], texts[0][1], "czech", "english"];
            var natives:Array = [];
            for each (var row:Array in languages)
            {
                answer.push(row[0], row[1]);
                natives.push(row[1]);
            }
            root.srp_language.apply(null, answer);
            root.show_settings(true);

            var menu:* = content.getChildByName("_srp_language");
            require(menu != null && menu.visible && menu.alpha == 1, "the list is on the gameplay tab");
            require(menu.dataProvider.length == languages.length, "every registered language listed");
            for (i = 0; i < natives.length; ++i)
            {
                require(menu.dataProvider[i] == natives[i], "listed by its own name: " + natives[i]);
            }
            var czech:int = -1;
            for (i = 0; i < languages.length; ++i) if (languages[i][0] == "czech") czech = i;
            require(menu.selectedIndex == czech, "the saved language selected");
            var heading:TextField = null, note:TextField = null;
            for (i = 0; i < content.numChildren; ++i)
            {
                var child:DisplayObject = content.getChildAt(i);
                if (child is TextField && TextField(child).text == texts[0][0]) heading = TextField(child);
                if (child is TextField && TextField(child).text == texts[0][1]) note = TextField(child);
            }
            require(heading != null && heading.visible, "the heading shows the label");
            require(note != null && note.visible, "the note shows while the saved language is not the running one");

            for each (var unsupported:String in ["_setter_static_use_seed", "_upload_speed", "_setter_static_allow_statistics"])
            {
                require(content[unsupported] != null && !content[unsupported].visible, unsupported + " is hidden");
            }
            var panel:Rectangle = content.bottomContainer.getBounds(content);
            for each (var part:DisplayObject in [heading, menu, note])
            {
                var bounds:Rectangle = part.getBounds(content);
                require(panel.containsRect(bounds), "in the bottom panel " + panel + ": " + part.name + " " + bounds);
            }
            var middle:Number = panel.x + panel.width / 2;
            require(Math.abs(menu.x + menu.width / 2 - middle) < 4, "the list centred in the panel");
            var model:TextFormat = content._upload_speed.textField.defaultTextFormat;
            var format:TextFormat = heading.defaultTextFormat;
            require(format.font == model.font && format.size == model.size && format.color == model.color,
                "the label styled as the \"Limit upload speed:\" label");
            require(heading.text.charAt(heading.text.length - 1) == ":", "the label ends with a colon: " + heading.text);
            trace("LANGUAGE label " + heading.text + " " + format.font + " " + format.size + " #" + uint(format.color).toString(16)
                + " at " + heading.x + "," + heading.y + "; list at " + menu.x + "," + menu.y + " w " + menu.width
                + "; note at " + note.x + "," + note.y + "; panel " + panel);

            var english:int = 0;
            require(!settings.btn_accept.enabled, "nothing to apply before a pick");
            pick(menu, english);
            require(settings.btn_accept.enabled && settings.btn_cancel.label == "Cancel", "a pick enables Apply and Cancel");
            require(note.visible, "the note follows the saved language, not a pending pick");
            pick(menu, czech);
            require(!settings.btn_accept.enabled && settings.btn_cancel.label == "Exit", "picking the saved one back: nothing to apply");
            pick(menu, english);
            click(settings.btn_cancel);
            require(menu.selectedIndex == czech && !settings.btn_accept.enabled && settings.btn_cancel.label == "Exit",
                "Cancel puts the saved language back");
            pick(menu, english);
            click(settings.btn_accept);
            require(menu.selectedIndex == english && !settings.btn_accept.enabled && !note.visible,
                "Apply saves the pick (saved and running are one: no note)");
            trace("LANGUAGE a pick is pending until Apply; Cancel drops it");

            answer[2] = "english";
            root.srp_language.apply(null, answer);
            require(!note.visible && menu.selectedIndex == 0, "no note once saved and running are one");
            // A tab change binds the tab's content again (create_content);
            // Settings' own tab change calls the exe first, which a test
            // player lacks, so the frame is changed here and bound again.
            content.gotoAndStop(2);
            root.set_settings(0, settingsList);
            require(!menu.visible && !heading.visible && !note.visible, "nothing of it on another tab");
            content.gotoAndStop(1);
            root.set_settings(0, settingsList);
            require(menu.visible && heading.visible, "back on the gameplay tab");
            trace("LANGUAGE list, saved pick, note and tabs pass");

            var fontName:String = heading.defaultTextFormat.font;
            var font:Font = null;
            for each (var name:String in ["CalibriBold", "FontTB"])
            {
                if (!domain.hasDefinition(name)) continue;
                var candidate:Font = new (domain.getDefinition(name) as Class)() as Font;
                if (candidate.fontName == fontName) font = candidate;
            }
            require(font != null, "the heading's font " + fontName + " is one the menu embeds");
            var letters:String = natives.join("");
            for each (var pair:Array in texts) letters += pair.join("");
            var missing:String = "";
            for (i = 0; i < letters.length; ++i)
            {
                var letter:String = letters.charAt(i);
                if (letter != " " && !font.hasGlyphs(letter) && missing.indexOf(letter) < 0) missing += letter;
            }
            require(missing == "", fontName + " lacks " + missing);
            trace("LANGUAGE " + fontName + " has the glyphs of every own name and text");
        }
    }
}
