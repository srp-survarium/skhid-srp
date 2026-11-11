package survarium.constants
{
   // A profile's (kit's) state in setup_player_profiles (docs/notes/lobby.md, profile record: the exe works
   // out 0 or 1 from the slots; 2, the stock premium-only kit, srp never sends: there is no premium).
   public class ProfileState
   {
      
      public static const COMPLETE:int = 0;
      
      // shows the profile's hint_text
      public static const INCOMPLETE:int = 1;
      
      public function ProfileState()
      {
         super();
      }
   }
}
