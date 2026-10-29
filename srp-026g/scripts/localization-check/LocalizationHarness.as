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
                var friends:Array = ["Friends", "Друзі", "Друзья", "Freunde", "Znajomi", "Prijatelji", "Přátelé", "Barátok", "Kaverit", "Сябры"];
                for (var i:int = 0; i < friends.length; ++i)
                {
                    data.setData({btn_friends: friends[i]});
                    require(words.language() == i, "language detection " + i);
                    require(words.sayFirst("startMatch") != "undefined", "start label " + i);
                }
                var roomTitles:Array = ["Sobe", "Místnosti", "Szobák", "Huoneet", "Пакоі"];
                var starts:Array = ["Pokreni", "Spustit", "Indítás", "Aloita", "Пачаць"];
                var schools:Array = ["Škola", "Škola", "Iskola", "Koulu", "Школа"];
                var nights:Array = ["noć", "noc", "éjszaka", "yö", "ноч"];
                var research:Array = ["Istraživanje", "Výzkum", "Kutatás", "Tutkimus", "Даследаванне"];
                var joined:Array = ["se pridružio sobi", "se připojuje do místnosti", "belépett a szobába", "liittyi huoneeseen", "увайшоў у пакой"];
                var letters:Array = ["ČĆŠŽĐčćšžđ", "ÁČĎÉĚÍŇÓŘŠŤÚŮÝŽáčďéěíňóřšťúůýž", "ÁÉÍÓÖŐÚÜŰáéíóöőúüű", "ÅÄÖåäö", "ЎўІіЁёЫыЭэ'"];
                var fontClass:Class = domain.getDefinition("Fonts_CB_Bold") as Class;
                var font:Font = new fontClass() as Font;
                var label:TextField = new TextField();
                label.embedFonts = true;
                label.defaultTextFormat = new TextFormat(font.fontName, 15, 0xffffff, true);
                for (i = 5; i < friends.length; ++i)
                {
                    data.setData({btn_friends: friends[i]});
                    var added:int = i - 5;
                    require(words.sayFirst("servers") == roomTitles[added], "room title " + i);
                    require(words.sayFirst("startMatch") == starts[added], "start button " + i);
                    require(words.levelName(2, i) == schools[added], "map name " + i);
                    require(words.weatherName(4, i) == nights[added], "weather " + i);
                    require(words.modeName(5, i) == research[added], "research mode " + i);
                    require(words.notice("Ada joined the room") == "Ada " + joined[added], "chat notice " + i);
                    require(font.hasGlyphs(letters[added]), "Calibri lacks glyphs for language " + i);
                    label.text = words.sayFirst("roomChat");
                    require(label.textWidth <= 104, "room chat tab exceeds 108 px including padding: " + i);
                }
                trace("LOCALIZATION ten-language detection, added menu texts, tab widths and Calibri glyphs pass");
                trace("LOCALIZATION RESULT pass");
            }
            catch (error:Error)
            {
                trace("LOCALIZATION RESULT fail: " + error);
            }
        }
    }
}
