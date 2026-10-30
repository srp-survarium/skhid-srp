package
{
   import flash.display.BitmapData;

   // The kill feed's weapon icon 21 (ui_desc/combat_log_icon), from the hud's: made by
   // scripts/kill-feed-icons/make_lobby_weapon_icons.py (bitmaps/, which the build adds).
   public dynamic class KillFeedWeapon21 extends BitmapData
   {
      public function KillFeedWeapon21(w:int = 59, h:int = 21)
      {
         super(w,h);
      }
   }
}
