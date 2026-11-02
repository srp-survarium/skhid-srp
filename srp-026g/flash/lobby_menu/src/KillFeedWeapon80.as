package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 80 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon80 extends BitmapData
   {
      public function KillFeedWeapon80(w:int = 69, h:int = 27)
      {
         super(w,h);
      }
   }
}
