package survarium.modes
{
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import survarium.data.LocalizationData;

   // The native HUD localization push supplies these keys in every supported language.
   public class ModeIntroText
   {
      // Each mode's notice under the timer (ModeNotice): Blackout's (exe patch 65) and Freeze Tag's
      // (mode 12).
      private static const START:Object = {
         "blackout":"st_srp_hud_blackout_start",
         "freeze_tag":"st_srp_hud_freeze_start"
      };

      public static function chat(mode:String) : String
      {
         return mode == "blackout" ? LocalizationData.getName("st_srp_hud_blackout_intro") : "";
      }

      public static function start(mode:String) : String
      {
         return mode != null && START.hasOwnProperty(mode) ? LocalizationData.getName(START[mode]) : "";
      }

      // The mode's text has come (the dictionary's push; before it a key reads as "#st_srp_...").
      public static function ready(mode:String) : Boolean
      {
         var text:String = start(mode);
         return text != "" && text.indexOf("#st_srp_") != 0;
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
