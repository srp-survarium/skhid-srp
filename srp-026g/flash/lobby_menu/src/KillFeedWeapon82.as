package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 82 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon82 extends BitmapData
   {
      public function KillFeedWeapon82(w:int = 21, h:int = 15)
      {
         super(w,h);
      }
   }
}
