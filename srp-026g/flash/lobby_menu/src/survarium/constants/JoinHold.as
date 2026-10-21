package survarium.constants
{
   // Why a room's match cannot be joined now (J;-1;<why>; the server's JoinHold).
   public class JoinHold
   {
      
      public static const NONE:int = 0;
      
      public static const JOINING:int = 1;
      
      public static const LOADING:int = 2;
      
      public static const LAST_SECONDS:int = 3;
      
      public static const FULL:int = 4;
      
      public static const MAINTENANCE:int = 5;
      
      public static const ENDING:int = 6;
      
      public function JoinHold()
      {
         super();
      }
   }
}
