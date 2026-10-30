package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 25 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon25 extends BitmapData
   {
      public function KillFeedWeapon25(w:int = 40, h:int = 23)
      {
         super(w,h);
      }
   }
}
