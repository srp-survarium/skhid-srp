package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 98 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon98 extends BitmapData
   {
      public function KillFeedWeapon98(w:int = 50, h:int = 13)
      {
         super(w,h);
      }
   }
}
