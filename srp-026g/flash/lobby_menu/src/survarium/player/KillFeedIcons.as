package survarium.player
{
   import flash.display.BitmapData;

   // The kill feed's weapon icons by ui_desc/combat_log_icon (null where the hud's clip has none):
   // made by scripts/kill-feed-icons/make_lobby_weapon_icons.py from the hud's own images.
   public final class KillFeedIcons
   {
      private static const ICONS:Array = [KillFeedWeapon0,KillFeedWeapon1,KillFeedWeapon2,KillFeedWeapon3,KillFeedWeapon4,KillFeedWeapon5,KillFeedWeapon6,KillFeedWeapon7,KillFeedWeapon8,KillFeedWeapon9,KillFeedWeapon10,KillFeedWeapon11,KillFeedWeapon12,KillFeedWeapon13,KillFeedWeapon14,KillFeedWeapon15,KillFeedWeapon16,null,KillFeedWeapon18,KillFeedWeapon19,KillFeedWeapon20,KillFeedWeapon21,KillFeedWeapon22,KillFeedWeapon23,KillFeedWeapon24,KillFeedWeapon25,KillFeedWeapon26,KillFeedWeapon27,KillFeedWeapon28,KillFeedWeapon29,KillFeedWeapon30,KillFeedWeapon31,KillFeedWeapon32,KillFeedWeapon33,KillFeedWeapon34,KillFeedWeapon35,KillFeedWeapon36,KillFeedWeapon37,KillFeedWeapon38,KillFeedWeapon39,KillFeedWeapon40,KillFeedWeapon41,KillFeedWeapon42,KillFeedWeapon43,KillFeedWeapon44,KillFeedWeapon45,KillFeedWeapon46,KillFeedWeapon47,KillFeedWeapon48,KillFeedWeapon49,KillFeedWeapon50,KillFeedWeapon51,KillFeedWeapon52,KillFeedWeapon53,KillFeedWeapon54,KillFeedWeapon55,KillFeedWeapon56,KillFeedWeapon57,KillFeedWeapon58,KillFeedWeapon59,KillFeedWeapon60,KillFeedWeapon61,KillFeedWeapon62,KillFeedWeapon63,KillFeedWeapon64,KillFeedWeapon65,KillFeedWeapon66,KillFeedWeapon67,KillFeedWeapon68,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,KillFeedWeapon85,KillFeedWeapon86,null,null,null,null,null,null,null,null,null,KillFeedWeapon96,KillFeedWeapon97,KillFeedWeapon98];
      // A new copy of the icon's bitmap, or null when there is none.
      public static function bitmap(icon:int) : BitmapData
      {
         var kind:Class = icon >= 0 && icon < ICONS.length ? ICONS[icon] as Class : null;
         return kind != null ? new kind() as BitmapData : null;
      }
   }
}
