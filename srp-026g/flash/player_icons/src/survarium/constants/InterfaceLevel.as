package survarium.constants
{
   // The interface's level in a match: the gameplay option g_interface_level (exe patch 64 gives it
   // to the hud and the player icons as root.set_interface_level; docs/notes/interface-level.md).
   // Disabled hides the whole interface, as F11 does. Minimal keeps what a fight needs.
   //
   // SHOWN is what each level shows, by element of this movie: [Full, Minimal]. Moving an element
   // between the levels is a change of its row. The hud movie has the same table for its elements.
   public class InterfaceLevel
   {

      public static const FULL:uint = 0;

      public static const MINIMAL:uint = 1;

      public static const DISABLED:uint = 2;

      public static const SHOWN:Object = {
         "players":[true,true],              // the teammates' markers over their heads
         "names":[true,false],               // the players' names over their heads
         "markers":[true,true],              // the markers of the match's objects (batteries, spots)
         "research":[true,true],             // Research's units: letter and capture ring
         "researchText":[true,false]         // under a unit: the time left or the metres
      };

      public function InterfaceLevel()
      {
         super();
      }

      // Whether `level` shows this movie's element `name`.
      public static function shows(name:String, level:uint) : Boolean
      {
         var row:Array = SHOWN[name] as Array;
         return level != DISABLED && (row == null || level >= row.length || row[level]);
      }
   }
}
