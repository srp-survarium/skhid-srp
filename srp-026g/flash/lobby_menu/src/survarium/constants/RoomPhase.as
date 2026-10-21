package survarium.constants
{
   // A room's phase (the server's rooms.rs).
   public class RoomPhase
   {
      
      public static const WAITING:int = 0;
      
      // not sent since 2026-10-19
      public static const COUNTDOWN:int = 1;
      
      public static const IN_MATCH:int = 2;
      
      public static const VOTE:int = 3;
      
      public function RoomPhase()
      {
         super();
      }
   }
}
