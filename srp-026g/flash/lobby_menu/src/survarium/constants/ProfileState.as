package survarium.constants
{
   // A profile's (kit's) state in setup_player_profiles (docs/notes/lobby.md, profile record: the exe passes
   // 2 on and works out 0 or 1 from the slots).
   public class ProfileState
   {
      
      public static const COMPLETE:int = 0;
      
      // shows the profile's hint_text
      public static const INCOMPLETE:int = 1;
      
      // st_premium_profile_hint
      public static const PREMIUM_ONLY:int = 2;
      
      public function ProfileState()
      {
         super();
      }
   }
}
