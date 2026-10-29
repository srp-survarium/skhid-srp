package survarium.messages
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
   import scaleform.clik.events.ButtonEvent;
   import scaleform.gfx.FocusManager;
   import survarium.Util;
   import survarium.controls.Message;
   import survarium.controls.NumericStepperExchange;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;

   // The stock "gold for silver" window rebuilt as an exchange both ways:
   // Pay above, Receive below, the swap between them, the rate under them.
   // Typing in either field updates the other. The game's own pieces are
   // reused: the window, its buttons and their skin, the coin icons and the
   // font of its texts.
   public class MessageMoneyExchange extends MessageAccept
   {

      // `47 04` amounts from here on buy gold with silver (srp's own); below
      // it they are the stock "this much gold for silver".
      public static const BUY_GOLD:uint = 0x40000000;

      private static const W:Number = 300;

      private static const H:Number = 300;

      // The stock window: 288 wide, its buttons 47 above its bottom.
      private static const STOCK_W:Number = 288;

      private static const BUTTONS_UP:Number = 47;

      private static const MAX_GOLD:uint = 10000000;

      // The silver of one gold when the exchange rate is not known yet.
      private static const FALLBACK_RATE:int = 100;

      private static const TEXT:Object = {
         "title":["Exchange","Обмін","Обмен","Tausch","Wymiana", "Razmeni", "Směnit", "Átváltás", "Vaihda", "Абмен"],
         "pay":["Pay","Віддаєте","Отдаёте","Sie zahlen","Płacisz", "Plati", "Zaplatíš", "Fizetsz", "Maksat", "Аддаяце"],
         "receive":["Receive","Отримуєте","Получаете","Sie erhalten","Otrzymujesz", "Primi", "Dostaneš", "Kapsz", "Saat", "Атрымліваеце"],
         "silver":["Silver","Срібло","Серебро","Silber","Srebro", "Srebro", "Stříbro", "Ezüst", "Hopea", "Срэбра"],
         "gold":["Gold","Золото","Золото","Gold","Złoto", "Zlato", "Zlato", "Arany", "Kulta", "Золата"],
         "balance":["Balance","Баланс","Баланс","Guthaben","Saldo", "Stanje", "Zůstatek", "Egyenleg", "Saldo", "Баланс"],
         "short_silver":["Not enough silver","Недостатньо срібла","Недостаточно серебра","Nicht genug Silber","Za mało srebra", "Nema dovoljno srebra", "Nedostatek stříbra", "Nincs elég ezüst", "Ei tarpeeksi hopeaa", "Недастаткова срэбра"],
         "short_gold":["Not enough gold","Недостатньо золота","Недостаточно золота","Nicht genug Gold","Za mało złota", "Nema dovoljno zlata", "Nedostatek zlata", "Nincs elég arany", "Ei tarpeeksi kultaa", "Недастаткова золата"],
         "button":["Exchange","Обміняти","Обменять","Tauschen","Wymień", "Razmeni", "Směnit", "Átváltás", "Vaihda", "Абмяняць"]
      };

      // The client's own "cancel" names its language.
      private static const CANCEL:Array = ["Cancel","Скасувати","Отмена","Abrechnen","Anuluj", "Otkaži", "Zrušit", "Mégse", "Peruuta", "Скасаваць"];

      public var textFieldLabel:TextField;

      public var exchangeStepper:NumericStepperExchange;

      // The exchange's own parts, over the stock window's.
      private var _board:Sprite;

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

      // Every "Get Gold" of the lobby opened the online store, which is gone:
      // it opens this window, to buy gold with silver. Shown at once over a
      // window still open (without the third argument it would wait for
      // that window to close), which stays visible under it (the fifth), as
      // the premium windows' own dialogs do; it has the focus again when
      // this one closes.
      public static function buyGold() : void
      {
         Message.showMessage("exchange",{
            label:"",
            srpBuyGold:true
         },true,0,true);
      }

      // From the top bar's gold button (buy gold) and silver button (sell gold).
      public static function open(buy:Boolean) : void
      {
         Message.showMessage("exchange",{
            label:"",
            srpBuyGold:buy
         });
      }

      public function MessageMoneyExchange()
      {
         super();
         // Made here: the size is final before Message measures the window
         // to centre it.
         this.buildExchange();
      }

      override protected function configUI() : void
      {
         var hasPremiumMoney:Boolean;
         super.configUI();
         this.exchangeStepper.setExchangeMoneyType(2);
         this.exchangeStepper.setExchangeMoneyFactor(PlayerInfo.premiumExchangeRate);
         hasPremiumMoney = PlayerInfo.premiumMoney > 0;
         this.exchangeStepper.minimum = hasPremiumMoney ? 1 : 0;
         this.exchangeStepper.value = this.exchangeStepper.minimum;
         this.exchangeStepper.maximum = Math.min(uint.MAX_VALUE,PlayerInfo.premiumMoney);
         this.exchangeStepper.editable = true;
         this.exchangeStepper.stepSize = 1;
         this.exchangeStepper.enabled = hasPremiumMoney;
         btnAccept.label = LocalizationData.getName("exchange_button");
         btnAccept.enabled = hasPremiumMoney;
         this.textFieldLabel.text = _data.label;
         this.configureExchange();
      }

      // The Exchange button and Enter.
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         this.snap();
         this.refresh();
         if(!btnAccept.enabled)
         {
            return;
         }
         // A uint, as the stock window sends it: the client reads it as one.
         var amount:uint = this._payGold ? this._gold : BUY_GOLD + this._gold;
         ExternalInterface.call("exchange_premium_money",amount);
         Message.onClick(getMessageId());
      }

      private function buildExchange() : void
      {
         this._board = new Sprite();
         var stock:TextFormat = this.textFieldLabel.getTextFormat();
         this._font = stock.font;
         this._bold = stock.bold;
         for(var i:int = 0; i < numChildren; i++)
         {
            var child:DisplayObject = getChildAt(i);
            if(getQualifiedClassName(child) == "MessageSmallBackground")
            {
               child.width = W;
               child.height = H;
            }
         }
         var dx:Number = (W - STOCK_W) / 2;
         btnAccept.x += dx;
         btnCancel.x += dx;
         btnAccept.y = H - BUTTONS_UP;
         btnCancel.y = H - BUTTONS_UP;
         this.textFieldLabel.x = 8;
         this.textFieldLabel.width = W - 16;
         this.textFieldLabel.y = 14;
         this.panel(14,50);
         this._payLabel = this.text(24,56,120,20,14,0x8D8A91,"left");
         this._payBalance = this.text(146,56,130,20,14,0x8D8A91,"right");
         this._payField = this.field(22,78);
         this._payCoin = this.coin(200,85);
         this._payName = this.text(222,82,66,26,18,0xB6B6B6,"left");
         this.panel(14,130);
         this._getLabel = this.text(24,136,120,20,14,0x8D8A91,"left");
         this._getBalance = this.text(146,136,130,20,14,0x8D8A91,"right");
         this._getField = this.field(22,158);
         this._getCoin = this.coin(200,165);
         this._getName = this.text(222,162,66,26,18,0xB6B6B6,"left");
         this._rate = this.text(14,206,W - 28,22,15,0xB6B6B6,"center");
         this._note = this.text(14,228,W - 28,22,15,0xD64531,"center");
         addChild(this._board);
      }

      // The dark box of the stock stepper: black at 40 %, a faint light
      // edge right and below.
      private function panel(x:Number, y:Number) : void
      {
         this._board.graphics.beginFill(0x000000,0.404);
         this._board.graphics.drawRect(x,y,W - 2 * x,70);
         this._board.graphics.endFill();
         this._board.graphics.beginFill(0xD4D4D4,0.047);
         this._board.graphics.drawRect(x,y + 69,W - 2 * x,1);
         this._board.graphics.drawRect(W - x - 1,y,1,69);
         this._board.graphics.endFill();
      }

      private function text(x:Number, y:Number, w:Number, h:Number, size:Number, color:uint, align:String) : TextField
      {
         var tf:TextField = new TextField();
         var format:TextFormat = new TextFormat(this._font,size,color,this._bold);
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
         this._board.addChild(tf);
         return tf;
      }

      private function field(x:Number, y:Number) : TextField
      {
         var tf:TextField = this.text(x,y,168,34,26,0xF4B717,"left");
         tf.type = TextFieldType.INPUT;
         tf.restrict = "0-9";
         tf.maxChars = 10;
         tf.selectable = true;
         tf.mouseEnabled = true;
         tf.tabEnabled = true;
         tf.addEventListener(Event.CHANGE,this.onEdit,false,0,true);
         tf.addEventListener(FocusEvent.FOCUS_OUT,this.onLeave,false,0,true);
         // The window is CLIK's modal clip: a click must hand it the keys.
         tf.addEventListener(MouseEvent.MOUSE_DOWN,this.onPress,false,0,true);
         return tf;
      }

      private function coin(x:Number, y:Number) : MovieClip
      {
         var cls:Class = getDefinitionByName("CoinsIcon2") as Class;
         var icon:MovieClip = new cls() as MovieClip;
         icon.x = x;
         icon.y = y;
         icon.mouseEnabled = false;
         icon.mouseChildren = false;
         icon.gotoAndStop(1);
         this._board.addChild(icon);
         return icon;
      }

      // "v^": a small button in the skin of EXCHANGE / CANCEL between the
      // two panels, a down and an up arrow drawn on it. Made in configUI,
      // after Message has measured the window to centre it: made in the
      // constructor, it threw the measurement off and the window opened in
      // the corner.
      private function swapButton(cx:Number, cy:Number) : void
      {
         var cls:Class = getDefinitionByName("SrvButtonScale") as Class;
         var button:* = new cls();
         button.label = "";
         button.setSize(46,28);
         button.x = cx - 23;
         button.y = cy - 14;
         button.tabEnabled = false;
         button.focusable = false;
         button.addEventListener(MouseEvent.CLICK,this.onSwap,false,0,true);
         this._board.addChild(button);
         var arrows:Shape = new Shape();
         arrows.graphics.lineStyle(1.5,0xD8D5D0,1,false,"normal","none");
         this.arrow(arrows,-5,1);
         this.arrow(arrows,5,-1);
         arrows.x = cx;
         arrows.y = cy;
         this._board.addChild(arrows);
      }

      // A shaft and a head, pointing down (way 1) or up (way -1).
      private function arrow(shape:Shape, x:Number, way:Number) : void
      {
         shape.graphics.moveTo(x,-5 * way);
         shape.graphics.lineTo(x,3 * way);
         shape.graphics.lineStyle();
         shape.graphics.beginFill(0xD8D5D0);
         shape.graphics.moveTo(x - 3.5,2 * way);
         shape.graphics.lineTo(x + 3.5,2 * way);
         shape.graphics.lineTo(x,6.5 * way);
         shape.graphics.lineTo(x - 3.5,2 * way);
         shape.graphics.endFill();
         shape.graphics.lineStyle(1.5,0xD8D5D0,1,false,"normal","none");
      }

      // After the stock set-up: the stock stepper hidden, the exchange's
      // words, Pay gold (sell) or silver (buy gold).
      private function configureExchange() : void
      {
         var cancel:String = LocalizationData.getName("cancel");
         for(var i:int = 0; i < CANCEL.length; i++)
         {
            if(cancel == CANCEL[i])
            {
               this._lang = i;
            }
         }
         if(!this._swap)
         {
            this._swap = true;
            this.swapButton(W / 2,125);
         }
         var data:Object = getMessageData();
         this._payGold = !(data != null && data.srpBuyGold == true);
         this._gold = 1;
         var stepper:* = this.exchangeStepper;
         if(stepper != null)
         {
            stepper.enabled = false;
            stepper.visible = false;
            stepper.focusable = false;
         }
         this.textFieldLabel.text = this.say("title");
         btnAccept.label = this.say("button").toUpperCase();
         this._payLabel.text = this.say("pay");
         this._getLabel.text = this.say("receive");
         this.snap();
         this.refresh();
      }

      private function say(key:String) : String
      {
         // The game's own word where the movie can read it: "Silver" is
         // st_regular_money (flash key regular_money). TEXT keeps the
         // same words, from the game's files, for a client without it.
         if(key == "silver")
         {
            var silver:String = LocalizationData.getName("regular_money");
            // getName gives "#regular_money" for a key it does not have.
            if((silver) && silver.charAt(0) != "#")
            {
               return silver;
            }
         }
         return TEXT[key][this._lang];
      }

      private function rate() : uint
      {
         return PlayerInfo.premiumExchangeRate > 0 ? PlayerInfo.premiumExchangeRate : FALLBACK_RATE;
      }

      // Util.getMoneyString groups by threes and leaves a space at the end.
      private static function money(value:Number) : String
      {
         var text:String = Util.getMoneyString(value);
         while(text.length > 0 && text.charAt(text.length - 1) == " ")
         {
            text = text.substr(0,text.length - 1);
         }
         return text;
      }

      private function goldField() : TextField
      {
         return this._payGold ? this._payField : this._getField;
      }

      private function silverField() : TextField
      {
         return this._payGold ? this._getField : this._payField;
      }

      private function limit(gold:Number) : uint
      {
         var most:Number = Math.min(MAX_GOLD,Math.floor(uint.MAX_VALUE / this.rate()),this.affordable());
         return uint(Math.max(0,Math.min(most,Math.floor(gold))));
      }

      // The most gold the Pay balance covers: all the gold to sell, or the
      // whole gold the silver buys.
      private function affordable() : uint
      {
         return this._payGold ? PlayerInfo.premiumMoney : uint(Math.floor(PlayerInfo.genericMoney / this.rate()));
      }

      // More than the balance pays for becomes the most it does, at once.
      private function onEdit(e:Event) : void
      {
         var source:TextField = e.target as TextField;
         var typed:Number = source.text.length > 0 ? Number(source.text) : 0;
         if(source == this.goldField())
         {
            this._gold = this.limit(typed);
            if(this._gold != typed)
            {
               retype(source,this._gold);
            }
            this.silverField().text = String(this._gold * this.rate());
         }
         else
         {
            // Silver goes by whole gold: the field is evened out on leaving.
            this._gold = this.limit(typed / this.rate());
            if(typed > this.limit(Number.POSITIVE_INFINITY) * this.rate())
            {
               retype(source,this._gold * this.rate());
            }
            this.goldField().text = String(this._gold);
         }
         this.refresh();
      }

      private static function retype(field:TextField, value:Number) : void
      {
         field.text = String(value);
         field.setSelection(field.text.length,field.text.length);
      }

      private function onPress(e:MouseEvent) : void
      {
         FocusManager.setFocus(e.currentTarget as TextField);
      }

      private function onLeave(e:FocusEvent) : void
      {
         this.snap();
      }

      private function onSwap(e:MouseEvent) : void
      {
         this._payGold = !this._payGold;
         this.snap();
         this.refresh();
      }

      private function snap() : void
      {
         this.goldField().text = String(this._gold);
         this.silverField().text = String(this._gold * this.rate());
      }

      private function refresh() : void
      {
         var silver:String = this.say("silver");
         var gold:String = this.say("gold");
         this._payName.text = this._payGold ? gold : silver;
         this._getName.text = this._payGold ? silver : gold;
         this._payCoin.gotoAndStop(this._payGold ? 2 : 1);
         this._getCoin.gotoAndStop(this._payGold ? 1 : 2);
         var payHas:uint = this._payGold ? PlayerInfo.premiumMoney : PlayerInfo.genericMoney;
         var getHas:uint = this._payGold ? PlayerInfo.genericMoney : PlayerInfo.premiumMoney;
         this._payBalance.text = this.say("balance") + ": " + money(payHas);
         this._getBalance.text = this.say("balance") + ": " + money(getHas);
         var each:String = money(this.rate());
         this._rate.text = this._payGold ? "1 " + gold + " = " + each + " " + silver : each + " " + silver + " = 1 " + gold;
         var cost:Number = this._payGold ? this._gold : this._gold * this.rate();
         var short:Boolean = cost > payHas;
         this._note.text = short ? this.say(this._payGold ? "short_gold" : "short_silver") : "";
         btnAccept.enabled = this._gold > 0 && !short;
      }
   }
}

