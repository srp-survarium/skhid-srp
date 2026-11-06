package survarium.modes
{
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import survarium.data.LocalizationData;

   // The native HUD localization push supplies these keys in every supported language.
   public class ModeIntroText
   {
      public static function chat(mode:String) : String
      {
         return mode == "blackout" ? LocalizationData.getName("st_srp_hud_blackout_intro") : "";
      }

      public static function start(mode:String) : String
      {
         return mode == "blackout" ? LocalizationData.getName("st_srp_hud_blackout_start") : "";
      }

      public static function ready(mode:String) : Boolean
      {
         return mode == "blackout" && chat(mode).indexOf("#st_srp_") != 0;
      }

      public static function pregame(label:TextField, mode:String, value:String) : void
      {
         label.autoSize = TextFieldAutoSize.LEFT;
         var introduction:Boolean = mode != null && ready(mode) && value != "";
         label.wordWrap = introduction;
         if(introduction)
         {
            label.multiline = true;
            label.width = 600;
         }
         label.text = introduction ? start(mode) + "\n" + value : value;
      }
   }
}
