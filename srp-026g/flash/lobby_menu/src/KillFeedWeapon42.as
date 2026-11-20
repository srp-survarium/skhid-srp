package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 42 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon42 extends BitmapData
   {
      public function KillFeedWeapon42(w:int = 62, h:int = 22)
      {
         super(w,h);
      }
   }
}
