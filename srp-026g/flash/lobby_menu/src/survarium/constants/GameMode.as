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
      
      // Blackout ("Protective device: blackout", docs/notes/blackout.md): the lobby's own mode, its
      // matches the plain battery mode with no interface but the players' markers
      public static const BLACKOUT:int = 6;
      
      // Scarcity ("Protective device: scarcity", docs/notes/scarcity.md): the lobby's own mode, its
      // matches the plain battery mode with a magazine and one more a gun, and no ammo crates
      public static const SCARCITY:int = 7;
      
      public function GameMode()
      {
         super();
      }
   }
}
