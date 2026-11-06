package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 65 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon65 extends BitmapData
   {
      public function KillFeedWeapon65(w:int = 71, h:int = 24)
      {
         super(w,h);
      }
   }
}
