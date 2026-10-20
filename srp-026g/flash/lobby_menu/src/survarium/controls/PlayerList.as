package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.TextField;
   import scaleform.clik.controls.ScrollingList;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ListEvent;
   
   public class PlayerList extends MovieClip
   {
      
      private var _players:Array;
      
      private var _team1:Array;
      
      private var _team2:Array;
      
      private var _team1DP:DataProvider;
      
      private var _team2DP:DataProvider;
      
      private var _playerName:String = "";
      
      private var _autoSizeLists:Boolean = false;
      
      private var _smallTeamRow:int = -1;
      
      private var _bigTeamRow:int = -1;
      
      public var team1:TextField;
      
      public var team2:TextField;
      
      public var team1list:ScrollingList;
      
      public var team2list:ScrollingList;
      
      public function PlayerList()
      {
         super();
         this.team1list.addEventListener(ListEvent.INDEX_CHANGE,this.onListIndexChange);
         this.team2list.addEventListener(ListEvent.INDEX_CHANGE,this.onListIndexChange);
         if(this.team1 && this.team2)
         {
            this.team1.mouseEnabled = this.team2.mouseEnabled = false;
         }
      }
      
      public function set autoSizeLists(param1:Boolean) : void
      {
         _autoSizeLists = param1;
         this.sizeLists();
      }
      
      public function get autoSizeLists() : Boolean
      {
         return this._autoSizeLists;
      }
      
      public function setRowSizes(param1:int, param2:int) : void
      {
         _smallTeamRow = param1;
         _bigTeamRow = param2;
      }
      
      protected function onListIndexChange(param1:Event) : void
      {
         trace("HUD_PL::onListIndexChange::",param1.target.name,(param1.target as ScrollingList).selectedIndex);
         if((param1.target as ScrollingList).selectedIndex == -1)
         {
            return;
         }
         if(param1.target == this.team1list)
         {
            this.team2list.selectedIndex = -1;
         }
         else
         {
            this.team1list.selectedIndex = -1;
         }
         this.team2list.validateNow();
         this.team2list.validateNow();
      }
      
      public function set_local_name(param1:String) : void
      {
         trace("HUD_PL::set_local_name",param1);
         _playerName = param1;
         var _loc2_:int = -1;
         var _loc3_:uint;
         if(this._players)
         {
            _loc3_ = 0;
            while(_loc3_ < this._players.length)
            {
               if(this._players[_loc3_].name == this._playerName)
               {
                  this._players[_loc3_].isLocal = true;
                  _loc2_ = (this._players[_loc3_].squad_id);
               }
               _loc3_++;
            }
            _loc3_ = 0;
            while(_loc3_ < this._players.length)
            {
               if(this._players[_loc3_].squad_id == _loc2_)
               {
                  this._players[_loc3_].isLocalSquad = true;
               }
               _loc3_++;
            }
            this.splitTeams();
            this.populateTeams();
         }
      }
      
      public function addPlayersList(param1:Array) : void
      {
         trace("HUD_PL::addPlayersList!");
         _players = param1;
         var _loc2_:int = -1;
         var _loc3_:uint = 0;
         while(_loc3_ < this._players.length)
         {
            trace("_players[t].score",this._players[_loc3_].score);
            if(this._players[_loc3_].score == undefined)
            {
               this._players[_loc3_].score = 0;
            }
            trace("_players[t].score",this._players[_loc3_].score,this._players[_loc3_].rank);
            if(this._players[_loc3_].name == this._playerName)
            {
               this._players[_loc3_].isLocal = true;
               _loc2_ = (this._players[_loc3_].squad_id);
            }
            _loc3_++;
         }
         _loc3_ = 0;
         while(_loc3_ < this._players.length)
         {
            if(this._players[_loc3_].squad_id == _loc2_)
            {
               this._players[_loc3_].isLocalSquad = true;
            }
            _loc3_++;
         }
         this.splitTeams();
         this.populateTeams();
      }
      
      public function set_online_player(param1:String, param2:Boolean) : void
      {
         var _loc3_:DataProvider;
         var _loc4_:uint;
         var _loc5_:uint;
         while(_loc5_ < this._players.length)
         {
            if(this._players[_loc5_].name == param1)
            {
               if(this._players[_loc5_].online == param2)
               {
                  return;
               }
               _loc4_ = (this._players[_loc5_].team);
               break;
            }
            _loc5_++;
         }
         if(_loc4_ == 1)
         {
            _loc3_ = this._team1DP;
         }
         else
         {
            _loc3_ = this._team2DP;
         }
         var _loc6_:uint = 0;
         while(_loc6_ < _loc3_.length)
         {
            if(_loc3_[_loc6_].name == param1)
            {
               _loc3_[_loc6_].online = param2;
               _loc3_.invalidate();
               return;
            }
            _loc6_++;
         }
      }
      
      public function updatePlayer(param1:Object) : void
      {
         var _loc2_:DataProvider;
         var _loc3_:uint;
         var _loc4_:uint = 0;
         while(_loc4_ < this._players.length)
         {
            if(this._players[_loc4_].id == param1.id)
            {
               _loc6_ = this._players[_loc4_];
               if(param1.kills == _loc6_.kills && param1.deaths == _loc6_.deaths && param1.ping == _loc6_.ping && param1.artifacts == _loc6_.artifacts && param1.rank == _loc6_.rank)
               {
                  return;
               }
               _loc3_ = (this._players[_loc4_].team);
               break;
            }
            _loc4_++;
         }
         if(_loc3_ == 1)
         {
            _loc2_ = this._team1DP;
         }
         else
         {
            _loc2_ = this._team2DP;
         }
         var _loc5_:uint = 0;
         var _loc6_:Object;
         var _loc7_:uint;
         while(_loc5_ < _loc2_.length)
         {
            if(_loc2_[_loc5_].id == param1.id)
            {
               if(param1.score == undefined)
               {
                  param1.score = 0;
               }
               _loc2_[_loc5_].kills = param1.kills;
               _loc2_[_loc5_].deaths = param1.deaths;
               _loc2_[_loc5_].ping = param1.ping;
               _loc2_[_loc5_].artifacts = param1.artifacts;
               _loc2_[_loc5_].rank = param1.rank;
               _loc2_[_loc5_].score = param1.score;
               trace("sortAfterUpdate::");
               this.sortTeam(_loc2_);
               _loc7_ = 0;
               while(_loc7_ < _loc2_.length)
               {
                  trace("teamDPS",_loc7_,_loc2_[_loc5_].name,_loc2_[_loc7_].score,_loc2_[_loc7_].rank,_loc2_[_loc7_].kills,_loc2_[_loc7_].deaths);
                  _loc7_++;
               }
               _loc2_.invalidate();
               return;
            }
            _loc5_++;
         }
      }
      
      private function sizeLists() : void
      {
         if(this._autoSizeLists)
         {
            this.team1list.height = this.team1list.rowHeight * (this.team1list.dataProvider ? this.team1list.dataProvider.length : 0);
            this.team2list.height = this.team2list.rowHeight * (this.team2list.dataProvider ? this.team2list.dataProvider.length : 0);
         }
      }
      
      private function sortTeam(param1:DataProvider) : void
      {
         var _loc2_:uint;
         while(_loc2_ < param1.length)
         {
            trace("team",_loc2_,param1[_loc2_].name,param1[_loc2_].score,param1[_loc2_].rank,param1[_loc2_].kills,param1[_loc2_].deaths);
            _loc2_++;
         }
         trace("HUD_PL::sort",param1);
         param1.sortOn(["score","rank","kills","deaths"],Array.NUMERIC | Array.DESCENDING);
         _loc2_ = 0;
         while(_loc2_ < param1.length)
         {
            trace("team",_loc2_,param1[_loc2_].name,param1[_loc2_].score,param1[_loc2_].rank,param1[_loc2_].kills,param1[_loc2_].deaths);
            _loc2_++;
         }
      }
      
      private function splitTeams() : void
      {
         _team1 = new Array();
         _team2 = new Array();
         var _loc1_:uint = 0;
         while(_loc1_ < this._players.length)
         {
            if(this._players[_loc1_].kills == undefined)
            {
               this._players[_loc1_].kills = 0;
            }
            if(this._players[_loc1_].deaths == undefined)
            {
               this._players[_loc1_].deaths = 0;
            }
            if(this._players[_loc1_].score == undefined)
            {
               this._players[_loc1_].score = 0;
            }
            if(this._players[_loc1_].team == 1)
            {
               this._team1.push(this._players[_loc1_]);
            }
            else
            {
               this._team2.push(this._players[_loc1_]);
            }
            _loc1_++;
         }
      }
      
      private function populateTeams() : void
      {
         _team1DP = new DataProvider(this._team1);
         this.sortTeam(this._team1DP);
         _loc4_ = 0;
         while(_loc4_ < this._team1DP.length)
         {
            trace("teamDP",_loc4_,this._team1DP[_loc4_].name,this._team1DP[_loc4_].score,this._team1DP[_loc4_].rank,this._team1DP[_loc4_].kills,this._team1DP[_loc4_].deaths);
            _loc4_++;
         }
         _team2DP = new DataProvider(this._team2);
         this.sortTeam(this._team2DP);
         if(this._smallTeamRow != -1)
         {
            _loc2_ = Math.max(this._team1DP.length,this._team2DP.length);
            _loc3_ = _loc2_ > 8 ? this._bigTeamRow : this._smallTeamRow;
            this.team1list.rowHeight = _loc3_;
            this.team2list.rowHeight = _loc3_;
            trace("populateTeamsSize::",_loc3_);
         }
         this.team1list.dataProvider = this._team1DP;
         this.team2list.dataProvider = this._team2DP;
         this.sizeLists();
         var _loc1_:uint = 0;
         var _loc2_:int;
         var _loc3_:int;
         var _loc4_:uint;
         if(this._team1)
         {
            _loc1_ = this._team1DP.length;
            _loc4_ = 0;
            while(_loc4_ < _loc1_)
            {
               if(this._team1DP[_loc4_].name == this._playerName)
               {
                  this.team1list.selectedIndex = _loc4_;
                  this.team1list.validateNow();
                  return;
               }
               _loc4_++;
            }
         }
         if(this._team2DP)
         {
            _loc1_ = this._team2DP.length;
            _loc4_ = 0;
            while(_loc4_ < _loc1_)
            {
               if(this._team2DP[_loc4_].name == this._playerName)
               {
                  this.team2list.selectedIndex = _loc4_;
                  this.team2list.validateNow();
                  return;
               }
               _loc4_++;
            }
         }
      }
   }
}

