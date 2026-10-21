package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.utils.Dictionary;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.ScrollingList;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.gfx.MouseEventEx;
   import survarium.data.LocalizationData;
   import survarium.gsearch.MatchMakingSettings;
   import survarium.rooms.RoomsWindow;
   
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
      
      // The right-click on a player is listened to (onActivate, once).
      private var _playerInfoClick:Boolean;
      
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
      
      public function set totalPlayers(value:int) : void
      {
         _totalPlayers = value;
         this._topLeague.length = Math.min(100,this._totalPlayers);
         var scrollIndex:int = this._start_index + this.list.scrollPosition;
         this.show_players_list(this._request_index,this._total_renderers,scrollIndex,false);
         invalidate(InvalidationType.SETTINGS);
      }
      
      public function set player_data(value:Object) : void
      {
         if(value)
         {
            value.is_local = true;
         }
         _player_data = value;
         this.playerPlace.data = this._player_data;
      }
      
      public function fillPlayers(players:Array) : void
      {
         // The rooms' answers come as ranking rows of their own range.
         if(RoomsWindow.take(players))
         {
            return;
         }
         if(NotificationListScrollContent.replayTake(players))
         {
            return;
         }
         if(MatchMakingSettings.pvbTake(players))
         {
            return;
         }
         if(BottomPanel.matchesTake(players))
         {
            return;
         }
         trace("fillPlayers:: filled",players.length,"requested::",this._requestList.length);
         var i:uint = 0;
         var league:Array;
         var playerIndex:int;
         while(i < players.length)
         {
            playerIndex = this._requestList.shift();
            this._playersCache[playerIndex] = players[i];
            i++;
         }
         if(this.tabBar.selectedIndex == 0)
         {
            league = this._topLeague;
         }
         else
         {
            league = this._myLeague;
         }
         i = 0;
         while(i < league.length)
         {
            if(!league[i])
            {
               league[i] = this._playersCache[this._start_index + i];
            }
            i++;
         }
         this.updateLocalPlayer(league);
         this.list.dataProvider = new DataProvider(league);
      }
      
      public function onActivate() : void
      {
         _playersCache = new Dictionary();
         var i:uint = 0;
         while(i < this._topLeague.length)
         {
            this._topLeague[i] = null;
            i++;
         }
         i = 0;
         while(i < this._myLeague.length)
         {
            this._myLeague[i] = null;
            i++;
         }
         this.request_players(0,100);
         if(this.tabBar.selectedIndex != 0)
         {
            this.onIndexChange(null,true);
         }
         // A right-click on a row of either tab, any page, or on the
         // player's own place under them: "Show player's information".
         if(!this._playerInfoClick)
         {
            this._playerInfoClick = true;
            addEventListener(MouseEvent.CLICK,this.onPlayerRightClick);
         }
      }

      protected function onLocal(value:String) : void
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
      
      protected function onListScroll(event:Event) : void
      {
         var scrolled:int = Math.max(-20,Math.min(this._lastScroll - this.list.scrollPosition,20));
         _request_index = this.list.scrollPosition;
         var offset:int = scrolled < 0 ? (this._total_renderers + scrolled + this._additionalRequest) : -this._additionalRequest;
         scrolled = Math.abs(scrolled);
         trace("onListScroll",offset,"scrolled::",scrolled,Math.abs(scrolled),this._minimumScrollUpdate);
         if(scrolled > this._minimumScrollUpdate)
         {
            _lastScroll = this.list.scrollPosition;
            this.request_players(Math.max(this._start_index + this._request_index + offset,this._start_index),scrolled);
         }
      }
      
      protected function request_players(start:int, count:int, padded:Boolean = false) : void
      {
         trace("request_players_init:: start:",start,"count:",count);
         count += padded ? this._additionalRequest * 2 : 0;
         trace("request_players_count::",start,count,"total_players:",this._totalPlayers,"league_start:",this._start_index);
         if(start + count > this._totalPlayers)
         {
            count = Math.max(0,this._totalPlayers - start);
         }
         if(start + count - this._start_index > 100)
         {
            count = Math.max(0,100 - (start - this._start_index));
         }
         if(count == 0)
         {
            return;
         }
         trace("request_players_st::",start,count);
         var firstMissing:int = -1;
         var endMissing:int = -1;
         var i:int = 0;
         while(i < count)
         {
            if(!this._playersCache[start + i])
            {
               if(firstMissing == -1)
               {
                  firstMissing = i;
               }
               endMissing = i + 1;
            }
            i++;
         }
         trace("request_players_end::",firstMissing,endMissing);
         start += firstMissing;
         count = endMissing - firstMissing;
         if(count <= 0)
         {
            return;
         }
         i = 0;
         while(i < count)
         {
            this._requestList.push(start + i);
            i++;
         }
         trace("request_players_values: ",start,count,"st:",this._start_index,this.list.scrollPosition,"requested::",this._requestList.length);
         ExternalInterface.call("request_players_range",start,count);
      }
      
      protected function onIndexChange(event:Event, reload:Boolean = false) : void
      {
         trace("PR::change_league");
         var leagueSize:uint = 100;
         this.list.scrollBar.removeEventListener(Event.SCROLL,this.onListScroll);
         var scrollIndex:int = this._start_index + this.list.scrollPosition;
         var total:int;
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
            total = (this._totalPlayers);
            if(this._player_data.place + 50 > total)
            {
               _start_index = Math.max(0,total - leagueSize);
            }
            else
            {
               _start_index = Math.max(0,this._player_data.place - 50);
            }
            trace("init_my_league:: from:",this._start_index,"to:",this._start_index + leagueSize);
            this.list.scrollPosition = Math.max(0,this._player_data.place - this._start_index - this._total_renderers / 2);
            _lastScroll = this.list.scrollPosition;
            if(this._player_data.place + this._total_renderers / 2 > total)
            {
               _request_index = Math.max(0,total - this._total_renderers);
            }
            else
            {
               _request_index = Math.max(0,this._player_data.place - this._total_renderers / 2);
            }
            this._myLeague.length = Math.min(leagueSize,this._totalPlayers - this._start_index);
            this.fillLists(this._myLeague,this._start_index);
            this.updateLocalPlayer(this._myLeague);
            this.list.dataProvider = new DataProvider(this._myLeague);
            this.show_players_list(this._request_index,this._total_renderers,scrollIndex,reload);
            this.list.scrollBar.addEventListener(Event.SCROLL,this.onListScroll);
         }
      }
      
      protected function show_players_list(requestIndex:int, count:int, lastIndex:int, reload:Boolean) : void
      {
         var shift:int;
         if(reload)
         {
            shift = this._total_renderers;
         }
         else
         {
            shift = Math.max(-this._total_renderers,Math.min(lastIndex - requestIndex,this._total_renderers));
         }
         var offset:int = shift < 0 ? (count + shift) : 0;
         var start:int;
         trace("show_players_list",this._start_index,"begin::",requestIndex,lastIndex,shift,count,"||",lastIndex,requestIndex);
         if(shift != 0)
         {
            start = requestIndex + offset;
            start = Math.max(this._start_index,start - this._additionalRequest);
            this.request_players(start,Math.abs(shift),true);
         }
      }
      
      protected function fillLists(league:Array, startIndex:int = 0) : void
      {
         trace("fillLists",league.length);
         var i:int = 0;
         while(i < league.length)
         {
            league[i] = this._playersCache[startIndex + i];
            i++;
         }
      }
      
      protected function updateLocalPlayer(league:Array) : void
      {
         var i:uint;
         if(this._player_data && league)
         {
            i = 0;
            while(i < league.length)
            {
               if(league[i] && league[i].nickname == this._player_data.nickname)
               {
                  league[i] = this._player_data;
                  break;
               }
               i++;
            }
         }
      }
      
      private function onPlayerRightClick(e:MouseEvent) : void
      {
         var ex:MouseEventEx = e as MouseEventEx;
         if(ex == null || ex.buttonIdx != 1)
         {
            return;
         }
         for(var at:DisplayObject = e.target as DisplayObject; at != null; at = at.parent)
         {
            var row:Object = this.rankingRow(at);
            if(row != null)
            {
               LobbyMenu.showPlayerInfoMenu(String(row.nickname),uint(row.place),LobbyMenu.PLAYER_INFO_BY_PLACE,this);
               return;
            }
         }
      }
      
      // A row's data (RankingItemRenderer's): its place and nickname.
      private function rankingRow(at:DisplayObject) : Object
      {
         try
         {
            var data:Object = Object(at).data;
            if(data != null && data.nickname != null && int(data.place) > 0)
            {
               return data;
            }
         }
         catch(e:Error)
         {
         }
         return null;
      }
   }
}

