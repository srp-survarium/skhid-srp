package survarium.controls
{
   import fl.transitions.easing.Strong;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.TimerEvent;
   import flash.text.TextField;
   import flash.utils.Dictionary;
   import flash.utils.Timer;
   import scaleform.clik.layout.LayoutData;
   import scaleform.clik.motion.Tween;
   
   public class PlayerInfo extends MovieClip
   {
      
      protected static var _name:String = "";
      
      protected static var _commander:Boolean = true;
      
      public static const MAX_FREE_ITEMS_COUNT:int = 1000;
      
      public static var maxWeight:Number = 0;
      
      public static var curWeight:Number = 0;
      
      public static var experienceCurrent:int = 0;
      
      public static var experienceNext:int = 0;
      
      public static var ammoBags:int = 0;
      
      public static var pocketSize:int = 0;
      
      public static var currentLevel:int = 0;
      
      public static var activeFaction:int = 0;
      
      public static var clanState:uint = 0;
      
      public static var premiumMoney:uint = 0;
      
      public static var genericMoney:uint = 0;
      
      public static var premiumState:uint = 0;
      
      public static var premiumExchangeRate:uint = 0;
      
      public static var premiumLeft:String;
      
      public static var questRewardsMultiplierReputation:Number = 1;
      
      public static var questRewardsMultiplierMoney:Number = 1;
      
      public static var isInTeam:Boolean = false;
      
      public static var isCommander:Boolean = false;
      
      public static var isInSquad:Boolean = false;
      
      public static var isSquadFull:Boolean = false;
      
      public static var isTeamCommander:Boolean = false;
      
      public static var isClanIviter:Boolean = false;
      
      public static var isClanManager:Boolean = false;
      
      protected static var _vouchersDictId:Dictionary = new Dictionary();
      
      protected static var _vouchersType:Dictionary = new Dictionary();
      
      protected var _coins:int;
      
      protected var _coinsP:int;
      
      protected var _coinsNew:int = 0;
      
      protected var _coinsPNew:int = 0;
      
      protected var outTimer:Timer = new Timer(2000,1);
      
      protected var outTimerP:Timer = new Timer(2000,1);
      
      public var layoutData:LayoutData;
      
      public var playerName:TextField;
      
      public var playerCoins:TextField;
      
      public var playerCoinsP:TextField;
      
      public var playerCoinsNew:TextField;
      
      public var playerCoinsPNew:TextField;
      
      public var _goldIcon:MovieClip;
      
      public var _picon:MovieClip;
      
      public function PlayerInfo()
      {
         super();
         this.outTimer.addEventListener(TimerEvent.TIMER_COMPLETE,this.onTimerComplete);
         this.outTimerP.addEventListener(TimerEvent.TIMER_COMPLETE,this.onTimerComplete);
      }
      
      public static function set commander(value:Boolean) : void
      {
         _commander = value;
      }
      
      public static function get commander() : Boolean
      {
         return _commander;
      }
      
      public static function set nickname(value:String) : void
      {
         _name = value;
      }
      
      public static function get nickname() : String
      {
         return _name;
      }
      
      public static function getVoucherId(voucherType:uint) : uint
      {
         trace("getVoucherId::",voucherType,_vouchersType[voucherType] ? _vouchersType[voucherType].id : "noSuchVoucher");
         if(!_vouchersType[voucherType])
         {
            return 0;
         }
         return _vouchersType[voucherType].id;
      }
      
      public static function getVoucherCount(voucherType:uint) : uint
      {
         trace("getVoucherCount::",voucherType,_vouchersType[voucherType] ? _vouchersType[voucherType].count : "noSuchVoucher");
         if(!_vouchersType[voucherType])
         {
            return 0;
         }
         return _vouchersType[voucherType].count;
      }
      
      public static function getVoucherDictId(voucherType:uint) : uint
      {
         if(!_vouchersType[voucherType])
         {
            return 0;
         }
         trace("getVoucherDictId::",voucherType,_vouchersType[voucherType].dictId);
         return _vouchersType[voucherType].dictId;
      }
      
      public static function setVoucherCount(voucherDictId:uint, voucherCount:uint, voucherType:uint, voucherId:uint) : void
      {
         var voucher:Object;
         trace("setVoucherCount::",voucherDictId,voucherCount,voucherType,voucherId);
         if(!_vouchersDictId[voucherDictId])
         {
            voucher = {
               id:voucherId,
               type:voucherType,
               count:voucherCount,
               dictId:voucherDictId
            };
            trace("createVaucher::",voucherType,voucherDictId,voucherId);
            _vouchersDictId[voucherDictId] = voucher;
            _vouchersType[voucherType] = voucher;
         }
         else
         {
            if(voucherId && _vouchersDictId[voucherDictId].id != voucherId)
            {
               trace("updateVoucherId::",voucherId);
               _vouchersDictId[voucherDictId].id = voucherId;
            }
            _vouchersDictId[voucherDictId].count = voucherCount;
         }
      }
      
      public static function setVouchers(vouchers:Array) : void
      {
         var total:uint = vouchers.length;
         var i:uint = 0;
         var voucherType:uint;
         var voucherDictId:uint;
         var voucher:Object;
         while(i < total)
         {
            voucherType = (vouchers[i].type);
            voucherDictId = (vouchers[i].dictId);
            voucher = {
               type:voucherType,
               count:0,
               dictId:voucherDictId
            };
            trace("createVaucher:;",voucherDictId,voucherType);
            _vouchersDictId[voucherDictId] = voucher;
            _vouchersType[voucherType] = voucher;
            i++;
         }
      }
      
      public function set premium_money_new(value:int) : void
      {
         _coinsPNew = 5000;
         this.outTimerP.reset();
         this.outTimerP.start();
         this.playerCoinsPNew.alpha = 0;
         this.playerCoinsPNew.x = 124;
         this.playerCoinsPNew.y = 48;
         this.playerCoinsPNew.textColor = value < 0 ? 13369344 : 6749952;
         new Tween(500,this.playerCoinsPNew,{
            x:124 + 15,
            alpha:1
         },{ease:Strong.easeOut});
         if(value < 0)
         {
            this.playerCoinsPNew.text = value.toString();
         }
         else
         {
            this.playerCoinsPNew.text = "+" + value.toString();
         }
      }
      
      public function set generic_money_new(value:int) : void
      {
         _coinsNew = 5000;
         this.outTimer.reset();
         this.outTimer.start();
         this.playerCoinsNew.x = 124;
         this.playerCoinsNew.y = 23;
         this.playerCoinsNew.alpha = 0;
         this.playerCoinsNew.textColor = value < 0 ? 13369344 : 6749952;
         new Tween(500,this.playerCoinsNew,{
            x:124 + 15,
            alpha:1
         },{ease:Strong.easeOut});
         if(value < 0)
         {
            this.playerCoinsNew.text = value.toString();
         }
         else
         {
            this.playerCoinsNew.text = "+" + value.toString();
         }
      }
      
      public function set premium_money(value:int) : void
      {
         _coinsP = value;
         this.playerCoinsP.text = value.toString();
      }
      
      public function get premium_money() : int
      {
         return this._coinsP;
      }
      
      public function set generic_money(value:int) : void
      {
         _coins = value;
         this.playerCoins.text = value.toString();
      }
      
      public function get generic_money() : int
      {
         return this._coins;
      }
      
      public function setData(newData:Object) : void
      {
         nickname = newData.nickname;
         premiumMoney = newData.premium_money;
         genericMoney = newData.generic_money;
         if(newData.premium_money_new != undefined && this.playerCoinsP.text != "")
         {
            premium_money_new = newData.premium_money_new;
         }
         else if(this.playerCoinsP.text != "" && newData.premium_money - this._coinsP != 0)
         {
            premium_money_new = newData.premium_money - this._coinsP;
         }
         if(newData.generic_money_new != undefined && this.playerCoins.text != "")
         {
            generic_money_new = newData.generic_money_new;
         }
         else if(this.playerCoins.text != "" && newData.generic_money - this._coins != 0)
         {
            generic_money_new = newData.generic_money - this._coins;
         }
         premium_money = newData.premium_money;
         generic_money = newData.generic_money;
         dispatchEvent(new Event(Event.CHANGE));
      }
      
      protected function onTimerComplete(event:Event) : void
      {
         if(event.target == this.outTimer)
         {
            this.playerCoinsNew.alpha = 1;
            new Tween(500,this.playerCoinsNew,{
               x:124 + 30,
               alpha:0
            },{ease:Strong.easeOut});
         }
         else
         {
            this.playerCoinsPNew.alpha = 1;
            new Tween(500,this.playerCoinsPNew,{
               x:124 + 30,
               alpha:0
            },{ease:Strong.easeOut});
         }
      }
   }
}

