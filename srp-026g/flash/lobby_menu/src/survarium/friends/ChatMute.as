package survarium.friends
{
   import flash.external.ExternalInterface;
   import survarium.data.LocalizationData;

   // "Mute chat" of a player's menu (docs/notes/chat-mute.md): its three choices, sent as srp's 49 05
   // extension (comment_about_user: nickname, kind 254, mode). The server keeps the choice; the hud's
   // Tab list menu (PlayerRowMenu) offers the same choices with the same texts.
   public class ChatMute
   {
      public static const ACTION:uint = 254;
      public static const OFF:uint = 0;
      public static const HIDDEN:uint = 1;
      public static const PLACEHOLDER:uint = 2;

      public static const LABEL:String = "st_srp_hud_mute_chat";

      public static function options(nickname:String):Array
      {
         return [option(nickname,HIDDEN,"st_srp_hud_mute_hide"),
                 option(nickname,PLACEHOLDER,"st_srp_hud_mute_mask"),
                 option(nickname,OFF,"st_srp_hud_unmute")];
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
