package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 30 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon30 extends BitmapData
   {
      public function KillFeedWeapon30(w:int = 31, h:int = 18)
      {
         super(w,h);
      }
   }
}
