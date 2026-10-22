package survarium.constants
{
   // The clan window's state (set_clan_window_state; docs/notes/social.md). The developers called
   // them CLAN_RESERVE_STATE ... CLAN_MANAGE_STATE.
   public class ClanState
   {
      
      // no clan (or its name was refused)
      public static const RESERVE:int = 0;
      
      // the founder of a clan being registered
      public static const REGISTRATION:int = 1;
      
      // invited to a clan being registered
      public static const INVITED:int = 2;
      
      // a member of a registered clan
      public static const OVERVIEW:int = 3;
      
      // commands a registered clan
      public static const MANAGE:int = 4;
      
      public function ClanState()
      {
         super();
      }
   }
}
