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
                var friends:Array = ["Friends", "Друзі", "Друзья", "Freunde", "Znajomi", "Prijatelji"];
                for (var i:int = 0; i < friends.length; ++i)
                {
                    data.setData({btn_friends: friends[i]});
                    require(words.language() == i, "language detection " + i);
                    require(words.sayFirst("startMatch") != "undefined", "start label " + i);
                }
                require(words.sayFirst("servers") == "Sobe", "Serbian room title");
                require(words.sayFirst("startMatch") == "Pokreni", "Serbian start button");
                require(words.levelName(2, 5) == "Škola", "Serbian map name");
                require(words.weatherName(4, 5) == "noć", "Serbian weather");
                require(words.modeName(5, 5) == "Istraživanje", "Serbian research mode");
                require(words.notice("Đorđe joined the room") == "Đorđe se pridružio sobi", "Serbian chat notice");
                var fontClass:Class = domain.getDefinition("Fonts_CB_Bold") as Class;
                var font:Font = new fontClass() as Font;
                require(font.hasGlyphs("ČĆŠŽĐčćšžđ"), "Calibri lacks Serbian Latin glyphs");
                trace("LOCALIZATION Serbian detection, room texts and Calibri Latin glyphs pass");
                trace("LOCALIZATION RESULT pass");
            }
            catch (error:Error)
            {
                trace("LOCALIZATION RESULT fail: " + error);
            }
        }
    }
}
