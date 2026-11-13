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

      // One kill ranking: fill the left column's ten rows, then overflow right.
      private static const COLUMN_ROWS:int = 10;
      private var _ranked:Boolean = false;

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

      public function get ranked() : Boolean
      {
         return this._ranked;
      }

      // A change lays the players out again; ranked, the sides' names are not shown.
      public function set ranked(value:Boolean) : void
      {
         _ranked = value;
         if(this.team1 && this.team2)
         {
            this.team1.visible = this.team2.visible = !value;
         }
         if(this._players)
         {
            this.splitTeams();
            this.populateTeams();
         }
      }

      // The ranking's order, the server's (docs/notes/ffa.md; the halves it sends as teams 0 and 1):
      // kills, then the score, highest first; then deaths, fewest first; then the place in the
      // match, which is the order the server's rows come in (`order`, addPlayersList).
      public static function byStanding(a:Object, b:Object) : int
      {
         var order:int = compare(Number(b.kills),Number(a.kills));
         if(order == 0)
         {
            order = compare(Number(b.score),Number(a.score));
         }
         if(order == 0)
         {
            order = compare(Number(a.deaths),Number(b.deaths));
         }
         return order != 0 ? order : compare(Number(a.order),Number(b.order));
      }

      private static function compare(a:*, b:*) : int
      {
         return a < b ? -1 : (a > b ? 1 : 0);
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
            // the row's place among the rows as given (byStanding's last word)
            this._players[i].order = i;
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
      
      private function columnOf(field:String, value:*) : DataProvider
      {
         for each(var column:DataProvider in [this._team1DP,this._team2DP])
         {
            if(column)
            {
               for(var i:uint = 0; i < column.length; i++)
               {
                  if(column[i][field] == value)
                  {
                     return column;
                  }
               }
            }
         }
         return null;
      }

      public function set_online_player(name:String, bool:Boolean) : void
      {
         for(var t:uint = 0; t < this._players.length; t++)
         {
            if(this._players[t].name == name)
            {
               if(this._players[t].online == bool)
               {
                  return;
               }
               break;
            }
         }
         var teamDP:DataProvider = this.columnOf("name",name);
         if(teamDP == null)
         {
            return;
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
         for(var t:uint = 0; t < this._players.length; t++)
         {
            if(this._players[t].id == player.id)
            {
               origin = this._players[t];
               if(player.kills == origin.kills && player.deaths == origin.deaths && player.ping == origin.ping && player.artifacts == origin.artifacts && player.rank == origin.rank && (player.score == undefined ? 0 : player.score) == origin.score)
               {
                  return;
               }
               break;
            }
         }
         var teamDP:DataProvider = this.columnOf("id",player.id);
         if(teamDP == null)
         {
            return;
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
               if(this._ranked)
               {
                  // new kills may move the player into the other column
                  this.splitTeams();
                  this.populateTeams();
                  return;
               }
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
         if(this._ranked)
         {
            teamDP.sort(PlayerList.byStanding);
         }
         else
         {
            teamDP.sortOn(["score","rank","kills","deaths"],Array.NUMERIC | Array.DESCENDING);
         }
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
            if(!this._ranked)
            {
               (this._players[i].team == TeamSide.ALLY ? this._team1 : this._team2).push(this._players[i]);
            }
            i++;
         }
         if(this._ranked)
         {
            var ranking:Array = this._players.concat();
            ranking.sort(PlayerList.byStanding);
            _team1 = ranking.slice(0,COLUMN_ROWS);
            _team2 = ranking.slice(COLUMN_ROWS);
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
         this.team1list.selectedIndex = -1;
         this.team2list.selectedIndex = -1;
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

