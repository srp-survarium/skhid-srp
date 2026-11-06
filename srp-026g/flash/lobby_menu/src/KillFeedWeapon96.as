package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 96 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon96 extends BitmapData
   {
      public function KillFeedWeapon96(w:int = 49, h:int = 17)
      {
         super(w,h);
      }
   }
}
