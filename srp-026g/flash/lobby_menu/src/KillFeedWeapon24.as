package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 24 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon24 extends BitmapData
   {
      public function KillFeedWeapon24(w:int = 57, h:int = 13)
      {
         super(w,h);
      }
   }
}
