package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 36 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon36 extends BitmapData
   {
      public function KillFeedWeapon36(w:int = 24, h:int = 17)
      {
         super(w,h);
      }
   }
}
