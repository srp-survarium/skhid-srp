package
{
    import flash.display.DisplayObject;
    import flash.display.Loader;
    import flash.display.MovieClip;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.UncaughtErrorEvent;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;
    import flash.text.Font;
    import flash.text.TextField;

    // The Settings' Language choice (exe patch 79) in the rebuilt main menu:
    // the exe's answer as root.srp_language gives it, the list, the saved
    // pick, the note, the other tabs, and the embedded font's glyphs for
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
            var language:* = domain.getDefinition("SrpLanguage");
            root.set_labels([{name: "gameplay_options_type", label: "Gameplay"}, {name: "settings_label", label: "Settings"}]);
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
            answer[2] = "english";
            root.srp_language.apply(null, answer);
            require(!note.visible && menu.selectedIndex == 0, "no note once saved and running are one");
            content.gotoAndStop(2);
            language.apply(settings);
            require(!menu.visible && !heading.visible && !note.visible, "nothing of it on another tab");
            content.gotoAndStop(1);
            language.apply(settings);
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
