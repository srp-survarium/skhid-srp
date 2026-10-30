package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 18 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon18 extends BitmapData
   {
      public function KillFeedWeapon18(w:int = 50, h:int = 18)
      {
         super(w,h);
      }
   }
}
