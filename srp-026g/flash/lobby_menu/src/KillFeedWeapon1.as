package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 1 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon1 extends BitmapData
   {
      public function KillFeedWeapon1(w:int = 43, h:int = 18)
      {
         super(w,h);
      }
   }
}
