package survarium.constants
{
   // A chat message channel: the exe's message_channel_enum (docs/notes/social.md; st_chat_channel_*).
   public class ChatChannel
   {
      
      public static const NONE:int = 0;
      
      public static const GENERAL:int = 1;
      
      public static const SYSTEM:int = 2;
      
      public static const CLAN:int = 3;
      
      public static const PRIVATE:int = 4;
      
      // "All"
      public static const MATCH:int = 5;
      
      // the exe sends TEAM_1 + (own team != 0)
      public static const TEAM_1:int = 6;
      
      public static const TEAM_2:int = 7;
      
      public static const SQUAD:int = 8;
      
      public static const CLAN_TEAM:int = 9;
      
      // srp's room chat
      public static const ROOM:int = 10;
      
      public function ChatChannel()
      {
         super();
      }
   }
}
