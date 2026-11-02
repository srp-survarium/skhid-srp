package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 72 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon72 extends BitmapData
   {
      public function KillFeedWeapon72(w:int = 59, h:int = 28)
      {
         super(w,h);
      }
   }
}
