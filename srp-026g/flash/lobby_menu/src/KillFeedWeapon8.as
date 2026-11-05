package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 8 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon8 extends BitmapData
   {
      public function KillFeedWeapon8(w:int = 63, h:int = 15)
      {
         super(w,h);
      }
   }
}
