package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 50 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon50 extends BitmapData
   {
      public function KillFeedWeapon50(w:int = 58, h:int = 24)
      {
         super(w,h);
      }
   }
}
