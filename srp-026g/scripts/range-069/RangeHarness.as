package
{
    import flash.display.*;
    import flash.events.*;
    import flash.net.URLRequest;
    import flash.system.*;
    import flash.text.TextField;

    public class RangeHarness extends Sprite
    {
        private var loader:Loader = new Loader();
        public function RangeHarness()
        {
            loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
                function(e:UncaughtErrorEvent):void { e.preventDefault(); });
            loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
            loader.load(new URLRequest(loaderInfo.parameters.movie),
                new LoaderContext(false, new ApplicationDomain()));
        }
        private function check(ok:Boolean, message:String):void
        {
            if (!ok) throw new Error(message);
            trace("RANGE PASS " + message);
        }
        private function near(actual:Number, expected:Number):Boolean
        {
            return Math.abs(actual - expected) < 0.06;
        }
        private function tell(Win:*, rows:Array):void
        {
            var wire:Array = [];
            for (var i:int = 0; i < rows.length; ++i)
                wire.push({place:2060000000 + i, nickname:rows[i]});
            Win.take(wire);
        }
        private function children(root:DisplayObjectContainer):Array
        {
            var out:Array = [];
            for (var i:int = 0; i < root.numChildren; ++i)
            {
                var child:DisplayObject = root.getChildAt(i);
                out.push(child);
                if (child is DisplayObjectContainer)
                    out = out.concat(children(child as DisplayObjectContainer));
            }
            return out;
        }
        private function loaded(e:Event):void
        {
            try
            {
                var d:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
                if (loaderInfo.parameters.movie == "hud.swf")
                {
                    var map:* = new (d.getDefinition("SrvMinimap") as Class)();
                    addChild(map);
                    map.centerPlayer = false;
                    var props:Object = {map:"shooting_range"};
                    map.setProps(props);
                    check(props.map == "shooting_range" && map.hasMap, "range map loads without fallback");
                    check(map.getMapSize().x == 512 && map.getMapSize().y == 512, "minimap bitmap is bound");
                    map.updatePlayerPos(-38, -4, 0, 0);
                    check(near(map.minimap.player.x, 256) && near(map.minimap.player.y, 256), "authored world centre maps to image centre");
                    map.updatePlayerPos(-37.412, -0.437, 0, 0);
                    check(near(map.minimap.player.x, 257.50528) && near(map.minimap.player.y, 265.12128), "player spawn uses correct scale and Z sign");
                }
                else
                {
                    var text:* = d.getDefinition("survarium.rooms.RoomsText");
                    var locale:* = d.getDefinition("survarium.data.LocalizationData");
                    check(text.levelKey(21) == "st_srp_map_shooting_range", "room level resolves range name");
                    for (var language:String in HarnessText.DATA)
                    {
                        locale.setData(HarnessText.DATA[language]);
                        var name:String = text.levelName(21);
                        check(name.length > 0 && name.indexOf("st_") != 0 && name != "21", "localized range name: " + language);
                    }
                    locale.setData(HarnessText.DATA.english);
                    var picture:BitmapData = new (d.getDefinition("Level_shooting_range") as Class)();
                    check(picture.width == 1920 && picture.height == 798, "original loading picture is bound");
                    var Win:* = d.getDefinition("survarium.rooms.RoomsWindow");
                    var panel:* = new (d.getDefinition("SrvBottomPanel") as Class)();
                    panel.y = 700;
                    var win:* = new Win(panel);
                    Win.me = win;
                    addChild(win);
                    tell(Win, ["S1;N;0;0;0"]);
                    tell(Win, ["S1;M;1;0", "0;250;21;1;4;0;0"]);
                    win.show(true);
                    var press:* = null;
                    for each (var item:* in children(win))
                        if ("label" in item && item.label == "Private room") press = item;
                    check(press != null, "room creation is available");
                    var ButtonEvent:Class = d.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
                    press.dispatchEvent(new ButtonEvent(ButtonEvent["CLICK"]));
                    var named:Boolean = false;
                    var illustrated:Boolean = false;
                    for each (item in children(win))
                    {
                        if (item is TextField && item.text == "Firing range") named = true;
                        if (item is Bitmap && item.bitmapData != null && item.width > 80)
                        {
                            var pixels:BitmapData = item.bitmapData;
                            if (pixels.getPixel(pixels.width / 2, pixels.height / 2) != 0x05080B)
                                illustrated = true;
                        }
                    }
                    check(named && illustrated, "range is an illustrated choice in room creation");
                }
                trace("RANGE RESULT pass");
            }
            catch (error:Error) { trace("RANGE RESULT fail: " + error.message); }
        }
    }
}
