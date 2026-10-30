package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 60 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon60 extends BitmapData
   {
      public function KillFeedWeapon60(w:int = 62, h:int = 16)
      {
         super(w,h);
      }
   }
}
