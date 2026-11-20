package
{
   import flash.display.*;
   import flash.events.*;
   import flash.net.URLRequest;
   import flash.system.*;
   import flash.utils.getQualifiedClassName;

   // The hud's minimap (survarium.controls.Minimap, as the hud's SrvMinimap):
   // a player's icon follows the data of every update_players, also when the
   // local team (update_local_player) comes after the other players, as for a
   // game that joined late on team 0.
   public class MinimapHarness extends Sprite
   {
      private var hud:Loader = new Loader();
      private var initialized:Boolean = false;
      private static var lastIcons:String = "";

      public function MinimapHarness()
      {
         // The game's HUD root expects game callbacks; test its controls alone.
         hud.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
            function(e:UncaughtErrorEvent):void { e.preventDefault(); });
         hud.load(new URLRequest("hud.swf"),new LoaderContext(false,new ApplicationDomain()));
         addEventListener(Event.ENTER_FRAME,tick);
      }

      private function check(ok:Boolean, message:String):void
      {
         if(!ok) throw new Error(message + " (icons: " + lastIcons + ")");
         trace("PASS " + message);
      }

      private static function player(id:int, team:uint, carrying:Boolean = false, squad:Boolean = false):Object
      {
         return {player_id:id,team:team,player_pos_x:0.5,player_pos_y:0.5,player_rotation_in_rad:0,
            is_local_player:false,is_carrying_item:carrying,is_squad_player:squad};
      }

      // The class names of the player icons (the layer holds a timeline clip too),
      // in the order the minimap holds them.
      private function icons(minimap:*):Array
      {
         var icon:Class = hud.contentLoaderInfo.applicationDomain.getDefinition("survarium.controls.MinimapObjectPlayer") as Class;
         var names:Array = [];
         var players:DisplayObjectContainer = minimap.minimap._players;
         for(var i:int = 0; i < players.numChildren; i++)
         {
            if(players.getChildAt(i) is icon) names.push(getQualifiedClassName(players.getChildAt(i)));
         }
         lastIcons = names.join();
         return names;
      }

      private function checkMinimap():void
      {
         var cls:Class = hud.contentLoaderInfo.applicationDomain.getDefinition("SrvMinimap") as Class;
         var minimap:* = new cls(); addChild(minimap);
         var ally:Object = player(0,0);
         var enemy:Object = player(1,1);
         // A team-0 game that joined late: its ally's data comes before its own team.
         minimap.setPlayersData([ally]);
         check(icons(minimap).join() == "PlayerRed","before the local team, the minimap's first team (1) shows a team-0 ally as an enemy");
         minimap.updatePlayerPos(0.5,0.5,0,0);
         minimap.setPlayersData([ally]);
         check(icons(minimap).join() == "PlayerBlue","the ally's icon turns blue once the team arrives");
         minimap.setPlayersData([ally,enemy]);
         check(icons(minimap).join() == "PlayerBlue,PlayerRed","an enemy keeps the enemy's icon");
         ally.is_carrying_item = true;
         minimap.setPlayersData([ally,enemy]);
         check(icons(minimap).sort().join() == "PlayerCarry,PlayerRed","an ally that takes the battery carries it");
         ally.is_carrying_item = false;
         enemy.is_carrying_item = true;
         minimap.setPlayersData([ally,enemy]);
         check(icons(minimap).sort().join() == "PlayerBlue,PlayerRed","and drops it; an enemy carrier is an enemy");
         minimap.setPlayersData([enemy]);
         check(icons(minimap).join() == "PlayerRed","a player left out of the data loses its icon");
         // A team-1 game had the minimap's first team by chance: unchanged.
         var other:* = new cls(); addChild(other);
         other.setPlayersData([player(0,1)]);
         check(icons(other).join() == "PlayerBlue","a team-1 ally before the local team, as before");
         other.updatePlayerPos(0.5,0.5,0,1);
         other.setPlayersData([player(0,1)]);
         check(icons(other).join() == "PlayerBlue","and after it");
         removeChild(minimap); removeChild(other);
      }

      private function tick(e:Event):void
      {
         try
         {
            if(initialized || hud.content == null) return;
            initialized = true;
            checkMinimap();
            trace("MINIMAP PASS");
         }
         catch(error:Error)
         {
            trace("MINIMAP FAIL " + error.message);
         }
      }
   }
}
