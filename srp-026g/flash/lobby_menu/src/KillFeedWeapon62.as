package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 62 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon62 extends BitmapData
   {
      public function KillFeedWeapon62(w:int = 82, h:int = 21)
      {
         super(w,h);
      }
   }
}
