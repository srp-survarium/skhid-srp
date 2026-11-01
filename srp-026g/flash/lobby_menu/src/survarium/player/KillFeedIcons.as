package survarium.player
{
   import flash.display.BitmapData;
   import flash.utils.getDefinitionByName;

   // The kill feed's weapon icons by ui_desc/combat_log_icon: the class KillFeedWeapon<icon>
   // (src/, bitmaps/), made by scripts/kill-feed-icons/make_lobby_weapon_icons.py from the hud's own
   // images; a weapon whose icon has none (the clip's empty 17, a stock icon past it) gets null.
   public final class KillFeedIcons
   {
      // A new copy of the icon's bitmap, or null when there is none.
      public static function bitmap(icon:int) : BitmapData
      {
         if(icon < 0)
         {
            return null;
         }
         try
         {
            return new (getDefinitionByName("KillFeedWeapon" + icon) as Class)() as BitmapData;
         }
         catch(error:Error)
         {
         }
         return null;
      }
   }
}
