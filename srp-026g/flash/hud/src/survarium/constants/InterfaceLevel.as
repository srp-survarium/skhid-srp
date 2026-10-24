package survarium.constants
{
   // The interface's level in a match: the gameplay option g_interface_level (exe patch 64 gives it
   // to the hud and the player icons as root.set_interface_level; docs/notes/interface-level.md).
   // Disabled hides the whole interface, as F11 does. Minimal keeps what a fight needs.
   //
   // SHOWN is what each level shows, by element: the hud's property, [Full, Minimal]. Moving an
   // element between the levels is a change of its row. An element not here is shown at Full and
   // Minimal (the hit and damage marks, the messages of the match, the kits, the Tab list). The
   // player icons movie has the same table for its elements (names, markers, Research's markers).
   public class InterfaceLevel
   {

      public static const FULL:uint = 0;

      public static const MINIMAL:uint = 1;

      public static const DISABLED:uint = 2;

      public static const SHOWN:Object = {
         "crosshair":[true,true],            // the crosshair and its hit mark
         "playerInfo":[true,true],           // health, armour, the body, ammo and the weapon
         "quick_slots":[true,true],          // grenades and the quick slots
         "progress_indicator":[true,true],   // the mode's score line and time
         "researchUnits":[true,true],        // Research's units and their timers
         "container_icon":[true,true],       // the battery carried
         "chat":[true,true],                 // the match chat (its own option hides it)
         "combatLog":[true,false],           // the kill feed
         "minimap":[true,false],             // the minimap
         "questList":[true,false],           // the quests' progress
         "questTaskList":[true,false]        // the quests' tasks beside the Tab list
      };

      public function InterfaceLevel()
      {
         super();
      }

      // Whether `level` shows the hud's element `name`.
      public static function shows(name:String, level:uint) : Boolean
      {
         var row:Array = SHOWN[name] as Array;
         return level != DISABLED && (row == null || level >= row.length || row[level]);
      }
   }
}
