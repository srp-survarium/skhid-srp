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
      
      public static function set commander(param1:Boolean) : void
      {
         _commander = param1;
      }
      
      public static function get commander() : Boolean
      {
         return _commander;
      }
      
      public static function set nickname(param1:String) : void
      {
         _name = param1;
      }
      
      public static function get nickname() : String
      {
         return _name;
      }
      
      public static function getVoucherId(param1:uint) : uint
      {
         trace("getVoucherId::",param1,_vouchersType[param1] ? _vouchersType[param1].id : "noSuchVoucher");
         if(!_vouchersType[param1])
         {
            return 0;
         }
         return _vouchersType[param1].id;
      }
      
      public static function getVoucherCount(param1:uint) : uint
      {
         trace("getVoucherCount::",param1,_vouchersType[param1] ? _vouchersType[param1].count : "noSuchVoucher");
         if(!_vouchersType[param1])
         {
            return 0;
         }
         return _vouchersType[param1].count;
      }
      
      public static function getVoucherDictId(param1:uint) : uint
      {
         if(!_vouchersType[param1])
         {
            return 0;
         }
         trace("getVoucherDictId::",param1,_vouchersType[param1].dictId);
         return _vouchersType[param1].dictId;
      }
      
      public static function setVoucherCount(param1:uint, param2:uint, param3:uint, param4:uint) : void
      {
         var _loc5_:Object;
         trace("setVoucherCount::",param1,param2,param3,param4);
         if(!_vouchersDictId[param1])
         {
            _loc5_ = {
               id:param4,
               type:param3,
               count:param2,
               dictId:param1
            };
            trace("createVaucher::",param3,param1,param4);
            _vouchersDictId[param1] = _loc5_;
            _vouchersType[param3] = _loc5_;
         }
         else
         {
            if(param4 && _vouchersDictId[param1].id != param4)
            {
               trace("updateVoucherId::",param4);
               _vouchersDictId[param1].id = param4;
            }
            _vouchersDictId[param1].count = param2;
         }
      }
      
      public static function setVouchers(param1:Array) : void
      {
         var _loc2_:uint = param1.length;
         var _loc3_:uint = 0;
         var _loc4_:uint;
         var _loc5_:uint;
         var _loc6_:Object;
         while(_loc3_ < _loc2_)
         {
            _loc4_ = (param1[_loc3_].type);
            _loc5_ = (param1[_loc3_].dictId);
            _loc6_ = {
               type:_loc4_,
               count:0,
               dictId:_loc5_
            };
            trace("createVaucher:;",_loc5_,_loc4_);
            _vouchersDictId[_loc5_] = _loc6_;
            _vouchersType[_loc4_] = _loc6_;
            _loc3_++;
         }
      }
      
      public function set premium_money_new(param1:int) : void
      {
         _coinsPNew = 5000;
         this.outTimerP.reset();
         this.outTimerP.start();
         this.playerCoinsPNew.alpha = 0;
         this.playerCoinsPNew.x = 124;
         this.playerCoinsPNew.y = 48;
         this.playerCoinsPNew.textColor = param1 < 0 ? 13369344 : 6749952;
         new Tween(500,this.playerCoinsPNew,{
            x:124 + 15,
            alpha:1
         },{ease:Strong.easeOut});
         if(param1 < 0)
         {
            this.playerCoinsPNew.text = param1.toString();
         }
         else
         {
            this.playerCoinsPNew.text = "+" + param1.toString();
         }
      }
      
      public function set generic_money_new(param1:int) : void
      {
         _coinsNew = 5000;
         this.outTimer.reset();
         this.outTimer.start();
         this.playerCoinsNew.x = 124;
         this.playerCoinsNew.y = 23;
         this.playerCoinsNew.alpha = 0;
         this.playerCoinsNew.textColor = param1 < 0 ? 13369344 : 6749952;
         new Tween(500,this.playerCoinsNew,{
            x:124 + 15,
            alpha:1
         },{ease:Strong.easeOut});
         if(param1 < 0)
         {
            this.playerCoinsNew.text = param1.toString();
         }
         else
         {
            this.playerCoinsNew.text = "+" + param1.toString();
         }
      }
      
      public function set premium_money(param1:int) : void
      {
         _coinsP = param1;
         this.playerCoinsP.text = param1.toString();
      }
      
      public function get premium_money() : int
      {
         return this._coinsP;
      }
      
      public function set generic_money(param1:int) : void
      {
         _coins = param1;
         this.playerCoins.text = param1.toString();
      }
      
      public function get generic_money() : int
      {
         return this._coins;
      }
      
      public function setData(param1:Object) : void
      {
         nickname = param1.nickname;
         premiumMoney = param1.premium_money;
         genericMoney = param1.generic_money;
         if(param1.premium_money_new != undefined && this.playerCoinsP.text != "")
         {
            premium_money_new = param1.premium_money_new;
         }
         else if(this.playerCoinsP.text != "" && param1.premium_money - this._coinsP != 0)
         {
            premium_money_new = param1.premium_money - this._coinsP;
         }
         if(param1.generic_money_new != undefined && this.playerCoins.text != "")
         {
            generic_money_new = param1.generic_money_new;
         }
         else if(this.playerCoins.text != "" && param1.generic_money - this._coins != 0)
         {
            generic_money_new = param1.generic_money - this._coins;
         }
         premium_money = param1.premium_money;
         generic_money = param1.generic_money;
         dispatchEvent(new Event(Event.CHANGE));
      }
      
      protected function onTimerComplete(param1:Event) : void
      {
         if(param1.target == this.outTimer)
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

