package
{
    import flash.display.Loader;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.UncaughtErrorEvent;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;
    import flash.text.Font;
    import flash.text.TextField;
    import flash.text.TextFormat;

    // Uses the rebuilt movie's own classes and embedded font; no game or server.
    // Each language's texts reach the movie as the exe gives them
    // (set_localization_data: HarnessText, from the dictionaries).
    public class LocalizationHarness extends Sprite
    {
        private var loader:Loader = new Loader();

        public function LocalizationHarness()
        {
            loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
            loader.contentLoaderInfo.addEventListener("ioError", function(e:Event):void {
                trace("LOCALIZATION RESULT fail: " + e);
            });
            // The lobby root expects the native game. Only its classes are used here.
            loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
                function(e:UncaughtErrorEvent):void { e.preventDefault(); });
            loader.load(new URLRequest("lobby_menu.swf"), new LoaderContext(false, new ApplicationDomain()));
        }

        private function require(value:Boolean, message:String):void
        {
            if (!value) throw new Error(message);
        }

        private function loaded(e:Event):void
        {
            try
            {
                var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
                var data:* = domain.getDefinition("survarium.data.LocalizationData");
                var words:* = domain.getDefinition("survarium.rooms.RoomsText");
                var fontClass:Class = domain.getDefinition("Fonts_CB_Bold") as Class;
                var font:Font = new fontClass() as Font;
                var label:TextField = new TextField();
                label.embedFonts = true;
                label.defaultTextFormat = new TextFormat(font.fontName, 15, 0xffffff, true);
                var keys:int = 0;
                for each (var language:String in HarnessText.LANGUAGES)
                {
                    var text:Object = HarnessText.DATA[language];
                    data.setData(text);
                    // The movie's own lookups by number.
                    require(words.levelName(2) == text.st_srp_map_school, "map name " + language);
                    require(words.levelName(13) == text.st_srp_map_vostok_v011, "ported map name " + language);
                    require(words.weatherName(4) == text.st_srp_weather_night, "weather " + language);
                    require(words.modeName(5) == text.st_srp_mode_research, "research mode " + language);
                    require(words.codeText(3) == text.st_srp_rooms_code_3 && words.codeText(0) == "", "room code " + language);
                    require(words.joinWhyText(7) == text.st_srp_rooms_join_why_7, "join reason " + language);
                    // The server's English room line, in the language, the name in its place.
                    var joined:String = words.notice("Ada joined the room");
                    require(joined == String(text.st_srp_rooms_joined_room).split("%1$s").join("Ada"), "chat notice " + language);
                    require(language == "english" || joined != "Ada joined the room", "chat notice untranslated " + language);
                    // Every text: its placeholders filled, its letters in the font.
                    keys = 0;
                    for (var key:String in text)
                    {
                        var filled:String = data.format(key, "11", "22", "33", "44");
                        require(!/%\d\$s/.test(filled), "a placeholder past %4$s left in " + key + " " + language);
                        require(font.hasGlyphs(filled.replace(/\s/g, "")), "Calibri lacks glyphs of " + key + " (" + language + "): " + filled);
                        keys++;
                    }
                    require(data.format("st_srp_replay_place_in_line", 3, 12).indexOf("%") < 0, "replay line " + language);
                    label.text = data.getName("st_srp_rooms_room_chat");
                    require(label.textWidth <= 104, "room chat tab exceeds 108 px including padding: " + language);
                }
                trace("LOCALIZATION " + HarnessText.LANGUAGES.length + " languages x " + keys
                    + " texts: lookups, placeholders, tab widths and Calibri glyphs pass");
                trace("LOCALIZATION RESULT pass");
            }
            catch (error:Error)
            {
                trace("LOCALIZATION RESULT fail: " + error.message);
            }
        }
    }
}
