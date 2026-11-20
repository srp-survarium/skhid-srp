package survarium.friends
{
   import flash.external.ExternalInterface;
   import survarium.data.LocalizationData;

   // The chat mute entries of a player's menu (docs/notes/chat-mute.md): the two that change the
   // player's state, sent as srp's 49 05 extension (comment_about_user: nickname, kind 254, mode). The
   // server keeps the choice and pushes every muted player in the ignore list, a player shown as
   // "<muted>" with its id marked (SHOWN_MUTED). The hud's Tab list menu (PlayerRowMenu) offers the
   // same entries with the same texts.
   public class ChatMute
   {
      public static const ACTION:uint = 254;
      public static const OFF:uint = 0;
      public static const HIDDEN:uint = 1;
      public static const PLACEHOLDER:uint = 2;

      // the lobby's mark on an ignore list id: shown as "<muted>", not hidden
      public static const SHOWN_MUTED:uint = 0x80000000;

      private static const CHOICES:Array = [{mode:HIDDEN,key:"st_srp_hud_mute_hide"},
                                            {mode:PLACEHOLDER,key:"st_srp_hud_mute_mask"},
                                            {mode:OFF,key:"st_srp_hud_unmute"}];

      // How `nickname` is muted now, by the ignore list the server pushed: OFF, HIDDEN or PLACEHOLDER.
      public static function modeOf(nickname:String):uint
      {
         var listed:Object = FriendsMenu.getIgnoredPlayer(nickname);
         if(listed == null)
         {
            return OFF;
         }
         return (uint(listed.id) & SHOWN_MUTED) != 0 ? PLACEHOLDER : HIDDEN;
      }

      public static function options(nickname:String):Array
      {
         var now:uint = modeOf(nickname);
         var out:Array = [];
         for each(var choice:Object in CHOICES)
         {
            if(choice.mode != now)
            {
               out.push(option(nickname,choice.mode,choice.key));
            }
         }
         return out;
      }

      private static function option(nickname:String, mode:uint, key:String):Object
      {
         return {name:LocalizationData.getName(key), click_function:function():void
         {
            ExternalInterface.call("comment_about_user",nickname,ACTION,mode);
         }};
      }
   }
}
