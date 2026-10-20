package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.utils.Dictionary;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.ScrollingList;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import survarium.data.LocalizationData;
   import survarium.gsearch.MatchMakingSettings;
   
   public class PlayerRanking extends UIWindow
   {
      
      public var playerPlace:MovieClip;
      
      public var btnClanRank:Button;
      
      public var list:ScrollingList;
      
      public var tabBar:ButtonBar;
      
      public var txPlace:TextField;
      
      public var txRanking:TextField;
      
      public var txPlace2:TextField;
      
      public var txRanking2:TextField;
      
      public var txClan2:TextField;
      
      public var txNickname:TextField;
      
      public var txLabel:TextField;
      
      protected var _player_data:Object;
      
      protected var _playersCache:Dictionary = new Dictionary();
      
      protected var _topLeague:Array = [];
      
      protected var _myLeague:Array = [];
      
      protected var _requestList:Array = new Array();
      
      protected var _additionalRequest:int = 5;
      
      protected var _minimumScrollUpdate:int = 3;
      
      protected var _lastScroll:int = 0;
      
      protected var _start_index:int = 0;
      
      protected var _request_index:int = 0;
      
      protected var _totalPlayers:uint = 0;
      
      protected const _total_renderers:int = 20;
      
      public function PlayerRanking()
      {
         super();
         LocalizationData.addTextField(this.txPlace,"place");
         LocalizationData.addTextField(this.txNickname,"nickname");
         LocalizationData.addTextField(this.txRanking,"ranking");
         LocalizationData.addTextField(this.txLabel,"ranking");
         LocalizationData.addTextField(this.txPlace2,"place");
         LocalizationData.addTextField(this.txRanking2,"ranking");
         LocalizationData.addTextField(this.txClan2,"clan_rank");
         LocalizationData.addCallback(this.onLocal,"ranking");
         LocalizationData.addButton(this.btnClanRank,"clan_rank");
         this.btnClanRank.enabled = false;
         this.btnClanRank.x += 20;
         this.btnClanRank.alpha = 0.5;
         this.playerPlace.background.gotoAndStop(1);
      }
      
      public function set totalPlayers(param1:int) : void
      {
         _totalPlayers = param1;
         this._topLeague.length = Math.min(100,this._totalPlayers);
         var _loc2_:int = this._start_index + this.list.scrollPosition;
         this.show_players_list(this._request_index,this._total_renderers,_loc2_,false);
         invalidate(InvalidationType.SETTINGS);
      }
      
      public function set player_data(param1:Object) : void
      {
         if(param1)
         {
            param1.is_local = true;
         }
         _player_data = param1;
         this.playerPlace.data = this._player_data;
      }
      
      public function fillPlayers(param1:Array) : void
      {
         if(SrpRooms.take(param1))
         {
            return;
         }
         if(NotificationListScrollContent.replayTake(param1))
         {
            return;
         }
         if(MatchMakingSettings.pvbTake(param1))
         {
            return;
         }
         if(BottomPanel.matchesTake(param1))
         {
            return;
         }
         trace("fillPlayers:: filled",param1.length,"requested::",this._requestList.length);
         var _loc2_:uint = 0;
         var _loc3_:Array;
         var _loc4_:int;
         while(_loc2_ < param1.length)
         {
            _loc4_ = this._requestList.shift();
            this._playersCache[_loc4_] = param1[_loc2_];
            _loc2_++;
         }
         if(this.tabBar.selectedIndex == 0)
         {
            _loc3_ = this._topLeague;
         }
         else
         {
            _loc3_ = this._myLeague;
         }
         _loc2_ = 0;
         while(_loc2_ < _loc3_.length)
         {
            if(!_loc3_[_loc2_])
            {
               _loc3_[_loc2_] = this._playersCache[this._start_index + _loc2_];
            }
            _loc2_++;
         }
         this.updateLocalPlayer(_loc3_);
         this.list.dataProvider = new DataProvider(_loc3_);
      }
      
      public function onActivate() : void
      {
         _playersCache = new Dictionary();
         var _loc1_:uint = 0;
         while(_loc1_ < this._topLeague.length)
         {
            this._topLeague[_loc1_] = null;
            _loc1_++;
         }
         _loc1_ = 0;
         while(_loc1_ < this._myLeague.length)
         {
            this._myLeague[_loc1_] = null;
            _loc1_++;
         }
         this.request_players(0,100);
         if(this.tabBar.selectedIndex != 0)
         {
            this.onIndexChange(null,true);
         }
         SrpPlayerGear.ranking(this);
      }
      
      protected function onLocal(param1:String) : void
      {
         this.tabBar.dataProvider = new DataProvider([{
            label:LocalizationData.getName("top100"),
            id:0
         },{
            label:LocalizationData.getName("my_place"),
            id:1
         }]);
         this.tabBar.addEventListener(ButtonEvent.CLICK,this.onIndexChange);
         this.tabBar.selectedIndex = 0;
      }
      
      protected function onListScroll(param1:Event) : void
      {
         var _loc2_:int = Math.max(-20,Math.min(this._lastScroll - this.list.scrollPosition,20));
         _request_index = this.list.scrollPosition;
         var _loc3_:int = _loc2_ < 0 ? (this._total_renderers + _loc2_ + this._additionalRequest) : -this._additionalRequest;
         _loc2_ = Math.abs(_loc2_);
         trace("onListScroll",_loc3_,"scrolled::",_loc2_,Math.abs(_loc2_),this._minimumScrollUpdate);
         if(_loc2_ > this._minimumScrollUpdate)
         {
            _lastScroll = this.list.scrollPosition;
            this.request_players(Math.max(this._start_index + this._request_index + _loc3_,this._start_index),_loc2_);
         }
      }
      
      protected function request_players(param1:int, param2:int, param3:Boolean = false) : void
      {
         trace("request_players_init:: start:",param1,"count:",param2);
         param2 += param3 ? this._additionalRequest * 2 : 0;
         trace("request_players_count::",param1,param2,"total_players:",this._totalPlayers,"league_start:",this._start_index);
         if(param1 + param2 > this._totalPlayers)
         {
            param2 = Math.max(0,this._totalPlayers - param1);
         }
         if(param1 + param2 - this._start_index > 100)
         {
            param2 = Math.max(0,100 - (param1 - this._start_index));
         }
         if(param2 == 0)
         {
            return;
         }
         trace("request_players_st::",param1,param2);
         var _loc4_:int = -1;
         var _loc5_:int = -1;
         var _loc6_:int = 0;
         while(_loc6_ < param2)
         {
            if(!this._playersCache[param1 + _loc6_])
            {
               if(_loc4_ == -1)
               {
                  _loc4_ = _loc6_;
               }
               _loc5_ = _loc6_ + 1;
            }
            _loc6_++;
         }
         trace("request_players_end::",_loc4_,_loc5_);
         param1 += _loc4_;
         param2 = _loc5_ - _loc4_;
         if(param2 <= 0)
         {
            return;
         }
         _loc6_ = 0;
         while(_loc6_ < param2)
         {
            this._requestList.push(param1 + _loc6_);
            _loc6_++;
         }
         trace("request_players_values: ",param1,param2,"st:",this._start_index,this.list.scrollPosition,"requested::",this._requestList.length);
         ExternalInterface.call("request_players_range",param1,param2);
      }
      
      protected function onIndexChange(param1:Event, param2:Boolean = false) : void
      {
         trace("PR::change_league");
         var _loc3_:uint = 100;
         this.list.scrollBar.removeEventListener(Event.SCROLL,this.onListScroll);
         var _loc4_:int = this._start_index + this.list.scrollPosition;
         var _loc5_:int;
         if(this.tabBar.selectedIndex == 0)
         {
            this.list.scrollPosition = 0;
            _lastScroll = 0;
            _request_index = 0;
            _start_index = 0;
            this.fillLists(this._topLeague,this._start_index);
            this.updateLocalPlayer(this._topLeague);
            this.list.dataProvider = new DataProvider(this._topLeague);
         }
         else if(this.tabBar.selectedIndex == 1)
         {
            _loc5_ = (this._totalPlayers);
            if(this._player_data.place + 50 > _loc5_)
            {
               _start_index = Math.max(0,_loc5_ - _loc3_);
            }
            else
            {
               _start_index = Math.max(0,this._player_data.place - 50);
            }
            trace("init_my_league:: from:",this._start_index,"to:",this._start_index + _loc3_);
            this.list.scrollPosition = Math.max(0,this._player_data.place - this._start_index - this._total_renderers / 2);
            _lastScroll = this.list.scrollPosition;
            if(this._player_data.place + this._total_renderers / 2 > _loc5_)
            {
               _request_index = Math.max(0,_loc5_ - this._total_renderers);
            }
            else
            {
               _request_index = Math.max(0,this._player_data.place - this._total_renderers / 2);
            }
            this._myLeague.length = Math.min(_loc3_,this._totalPlayers - this._start_index);
            this.fillLists(this._myLeague,this._start_index);
            this.updateLocalPlayer(this._myLeague);
            this.list.dataProvider = new DataProvider(this._myLeague);
            this.show_players_list(this._request_index,this._total_renderers,_loc4_,param2);
            this.list.scrollBar.addEventListener(Event.SCROLL,this.onListScroll);
         }
      }
      
      protected function show_players_list(param1:int, param2:int, param3:int, param4:Boolean) : void
      {
         var _loc5_:int;
         if(param4)
         {
            _loc5_ = this._total_renderers;
         }
         else
         {
            _loc5_ = Math.max(-this._total_renderers,Math.min(param3 - param1,this._total_renderers));
         }
         var _loc6_:int = _loc5_ < 0 ? (param2 + _loc5_) : 0;
         var _loc7_:int;
         trace("show_players_list",this._start_index,"begin::",param1,param3,_loc5_,param2,"||",param3,param1);
         if(_loc5_ != 0)
         {
            _loc7_ = param1 + _loc6_;
            _loc7_ = Math.max(this._start_index,_loc7_ - this._additionalRequest);
            this.request_players(_loc7_,Math.abs(_loc5_),true);
         }
      }
      
      protected function fillLists(param1:Array, param2:int = 0) : void
      {
         trace("fillLists",param1.length);
         var _loc3_:int = 0;
         while(_loc3_ < param1.length)
         {
            param1[_loc3_] = this._playersCache[param2 + _loc3_];
            _loc3_++;
         }
      }
      
      protected function updateLocalPlayer(param1:Array) : void
      {
         var _loc2_:uint;
         if(this._player_data && param1)
         {
            _loc2_ = 0;
            while(_loc2_ < param1.length)
            {
               if(param1[_loc2_] && param1[_loc2_].nickname == this._player_data.nickname)
               {
                  param1[_loc2_] = this._player_data;
                  break;
               }
               _loc2_++;
            }
         }
      }
   }
}

