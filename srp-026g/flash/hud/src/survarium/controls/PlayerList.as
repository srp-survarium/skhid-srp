package survarium.controls
{
   import flash.display.DisplayObject;
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
      
      // The Settings' "Hide player scores" (exe patch 83): each team's players by clan, then name (byClanAndName),
      // their score, kills, deaths and rank not shown (PlayerListRenderer asks namesOnly).
      private var _namesOnly:Boolean = false;

      // Free for all (HUD.set_game_type with GAME_MODE_FREE_FOR_ALL; the exe sends the local player as
      // the ally, every other as an enemy): every player on one ranking (byStanding, or names only by
      // byClanAndName), filling the left column before continuing on the right.
      // Other modes keep each team in its own column.
      private var _ranked:Boolean = false;

      // PlayerListz's authored lists each fit ten 30 px rows (twenty match places).
      private static const COLUMN_ROWS:int = 10;

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
      
      public function get namesOnly() : Boolean
      {
         return this._namesOnly;
      }
      
      // A change lays the teams out again, in the other order, their rows drawn again.
      public function set namesOnly(value:Boolean) : void
      {
         if(value == this._namesOnly)
         {
            return;
         }
         _namesOnly = value;
         this.showHeaders(!value);
         if(this._players)
         {
            this.splitTeams();
            this.populateTeams();
         }
      }

      public function get ranked() : Boolean
      {
         return this._ranked;
      }

      // A change lays the players out again; ranked, the teams' names are not shown.
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

      // The ranking's order, the results screen's (the server's, docs/notes/ffa.md): kills, then the
      // score (the row's `rank` carries the score the game counts; it sends no `score`), highest
      // first; then deaths, fewest first; then the player's place in the match (the row's id).
      public static function byStanding(a:Object, b:Object) : int
      {
         var order:int = compare(Number(b.kills),Number(a.kills));
         if(order == 0)
         {
            order = compare(Number(b.score),Number(a.score));
         }
         if(order == 0)
         {
            order = compare(Number(b.rank),Number(a.rank));
         }
         if(order == 0)
         {
            order = compare(Number(a.deaths),Number(b.deaths));
         }
         return order != 0 ? order : compare(Number(a.id),Number(b.id));
      }

      // The column that shows a player's row (found by `field`), null for none.
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
      
      // A row's name is "[TAG] name" for a player in a clan (initialize_match makes it so), else the name.
      // Its clan tag, "" for none.
      public static function clanOf(name:String) : String
      {
         var end:int = name.indexOf("] ");
         return name.charAt(0) == "[" && end > 1 ? name.substring(1,end) : "";
      }
      
      // The names-only order: the clans' players by tag, then the players without a clan, then the
      // bots (the server gives them the tag "BOT"); by name inside each, case-insensitive, then by id
      // (one order for equal names: nothing of a player's rank shows).
      public static function byClanAndName(a:Object, b:Object) : int
      {
         var order:int = compare(groupOf(a.name),groupOf(b.name));
         if(order == 0)
         {
            order = compare(clanOf(a.name).toLowerCase(),clanOf(b.name).toLowerCase());
         }
         if(order == 0)
         {
            order = compare(String(a.name).toLowerCase(),String(b.name).toLowerCase());
         }
         return order != 0 ? order : compare(Number(a.id),Number(b.id));
      }
      
      // 0 a clan's player, 1 without a clan, 2 a bot.
      private static function groupOf(name:String) : int
      {
         var clan:String = clanOf(name);
         return clan == "" ? 1 : (clan.toUpperCase() == "BOT" ? 2 : 0);
      }
      
      private static function compare(a:*, b:*) : int
      {
         return a < b ? -1 : (a > b ? 1 : 0);
      }
      
      // The columns' headers: the icons the timeline puts over each list, every child but the team
      // names and the two lists.
      private function showHeaders(shown:Boolean) : void
      {
         for(var i:int = 0; i < numChildren; i++)
         {
            var child:DisplayObject = getChildAt(i);
            if(child != this.team1 && child != this.team2 && child != this.team1list && child != this.team2list)
            {
               child.visible = shown;
            }
         }
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
               if(player.kills == origin.kills && player.deaths == origin.deaths && player.ping == origin.ping && player.artifacts == origin.artifacts && player.rank == origin.rank)
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
      
      private function sortTeam(team:DataProvider) : void
      {
         for(var i:uint = 0; i < team.length; i++)
         {
            trace("team",i,team[i].name,team[i].score,team[i].rank,team[i].kills,team[i].deaths);
         }
         trace("HUD_PL::sort",team);
         if(this._namesOnly)
         {
            team.sort(PlayerList.byClanAndName);
         }
         else if(this._ranked)
         {
            team.sort(PlayerList.byStanding);
         }
         else
         {
            team.sortOn(["score","rank","kills","deaths"],Array.NUMERIC | Array.DESCENDING);
         }
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
            if(!this._ranked)
            {
               (this._players[i].team == TeamSide.ALLY ? this._team1 : this._team2).push(this._players[i]);
            }
         }
         if(this._ranked)
         {
            var ranking:Array = this._players.concat();
            ranking.sort(this._namesOnly ? PlayerList.byClanAndName : PlayerList.byStanding);
            _team1 = ranking.slice(0,COLUMN_ROWS);
            _team2 = ranking.slice(COLUMN_ROWS);
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

