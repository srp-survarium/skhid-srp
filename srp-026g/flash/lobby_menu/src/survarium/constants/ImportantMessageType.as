package survarium.constants
{
   // root.add_important_message's type (the exe's important_message_arrived 0x226020 maps the
   // server's types to these; docs/notes/social.md). 5 is never sent.
   public class ImportantMessageType
   {
      
      public static const FRIEND_REQUEST:int = 0;
      
      public static const SQUAD_INVITE:int = 1;
      
      public static const REPUTATION_LEVEL:int = 2;
      
      public static const CLAN_INVITE:int = 3;
      
      public static const CLAN_TEAM_INVITE:int = 4;
      
      public static const REFERRAL_REWARD:int = 6;
      
      public static const PREMIUM_ACTIVATED:int = 7;
      
      public static const SKILL_POINT_REWARD:int = 8;
      
      public function ImportantMessageType()
      {
         super();
      }
   }
}
