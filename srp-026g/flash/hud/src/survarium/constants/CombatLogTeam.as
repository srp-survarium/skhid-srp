package survarium.constants
{
   // A combat log name's who_team / victim_team: an index into CombatLogItem._colorCode (the exe,
   // 0x237973: SELF for the local player, else ENEMY when the team differs from the local player's).
   public class CombatLogTeam
   {
      
      public static const ALLY:int = 0;
      
      public static const ENEMY:int = 1;
      
      public static const SELF:int = 2;
      
      public function CombatLogTeam()
      {
         super();
      }
   }
}
