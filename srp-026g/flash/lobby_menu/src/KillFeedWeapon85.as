package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 85 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon85 extends BitmapData
   {
      public function KillFeedWeapon85(w:int = 55, h:int = 11)
      {
         super(w,h);
      }
   }
}
