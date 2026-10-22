package survarium.constants
{
   // A notification's type: the renderer LobbyMenu registers for it.
   public class NotificationType
   {
      
      public static const PLAIN:int = 0;
      
      public static const QUEST:int = 1;
      
      // a referral reached a level (reach_level)
      public static const INVITED_REWARD:int = 2;
      
      // a match result
      public static const MATCH:int = 3;
      
      // premium activated (NotificationPremDiscount)
      public static const PREMIUM:int = 4;
      
      public static const FACTION_RANK:int = 8;
      
      // a skill level
      public static const PLAYER_RANK:int = 9;
      
      public function NotificationType()
      {
         super();
      }
   }
}
