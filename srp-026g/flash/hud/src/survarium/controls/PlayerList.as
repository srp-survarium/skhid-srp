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
      
      public function set autoSizeLists(value:Boolean) : void
      {
         _autoSizeLists = value;
         if(this._autoSizeLists)
         {
            this.team1list.height = this.team1list.rowHeight * (this.team1list.dataProvider ? this.team1list.dataProvider.length : 0);
            this.team2list.height = this.team2list.rowHeight * (this.team2list.dataProvider ? this.team2list.dataProvider.length : 0);
         }
      }
      
      public function get autoSizeLists() : Boolean
      {
         return this._autoSizeLists;
      }
      
      protected function onListIndexChange(e:Event) : void
      {
         trace("HUD_PL::onListIndexChange::",e.target.name,(e.target as ScrollingList).selectedIndex);
         if((e.target as ScrollingList).selectedIndex == -1)
         {
            return;
         }
         if(e.target == this.team1list)
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
      
      public function set_local_name(value:String) : void
      {
         trace("HUD_PL::set_local_name",value);
         _playerName = value;
         var playerSquad:int = -1;
         var t:uint;
         if(this._players)
         {
            for(t = 0; t < this._players.length; t++)
            {
               if(this._players[t].name == this._playerName)
               {
                  this._players[t].isLocal = true;
                  playerSquad = (this._players[t].squad_id);
               }
            }
            for(t = 0; t < this._players.length; t++)
            {
               if(this._players[t].squad_id == playerSquad)
               {
                  this._players[t].isLocalSquad = true;
               }
            }
            this.splitTeams();
            this.populateTeams();
         }
      }
      
      public function addPlayersList(players:Array) : void
      {
         trace("HUD_PL::addPlayersList!");
         _players = players;
         var playerSquad:int = -1;
         for(var t:uint = 0; t < this._players.length; t++)
         {
            trace("_players[t].score",this._players[t].score);
            if(this._players[t].score == undefined)
            {
               this._players[t].score = 0;
            }
            trace("_players[t].score",this._players[t].score,this._players[t].rank);
            if(this._players[t].name == this._playerName)
            {
               this._players[t].isLocal = true;
               playerSquad = (this._players[t].squad_id);
            }
         }
         for(t = 0; t < this._players.length; t++)
         {
            if(this._players[t].squad_id == playerSquad)
            {
               this._players[t].isLocalSquad = true;
            }
         }
         this.splitTeams();
         this.populateTeams();
      }
      
      public function set_online_player(name:String, bool:Boolean) : void
      {
         var teamDP:DataProvider;
         var team:uint;
         for(var t:uint = 0; t < this._players.length; t++)
         {
            if(this._players[t].name == name)
            {
               if(this._players[t].online == bool)
               {
                  return;
               }
               team = (this._players[t].team);
               break;
            }
         }
         if(team == 1)
         {
            teamDP = this._team1DP;
         }
         else
         {
            teamDP = this._team2DP;
         }
         for(var i:uint = 0; i < teamDP.length; i++)
         {
            if(teamDP[i].name == name)
            {
               teamDP[i].online = bool;
               teamDP.invalidate();
               return;
            }
         }
      }
      
      public function updatePlayer(player:Object) : void
      {
         var teamDP:DataProvider;
         var team:uint;
         for(var t:uint = 0; t < this._players.length; t++)
         {
            if(this._players[t].id == player.id)
            {
               origin = this._players[t];
               if(player.kills == origin.kills && player.deaths == origin.deaths && player.ping == origin.ping && player.artifacts == origin.artifacts && player.rank == origin.rank)
               {
                  return;
               }
               team = (this._players[t].team);
               break;
            }
         }
         if(team == 1)
         {
            teamDP = this._team1DP;
         }
         else
         {
            teamDP = this._team2DP;
         }
         for(var i:uint = 0; i < teamDP.length; i++)
         {
            var origin:Object;
            var j:uint;
            if(teamDP[i].id == player.id)
            {
               if(player.score == undefined)
               {
                  player.score = 0;
               }
               teamDP[i].kills = player.kills;
               teamDP[i].deaths = player.deaths;
               teamDP[i].ping = player.ping;
               teamDP[i].artifacts = player.artifacts;
               teamDP[i].rank = player.rank;
               teamDP[i].score = player.score;
               trace("sortAfterUpdate::");
               this.sortTeam(teamDP);
               for(j = 0; j < teamDP.length; j++)
               {
                  trace("teamDPS",j,teamDP[i].name,teamDP[j].score,teamDP[j].rank,teamDP[j].kills,teamDP[j].deaths);
               }
               teamDP.invalidate();
               return;
            }
         }
      }
      
      private function sortTeam(team:DataProvider) : void
      {
         for(var i:uint = 0; i < team.length; i++)
         {
            trace("team",i,team[i].name,team[i].score,team[i].rank,team[i].kills,team[i].deaths);
         }
         trace("HUD_PL::sort",team);
         team.sortOn(["score","rank","kills","deaths"],Array.NUMERIC | Array.DESCENDING);
         for(i = 0; i < team.length; i++)
         {
            trace("team",i,team[i].name,team[i].score,team[i].rank,team[i].kills,team[i].deaths);
         }
      }
      
      private function splitTeams() : void
      {
         _team1 = new Array();
         _team2 = new Array();
         for(var i:uint = 0; i < this._players.length; i++)
         {
            if(this._players[i].kills == undefined)
            {
               this._players[i].kills = 0;
            }
            if(this._players[i].deaths == undefined)
            {
               this._players[i].deaths = 0;
            }
            if(this._players[i].score == undefined)
            {
               this._players[i].score = 0;
            }
            if(this._players[i].team == 1)
            {
               this._team1.push(this._players[i]);
            }
            else
            {
               this._team2.push(this._players[i]);
            }
         }
      }
      
      private function populateTeams() : void
      {
         _team1DP = new DataProvider(this._team1);
         this.sortTeam(this._team1DP);
         for(i = 0; i < this._team1DP.length; i++)
         {
            trace("teamDP",i,this._team1DP[i].name,this._team1DP[i].score,this._team1DP[i].rank,this._team1DP[i].kills,this._team1DP[i].deaths);
         }
         this.team1list.dataProvider = this._team1DP;
         _team2DP = new DataProvider(this._team2);
         this.sortTeam(this._team2DP);
         this.team2list.dataProvider = this._team2DP;
         if(this._autoSizeLists)
         {
            this.team1list.height = this.team1list.rowHeight * this._team1DP.length;
            this.team2list.height = this.team2list.rowHeight * this._team2DP.length;
         }
         var l:uint = 0;
         var i:uint;
         if(this._team1)
         {
            l = this._team1DP.length;
            for(i = 0; i < l; i++)
            {
               if(this._team1DP[i].name == this._playerName)
               {
                  this.team1list.selectedIndex = i;
                  this.team1list.validateNow();
                  return;
               }
            }
         }
         if(this._team2DP)
         {
            l = this._team2DP.length;
            for(i = 0; i < l; i++)
            {
               if(this._team2DP[i].name == this._playerName)
               {
                  this.team2list.selectedIndex = i;
                  this.team2list.validateNow();
                  return;
               }
            }
         }
      }
   }
}

