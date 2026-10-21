package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.TextField;
   import scaleform.clik.controls.ScrollingList;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ListEvent;
   import survarium.constants.TeamSide;
   
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
      
      public function set autoSizeLists(value:Boolean) : void
      {
         _autoSizeLists = value;
         this.sizeLists();
      }
      
      public function get autoSizeLists() : Boolean
      {
         return this._autoSizeLists;
      }
      
      public function setRowSizes(smallRow:int, bigRow:int) : void
      {
         _smallTeamRow = smallRow;
         _bigTeamRow = bigRow;
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
      
      public function set_local_name(playerName:String) : void
      {
         trace("HUD_PL::set_local_name",playerName);
         _playerName = playerName;
         var squadId:int = -1;
         var i:uint;
         if(this._players)
         {
            i = 0;
            while(i < this._players.length)
            {
               if(this._players[i].name == this._playerName)
               {
                  this._players[i].isLocal = true;
                  squadId = (this._players[i].squad_id);
               }
               i++;
            }
            i = 0;
            while(i < this._players.length)
            {
               if(this._players[i].squad_id == squadId)
               {
                  this._players[i].isLocalSquad = true;
               }
               i++;
            }
            this.splitTeams();
            this.populateTeams();
         }
      }
      
      public function addPlayersList(players:Array) : void
      {
         trace("HUD_PL::addPlayersList!");
         _players = players;
         var squadId:int = -1;
         var i:uint = 0;
         while(i < this._players.length)
         {
            trace("_players[t].score",this._players[i].score);
            if(this._players[i].score == undefined)
            {
               this._players[i].score = 0;
            }
            trace("_players[t].score",this._players[i].score,this._players[i].rank);
            if(this._players[i].name == this._playerName)
            {
               this._players[i].isLocal = true;
               squadId = (this._players[i].squad_id);
            }
            i++;
         }
         i = 0;
         while(i < this._players.length)
         {
            if(this._players[i].squad_id == squadId)
            {
               this._players[i].isLocalSquad = true;
            }
            i++;
         }
         this.splitTeams();
         this.populateTeams();
      }
      
      public function set_online_player(playerName:String, isOnline:Boolean) : void
      {
         var teamDP:DataProvider;
         var team:uint;
         var i:uint;
         while(i < this._players.length)
         {
            if(this._players[i].name == playerName)
            {
               if(this._players[i].online == isOnline)
               {
                  return;
               }
               team = (this._players[i].team);
               break;
            }
            i++;
         }
         if(team == TeamSide.ALLY)
         {
            teamDP = this._team1DP;
         }
         else
         {
            teamDP = this._team2DP;
         }
         var j:uint = 0;
         while(j < teamDP.length)
         {
            if(teamDP[j].name == playerName)
            {
               teamDP[j].online = isOnline;
               teamDP.invalidate();
               return;
            }
            j++;
         }
      }
      
      public function updatePlayer(player:Object) : void
      {
         var teamDP:DataProvider;
         var team:uint;
         var i:uint = 0;
         while(i < this._players.length)
         {
            if(this._players[i].id == player.id)
            {
               oldPlayer = this._players[i];
               if(player.kills == oldPlayer.kills && player.deaths == oldPlayer.deaths && player.ping == oldPlayer.ping && player.artifacts == oldPlayer.artifacts && player.rank == oldPlayer.rank)
               {
                  return;
               }
               team = (this._players[i].team);
               break;
            }
            i++;
         }
         if(team == TeamSide.ALLY)
         {
            teamDP = this._team1DP;
         }
         else
         {
            teamDP = this._team2DP;
         }
         var j:uint = 0;
         var oldPlayer:Object;
         var k:uint;
         while(j < teamDP.length)
         {
            if(teamDP[j].id == player.id)
            {
               if(player.score == undefined)
               {
                  player.score = 0;
               }
               teamDP[j].kills = player.kills;
               teamDP[j].deaths = player.deaths;
               teamDP[j].ping = player.ping;
               teamDP[j].artifacts = player.artifacts;
               teamDP[j].rank = player.rank;
               teamDP[j].score = player.score;
               trace("sortAfterUpdate::");
               this.sortTeam(teamDP);
               k = 0;
               while(k < teamDP.length)
               {
                  trace("teamDPS",k,teamDP[j].name,teamDP[k].score,teamDP[k].rank,teamDP[k].kills,teamDP[k].deaths);
                  k++;
               }
               teamDP.invalidate();
               return;
            }
            j++;
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
      
      private function sortTeam(teamDP:DataProvider) : void
      {
         var i:uint;
         while(i < teamDP.length)
         {
            trace("team",i,teamDP[i].name,teamDP[i].score,teamDP[i].rank,teamDP[i].kills,teamDP[i].deaths);
            i++;
         }
         trace("HUD_PL::sort",teamDP);
         teamDP.sortOn(["score","rank","kills","deaths"],Array.NUMERIC | Array.DESCENDING);
         i = 0;
         while(i < teamDP.length)
         {
            trace("team",i,teamDP[i].name,teamDP[i].score,teamDP[i].rank,teamDP[i].kills,teamDP[i].deaths);
            i++;
         }
      }
      
      private function splitTeams() : void
      {
         _team1 = new Array();
         _team2 = new Array();
         var i:uint = 0;
         while(i < this._players.length)
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
            if(this._players[i].team == TeamSide.ALLY)
            {
               this._team1.push(this._players[i]);
            }
            else
            {
               this._team2.push(this._players[i]);
            }
            i++;
         }
      }
      
      private function populateTeams() : void
      {
         _team1DP = new DataProvider(this._team1);
         this.sortTeam(this._team1DP);
         i = 0;
         while(i < this._team1DP.length)
         {
            trace("teamDP",i,this._team1DP[i].name,this._team1DP[i].score,this._team1DP[i].rank,this._team1DP[i].kills,this._team1DP[i].deaths);
            i++;
         }
         _team2DP = new DataProvider(this._team2);
         this.sortTeam(this._team2DP);
         if(this._smallTeamRow != -1)
         {
            maxTeamSize = Math.max(this._team1DP.length,this._team2DP.length);
            rowSize = maxTeamSize > 8 ? this._bigTeamRow : this._smallTeamRow;
            this.team1list.rowHeight = rowSize;
            this.team2list.rowHeight = rowSize;
            trace("populateTeamsSize::",rowSize);
         }
         this.team1list.dataProvider = this._team1DP;
         this.team2list.dataProvider = this._team2DP;
         this.sizeLists();
         var count:uint = 0;
         var maxTeamSize:int;
         var rowSize:int;
         var i:uint;
         if(this._team1)
         {
            count = this._team1DP.length;
            i = 0;
            while(i < count)
            {
               if(this._team1DP[i].name == this._playerName)
               {
                  this.team1list.selectedIndex = i;
                  this.team1list.validateNow();
                  return;
               }
               i++;
            }
         }
         if(this._team2DP)
         {
            count = this._team2DP.length;
            i = 0;
            while(i < count)
            {
               if(this._team2DP[i].name == this._playerName)
               {
                  this.team2list.selectedIndex = i;
                  this.team2list.validateNow();
                  return;
               }
               i++;
            }
         }
      }
   }
}

