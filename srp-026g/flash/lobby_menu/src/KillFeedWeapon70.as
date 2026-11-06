package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 70 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon70 extends BitmapData
   {
      public function KillFeedWeapon70(w:int = 61, h:int = 26)
      {
         super(w,h);
      }
   }
}
