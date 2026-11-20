package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 40 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon40 extends BitmapData
   {
      public function KillFeedWeapon40(w:int = 56, h:int = 14)
      {
         super(w,h);
      }
   }
}
