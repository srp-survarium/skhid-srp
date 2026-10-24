package survarium.constants
{
   // A match's game mode (docs/notes/modes_maps.md).
   public class GameMode
   {
      
      // batteries ("Protective device")
      public static const GATHER_VICTORY_ITEMS:int = 2;
      
      // timed batteries with chargers
      public static const VICTORY_ITEMS_SPAWN:int = 3;
      
      public static const TEAM_DEATHMATCH:int = 4;
      
      // Research: capture the research units (docs/notes/research_mode.md)
      public static const RESEARCH:int = 5;
      
      // Blackout ("Batteries: Blackout", docs/notes/blackout.md): the lobby's own mode, its
      // matches the plain battery mode with no interface and the teams' armbands
      public static const BLACKOUT:int = 6;
      
      public function GameMode()
      {
         super();
      }
   }
}
