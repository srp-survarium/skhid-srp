package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 32 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon32 extends BitmapData
   {
      public function KillFeedWeapon32(w:int = 56, h:int = 14)
      {
         super(w,h);
      }
   }
}
