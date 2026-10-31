package
{
   import flash.display.*;
   import flash.events.*;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.net.URLRequest;
   import flash.system.*;
   import flash.text.TextField;
   import flash.utils.getQualifiedClassName;

   // The names on the Tab map (srp-026g/docs/notes/tab-map-names.md) with the hud movie's own Tab
   // list (SrvPlayerList: its map, MineMinimapTab, a survarium.controls.Minimap) and corner minimap
   // (SrvMinimap), fed as HUD feeds them: list_set_players gives the list and the Tab map the rows,
   // update_players and update_local_player both maps, show_map_names (the exe's patch 82, Alt) the
   // Tab map alone. The hud's root does not run outside the game (its constructor fails in Ruffle),
   // so its three calls are the harness's, as HUD.as has them. Checks:
   // - no Alt: no names; Alt held: each icon of another player its place's name, centred over it,
   //   the own icon none, a place the list has no row for none, the corner minimap none;
   // - the names follow the icons as they move, the same text fields (none made again), over many
   //   updates; Alt let go hides them, held again shows the same ones;
   // - a player that leaves the data loses its icon and name; one that comes (an enemy spotted)
   //   gets its name; an ally taking the battery (another icon) keeps its name; the list's rows
   //   made again rename;
   // - Blackout: its Tab keeps the list hidden (HUD.show_player_list, forced), and the names are in
   //   the list's map: none on screen with Alt held.
   // Prints "NAMES ..." lines and ends with "NAMES RESULT pass" or "NAMES RESULT fail: ...".
   public class TabMapNamesHarness extends Sprite
   {
      private var hud:Loader = new Loader();
      private var initialized:Boolean = false;
      private var failures:Array = [];
      private var domain:ApplicationDomain;
      private var list:*;
      private var tabMap:*;
      private var corner:*;
      private var rows:Array;

      public function TabMapNamesHarness()
      {
         hud.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
            function(e:UncaughtErrorEvent):void { e.preventDefault(); });
         hud.load(new URLRequest("hud.swf"),new LoaderContext(false,new ApplicationDomain()));
         addEventListener(Event.ENTER_FRAME,tick);
      }

      private function check(ok:Boolean, message:String):void
      {
         trace("NAMES " + (ok ? "ok " : "FAIL ") + message);
         if(!ok) failures.push(message);
      }

      private static function player(id:int, team:uint, x:Number, y:Number, local:Boolean = false,
                                     carrying:Boolean = false):Object
      {
         return {player_id:id,team:team,player_pos_x:x,player_pos_y:y,player_rotation_in_rad:0,
            is_local_player:local,is_carrying_item:carrying,is_squad_player:false};
      }

      // HUD.list_set_players, update_players, update_local_player and show_map_names.
      private function listSetPlayers(value:Array):void
      {
         rows = value;
         list.players.addPlayersList(value);
         tabMap.setPlayerNames(value);
      }

      private function updatePlayers(value:Array):void
      {
         corner.setPlayersData(value);
         tabMap.setPlayersData(value);
      }

      private function updateLocal(x:Number, y:Number, team:uint):void
      {
         corner.updatePlayerPos(x,y,0,team);
         tabMap.updatePlayerPos(x,y,0,team);
      }

      private function showMapNames(shown:Boolean):void
      {
         tabMap.namesShown = shown;
      }

      // A map's player icons by place (a MinimapObjectPlayer's index is its player_id).
      private function icons(map:*):Object
      {
         var Icon:Class = domain.getDefinition("survarium.controls.MinimapObjectPlayer") as Class;
         var out:Object = {};
         var players:DisplayObjectContainer = map.minimap._players;
         for(var i:int = 0; i < players.numChildren; i++)
         {
            var child:DisplayObject = players.getChildAt(i);
            if(child is Icon) out[Object(child).index] = child;
         }
         return out;
      }

      // The names a map shows, place=name, sorted; only those on screen (every parent visible).
      private function names(map:*, onScreen:Boolean = false):String
      {
         var out:Array = [];
         var all:Object = icons(map);
         for(var place:String in all)
         {
            var name:String = all[place].shownName;
            if(name != null && (!onScreen || visibleOnStage(label(all[place])))) out.push(place + "=" + name);
         }
         return out.sort().join(",");
      }

      private static function label(icon:DisplayObjectContainer):TextField
      {
         for(var i:int = 0; i < icon.numChildren; i++)
         {
            if(icon.getChildAt(i) is TextField) return icon.getChildAt(i) as TextField;
         }
         return null;
      }

      private static function visibleOnStage(object:DisplayObject):Boolean
      {
         for(var o:DisplayObject = object; o != null; o = o.parent)
         {
            if(!o.visible) return false;
         }
         return object != null && object.stage != null;
      }

      // The label centred over its icon: its centre on the icon's x, its bottom 2 px into the icon's
      // top (the icon's own bounds: without the label, which bounds count even hidden).
      private static function placed(icon:DisplayObjectContainer):Boolean
      {
         var text:TextField = label(icon);
         var at:int = icon.getChildIndex(text);
         icon.removeChild(text);
         var own:Rectangle = icon.getBounds(icon);
         icon.addChildAt(text,at);
         return Math.abs(text.x + text.width / 2) < 0.5 && Math.abs(text.y + text.height - 2 - own.top) < 0.5
            && text.width > 10 && text.height > 5;
      }

      private function checkNames():void
      {
         var List:Class = domain.getDefinition("SrvPlayerList") as Class;
         var Corner:Class = domain.getDefinition("SrvMinimap") as Class;
         list = new List();
         list.scaleX = list.scaleY = 0.6;
         addChild(list);
         tabMap = list.minimap;
         tabMap.centerPlayer = false;
         tabMap.setProps({map:"level_03"});
         corner = new Corner();
         corner.x = 140; corner.y = 580;
         addChild(corner);
         corner.setProps({map:"level_03"});
         check(tabMap != null && corner != null,"the Tab list's map (" + getQualifiedClassName(tabMap) + ") and the corner minimap");

         // places 0 (own, team 0), 1 and 3 allies, 2 an enemy spotted; 5 an ally with no row
         listSetPlayers([{id:0,team:1,name:"[SRP] Self"},{id:1,team:1,name:"Ally"},
            {id:2,team:2,name:"[KLAN] Enemy"},{id:3,team:1,name:"Medic"},{id:4,team:2,name:"Unseen"}]);
         updateLocal(10,10,0);
         var data:Array = [player(0,0,10,10,true),player(1,0,30,10),player(2,1,-20,-40),player(3,0,15,12),
            player(5,0,-40,30)];
         updatePlayers(data);
         check(names(tabMap) == "","no Alt: no names (" + names(tabMap) + ")");
         showMapNames(true);
         check(names(tabMap) == "1=Ally,2=[KLAN] Enemy,3=Medic","Alt held: the allies' and the spotted enemy's names, "
            + "the own icon none, a place with no row none (" + names(tabMap) + ")");
         check(names(corner) == "","the corner minimap shows none (" + names(corner) + ")");
         var shown:Object = icons(tabMap);
         check(placed(shown[1]) && placed(shown[2]) && placed(shown[3]),"each name centred just over its icon");
         var text:TextField = label(shown[1]);
         var before:Point = text.localToGlobal(new Point(0,0));
         var iconBefore:Point = shown[1].localToGlobal(new Point(0,0));
         var children:int = shown[1].numChildren;
         for(var i:int = 0; i < 100; i++)
         {
            data[1] = player(1,0,30 + i * 0.5,10 - i * 0.25);
            updatePlayers(data);
         }
         var moved:Object = icons(tabMap);
         var after:Point = label(moved[1]).localToGlobal(new Point(0,0));
         var iconAfter:Point = moved[1].localToGlobal(new Point(0,0));
         check(moved[1] == shown[1] && label(moved[1]) == text && moved[1].numChildren == children,
            "100 updates: the same icon and text field, nothing made again");
         check(Math.abs(after.x - before.x - (iconAfter.x - iconBefore.x)) < 0.01
            && Math.abs(after.y - before.y - (iconAfter.y - iconBefore.y)) < 0.01 && iconAfter.x != iconBefore.x,
            "the name moves with its icon (" + int(iconAfter.x - iconBefore.x) + "," + int(iconAfter.y - iconBefore.y) + ")");

         showMapNames(false);
         check(names(tabMap) == "","Alt let go: no names (" + names(tabMap) + ")");
         showMapNames(true);
         check(names(tabMap) == "1=Ally,2=[KLAN] Enemy,3=Medic" && label(icons(tabMap)[1]) == text,
            "Alt again: the same names, the same text field");

         // the enemy no longer spotted, place 4 spotted, the ally at 1 takes the battery
         data = [player(0,0,10,10,true),player(1,0,40,5,false,true),player(3,0,15,12),player(4,1,0,-30)];
         updatePlayers(data);
         check(names(tabMap) == "1=Ally,3=Medic,4=Unseen","the enemy lost from sight loses its name, the one spotted "
            + "gets its own, the ally carrying (" + getQualifiedClassName(icons(tabMap)[1]) + ") keeps its ("
            + names(tabMap) + ")");
         check(placed(icons(tabMap)[1]) && placed(icons(tabMap)[4]),"the new icons' names centred over them");

         // the list made again (a player left and another took place 3)
         listSetPlayers([{id:0,team:1,name:"[SRP] Self"},{id:1,team:1,name:"Ally"},{id:3,team:1,name:"Newcomer"},
            {id:4,team:2,name:"Unseen"}]);
         check(names(tabMap) == "1=Ally,3=Newcomer,4=Unseen","the rows made again rename place 3 (" + names(tabMap) + ")");
         check(placed(icons(tabMap)[3]),"the renamed one centred again");
         check(names(tabMap,true) == "1=Ally,3=Newcomer,4=Unseen","Tab (the list shown): the names on screen");

         // Blackout: HUD.show_player_list, forced, never shows the list
         list.visible = false;
         check(names(tabMap,true) == "","Blackout's Tab (the list kept hidden), Alt held: no name on screen ("
            + names(tabMap,true) + ")");
         list.visible = true;
         trace("NAMES screenshot");
      }

      private function tick(e:Event):void
      {
         try
         {
            if(initialized || hud.content == null) return;
            initialized = true;
            domain = hud.contentLoaderInfo.applicationDomain;
            checkNames();
         }
         catch(error:Error)
         {
            failures.push(error.getStackTrace());
         }
         trace("NAMES RESULT " + (failures.length == 0 ? "pass" : "fail: " + failures.join("; ")));
      }
   }
}
