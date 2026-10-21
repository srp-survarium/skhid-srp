package survarium.constants
{
   // The game's UI sounds, played with ExternalInterface.call("sound_play", id): the id is the
   // sound's index in gameplay/ui_sounds. Index 7 is not used by the movies.
   public class UiSound
   {
      
      public static const CLICK:int = 0;
      
      public static const HOVER:int = 1;
      
      public static const PLAY_CLICK_IN:int = 2;
      
      public static const PLAY_CLICK_OUT:int = 3;
      
      public static const TYPE:int = 4;
      
      public static const MESSAGE:int = 5;
      
      public static const INVITE:int = 6;
      
      public static const BACKSPACE:int = 8;
      
      public static const MENU_SLIDE_IN:int = 9;
      
      public static const MENU_SLIDE_OUT:int = 10;
      
      public static const DAMAGE_PIERCED:int = 11;
      
      public static const DAMAGE_NOT_PIERCED:int = 12;
      
      public static const MENU_MESSAGE:int = 13;
      
      public static const AMMO_PICK_UP:int = 14;
      
      public static const MATCH_FOUND:int = 15;
      
      public static const SQUAD_PANEL_SLIDE:int = 16;
      
      public static const TASK_PANEL_SLIDE:int = 17;
      
      public static const TUTORIAL_SUCCESS:int = 18;
      
      public function UiSound()
      {
         super();
      }
   }
}
