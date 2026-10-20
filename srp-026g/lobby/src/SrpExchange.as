package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.FocusEvent;
    import flash.events.MouseEvent;
    import flash.external.ExternalInterface;
    import flash.text.TextField;
    import flash.text.TextFieldType;
    import flash.text.TextFormat;
    import flash.utils.getDefinitionByName;
    import flash.utils.getQualifiedClassName;
    import scaleform.gfx.FocusManager;
    import survarium.Util;
    import survarium.controls.Message;
    import survarium.controls.PlayerInfo;
    import survarium.data.LocalizationData;
    import survarium.messages.MessageMoneyExchange;

    // The stock "gold for silver" window rebuilt as an exchange both ways:
    // Pay above, Receive below, the swap between them, the rate under them.
    // Typing in either field updates the other. The game's own pieces are
    // reused: the window, its buttons and their skin, the coin icons and the
    // font of its texts.
    public class SrpExchange extends Sprite
    {
        // `47 04` amounts from here on buy gold with silver (srp's own); below
        // it they are the stock "this much gold for silver".
        public static const BUY_GOLD:uint = 0x40000000;
        public static const NAME:String = "srpExchange";
        public static const W:Number = 300;
        public static const H:Number = 300;
        // The stock window: 288 wide, its buttons 47 above its bottom.
        public static const STOCK_W:Number = 288;
        public static const BUTTONS_UP:Number = 47;
        public static const MAX_GOLD:uint = 10000000;

        private static const TEXT:Object = {
            "title": ["Exchange", "Обмін", "Обмен", "Tausch", "Wymiana"],
            "pay": ["Pay", "Віддаєте", "Отдаёте", "Sie zahlen", "Płacisz"],
            "receive": ["Receive", "Отримуєте", "Получаете", "Sie erhalten", "Otrzymujesz"],
            "silver": ["Silver", "Срібло", "Серебро", "Silber", "Srebro"],
            "gold": ["Gold", "Золото", "Золото", "Gold", "Złoto"],
            "balance": ["Balance", "Баланс", "Баланс", "Guthaben", "Saldo"],
            "short_silver": ["Not enough silver", "Недостатньо срібла", "Недостаточно серебра",
                "Nicht genug Silber", "Za mało srebra"],
            "short_gold": ["Not enough gold", "Недостатньо золота", "Недостаточно золота",
                "Nicht genug Gold", "Za mało złota"],
            "button": ["Exchange", "Обміняти", "Обменять", "Tauschen", "Wymień"]
        };
        // The client's own "cancel" names its language.
        private static const CANCEL:Array = ["Cancel", "Скасувати", "Отмена", "Abrechnen", "Anuluj"];

        private var _menu:MessageMoneyExchange;
        private var _lang:int = 0;
        // The stock fields' font: bold Calibri from the font library. The
        // movie's own plain "Calibri" is a stub of two glyphs.
        private var _font:String;
        private var _bold:Object;
        private var _payGold:Boolean = false;
        private var _gold:uint = 1;
        private var _payLabel:TextField;
        private var _payBalance:TextField;
        private var _payField:TextField;
        private var _payName:TextField;
        private var _payCoin:MovieClip;
        private var _getLabel:TextField;
        private var _getBalance:TextField;
        private var _getField:TextField;
        private var _getName:TextField;
        private var _getCoin:MovieClip;
        private var _rate:TextField;
        private var _note:TextField;
        private var _swap:Boolean = false;

        // From the gold button (buy gold) and the silver button (sell gold).
        public static function open(buyGold:Boolean):void
        {
            Message.showMessage("exchange", {"label": "", "srpBuyGold": buyGold});
        }

        // The end of MessageMoneyExchange's constructor: the size is final
        // before Message measures the window to centre it.
        public static function build(menu:MessageMoneyExchange):void
        {
            var view:SrpExchange = new SrpExchange(menu);
            view.name = NAME;
            menu.addChild(view);
        }

        // The end of MessageMoneyExchange.configUI, after the stock set-up.
        public static function apply(menu:MessageMoneyExchange):void
        {
            var view:SrpExchange = menu.getChildByName(NAME) as SrpExchange;
            if (view != null)
                view.configure();
        }

        // MessageMoneyExchange.onMessageAccept: the Exchange button and Enter.
        public static function accept(menu:MessageMoneyExchange):void
        {
            var view:SrpExchange = menu.getChildByName(NAME) as SrpExchange;
            if (view == null)
                return;
            view.snap();
            view.refresh();
            if (!menu.btnAccept.enabled)
                return;
            // A uint, as the stock window sends it: the client reads it as one.
            var amount:uint = view._payGold ? view._gold : BUY_GOLD + view._gold;
            ExternalInterface.call("exchange_premium_money", amount);
            Message.onClick(menu.getMessageId());
        }

        public function SrpExchange(menu:MessageMoneyExchange)
        {
            super();
            _menu = menu;
            var stock:TextFormat = menu.textFieldLabel.getTextFormat();
            _font = stock.font;
            _bold = stock.bold;
            for (var i:int = 0; i < menu.numChildren; i++)
            {
                var child:DisplayObject = menu.getChildAt(i);
                if (getQualifiedClassName(child) == "MessageSmallBackground")
                {
                    child.width = W;
                    child.height = H;
                }
            }
            var dx:Number = (W - STOCK_W) / 2;
            menu.btnAccept.x += dx;
            menu.btnCancel.x += dx;
            menu.btnAccept.y = H - BUTTONS_UP;
            menu.btnCancel.y = H - BUTTONS_UP;
            menu.textFieldLabel.x = 8;
            menu.textFieldLabel.width = W - 16;
            menu.textFieldLabel.y = 14;

            panel(14, 50);
            _payLabel = text(24, 56, 120, 20, 14, 0x8D8A91, "left");
            _payBalance = text(146, 56, 130, 20, 14, 0x8D8A91, "right");
            _payField = field(22, 78);
            _payCoin = coin(200, 85);
            _payName = text(222, 82, 66, 26, 18, 0xB6B6B6, "left");

            panel(14, 130);
            _getLabel = text(24, 136, 120, 20, 14, 0x8D8A91, "left");
            _getBalance = text(146, 136, 130, 20, 14, 0x8D8A91, "right");
            _getField = field(22, 158);
            _getCoin = coin(200, 165);
            _getName = text(222, 162, 66, 26, 18, 0xB6B6B6, "left");

            _rate = text(14, 206, W - 28, 22, 15, 0xB6B6B6, "center");
            _note = text(14, 228, W - 28, 22, 15, 0xD64531, "center");
        }

        // The dark box of the stock stepper: black at 40 %, a faint light
        // edge right and below.
        private function panel(x:Number, y:Number):void
        {
            graphics.beginFill(0x000000, 0.404);
            graphics.drawRect(x, y, W - 2 * x, 70);
            graphics.endFill();
            graphics.beginFill(0xD4D4D4, 0.047);
            graphics.drawRect(x, y + 69, W - 2 * x, 1);
            graphics.drawRect(W - x - 1, y, 1, 69);
            graphics.endFill();
        }

        private function text(x:Number, y:Number, w:Number, h:Number, size:Number,
            color:uint, align:String):TextField
        {
            var tf:TextField = new TextField();
            var format:TextFormat = new TextFormat(_font, size, color, _bold);
            format.align = align;
            tf.defaultTextFormat = format;
            tf.embedFonts = true;
            tf.x = x;
            tf.y = y;
            tf.width = w;
            tf.height = h;
            tf.multiline = false;
            tf.wordWrap = false;
            tf.selectable = false;
            tf.mouseEnabled = false;
            tf.tabEnabled = false;
            addChild(tf);
            return tf;
        }

        private function field(x:Number, y:Number):TextField
        {
            var tf:TextField = text(x, y, 168, 34, 26, 0xF4B717, "left");
            tf.type = TextFieldType.INPUT;
            tf.restrict = "0-9";
            tf.maxChars = 10;
            tf.selectable = true;
            tf.mouseEnabled = true;
            tf.tabEnabled = true;
            tf.addEventListener(Event.CHANGE, onEdit, false, 0, true);
            tf.addEventListener(FocusEvent.FOCUS_OUT, onLeave, false, 0, true);
            // The window is CLIK's modal clip: a click must hand it the keys.
            tf.addEventListener(MouseEvent.MOUSE_DOWN, onPress, false, 0, true);
            return tf;
        }

        private function coin(x:Number, y:Number):MovieClip
        {
            var cls:Class = getDefinitionByName("CoinsIcon2") as Class;
            var icon:MovieClip = new cls() as MovieClip;
            icon.x = x;
            icon.y = y;
            icon.mouseEnabled = false;
            icon.mouseChildren = false;
            icon.gotoAndStop(1);
            addChild(icon);
            return icon;
        }

        // "v^": a small button in the skin of EXCHANGE / CANCEL between the
        // two panels, a down and an up arrow drawn on it. Made in configUI,
        // after Message has measured the window to centre it: made in the
        // constructor, it threw the measurement off and the window opened in
        // the corner.
        private function swapButton(cx:Number, cy:Number):void
        {
            var cls:Class = getDefinitionByName("SrvButtonScale") as Class;
            var button:* = new cls();
            button.label = "";
            button.setSize(46, 28);
            button.x = cx - 23;
            button.y = cy - 14;
            button.tabEnabled = false;
            button.focusable = false;
            button.addEventListener(MouseEvent.CLICK, onSwap, false, 0, true);
            addChild(button);
            var arrows:Shape = new Shape();
            arrows.graphics.lineStyle(1.5, 0xD8D5D0, 1, false, "normal", "none");
            arrow(arrows, -5, 1);
            arrow(arrows, 5, -1);
            arrows.x = cx;
            arrows.y = cy;
            addChild(arrows);
        }

        // A shaft and a head, pointing down (way 1) or up (way -1).
        private function arrow(shape:Shape, x:Number, way:Number):void
        {
            shape.graphics.moveTo(x, -5 * way);
            shape.graphics.lineTo(x, 3 * way);
            shape.graphics.lineStyle();
            shape.graphics.beginFill(0xD8D5D0);
            shape.graphics.moveTo(x - 3.5, 2 * way);
            shape.graphics.lineTo(x + 3.5, 2 * way);
            shape.graphics.lineTo(x, 6.5 * way);
            shape.graphics.lineTo(x - 3.5, 2 * way);
            shape.graphics.endFill();
            shape.graphics.lineStyle(1.5, 0xD8D5D0, 1, false, "normal", "none");
        }

        private function configure():void
        {
            var cancel:String = LocalizationData.getName("cancel");
            for (var i:int = 0; i < CANCEL.length; i++)
                if (cancel == CANCEL[i])
                    _lang = i;
            if (!_swap)
            {
                _swap = true;
                swapButton(W / 2, 125);
            }
            var data:Object = _menu.getMessageData();
            _payGold = !(data != null && data.srpBuyGold == true);
            _gold = 1;
            var stepper:* = _menu.exchangeStepper;
            if (stepper != null)
            {
                stepper.enabled = false;
                stepper.visible = false;
                stepper.focusable = false;
            }
            _menu.textFieldLabel.text = say("title");
            _menu.btnAccept.label = say("button").toUpperCase();
            _payLabel.text = say("pay");
            _getLabel.text = say("receive");
            snap();
            refresh();
        }

        private function say(key:String):String
        {
            // The game's own word where the movie can read it: "Silver" is
            // st_regular_money (flash key regular_money). TEXT keeps the
            // same words, from the game's files, for a client without it.
            if (key == "silver")
            {
                var silver:String = LocalizationData.getName("regular_money");
                // getName gives "#regular_money" for a key it does not have.
                if (silver && silver.charAt(0) != "#")
                    return silver;
            }
            return TEXT[key][_lang];
        }

        private function rate():uint
        {
            return PlayerInfo.premiumExchangeRate > 0 ? PlayerInfo.premiumExchangeRate : 100;
        }

        // Util.getMoneyString groups by threes and leaves a space at the end.
        private static function money(value:Number):String
        {
            var text:String = Util.getMoneyString(value);
            while (text.length > 0 && text.charAt(text.length - 1) == " ")
                text = text.substr(0, text.length - 1);
            return text;
        }

        private function goldField():TextField
        {
            return _payGold ? _payField : _getField;
        }

        private function silverField():TextField
        {
            return _payGold ? _getField : _payField;
        }

        private function limit(gold:Number):uint
        {
            var most:Number = Math.min(MAX_GOLD, Math.floor(uint.MAX_VALUE / rate()), affordable());
            return uint(Math.max(0, Math.min(most, Math.floor(gold))));
        }

        // The most gold the Pay balance covers: all the gold to sell, or the
        // whole gold the silver buys.
        private function affordable():uint
        {
            return _payGold ? PlayerInfo.premiumMoney : uint(Math.floor(PlayerInfo.genericMoney / rate()));
        }

        // More than the balance pays for becomes the most it does, at once.
        private function onEdit(e:Event):void
        {
            var source:TextField = e.target as TextField;
            var typed:Number = source.text.length > 0 ? Number(source.text) : 0;
            if (source == goldField())
            {
                _gold = limit(typed);
                if (_gold != typed)
                    retype(source, _gold);
                silverField().text = String(_gold * rate());
            }
            else
            {
                // Silver goes by whole gold: the field is evened out on leaving.
                _gold = limit(typed / rate());
                if (typed > limit(Number.POSITIVE_INFINITY) * rate())
                    retype(source, _gold * rate());
                goldField().text = String(_gold);
            }
            refresh();
        }

        private static function retype(field:TextField, value:Number):void
        {
            field.text = String(value);
            field.setSelection(field.text.length, field.text.length);
        }

        private function onPress(e:MouseEvent):void
        {
            FocusManager.setFocus(e.currentTarget as TextField);
        }

        private function onLeave(e:FocusEvent):void
        {
            snap();
        }

        private function onSwap(e:MouseEvent):void
        {
            _payGold = !_payGold;
            snap();
            refresh();
        }

        private function snap():void
        {
            goldField().text = String(_gold);
            silverField().text = String(_gold * rate());
        }

        private function refresh():void
        {
            var silver:String = say("silver");
            var gold:String = say("gold");
            _payName.text = _payGold ? gold : silver;
            _getName.text = _payGold ? silver : gold;
            _payCoin.gotoAndStop(_payGold ? 2 : 1);
            _getCoin.gotoAndStop(_payGold ? 1 : 2);
            var payHas:uint = _payGold ? PlayerInfo.premiumMoney : PlayerInfo.genericMoney;
            var getHas:uint = _payGold ? PlayerInfo.genericMoney : PlayerInfo.premiumMoney;
            _payBalance.text = say("balance") + ": " + money(payHas);
            _getBalance.text = say("balance") + ": " + money(getHas);
            var each:String = money(rate());
            _rate.text = _payGold ? "1 " + gold + " = " + each + " " + silver
                                  : each + " " + silver + " = 1 " + gold;
            var cost:Number = _payGold ? _gold : _gold * rate();
            var short:Boolean = cost > payHas;
            _note.text = short ? say(_payGold ? "short_gold" : "short_silver") : "";
            _menu.btnAccept.enabled = _gold > 0 && !short;
        }
    }
}
