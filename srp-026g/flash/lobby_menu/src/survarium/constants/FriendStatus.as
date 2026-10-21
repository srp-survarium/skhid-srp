package survarium.constants
{
   // A friend list row's status (FriendsMenu._statusMap; the server's social.rs: LOBBY, IN_GAME, OFFLINE).
   public class FriendStatus
   {
      
      // the movie's own: a group header row
      public static const SEPARATOR:int = -1;
      
      public static const LOBBY:int = 0;
      
      public static const IN_GAME:int = 1;
      
      public static const OFFLINE:int = 2;
      
      public static const AWAY:int = 3;
      
      // a friend request
      public static const INVITE:int = 4;
      
      public function FriendStatus()
      {
         super();
      }
   }
}
