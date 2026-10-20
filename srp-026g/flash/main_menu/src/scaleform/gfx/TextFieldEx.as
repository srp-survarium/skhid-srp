package scaleform.gfx
{
   import flash.display.BitmapData;
   import flash.text.TextField;
   
   public final class TextFieldEx extends InteractiveObjectEx
   {
      
      public static const VALIGN_NONE:String = "none";
      
      public static const VALIGN_TOP:String = "top";
      
      public static const VALIGN_CENTER:String = "center";
      
      public static const VALIGN_BOTTOM:String = "bottom";
      
      public static const TEXTAUTOSZ_NONE:String = "none";
      
      public static const TEXTAUTOSZ_SHRINK:String = "shrink";
      
      public static const TEXTAUTOSZ_FIT:String = "fit";
      
      public function TextFieldEx()
      {
         super();
      }
      
      public static function appendHtml(param1:TextField, param2:String) : void
      {
      }
      
      public static function setIMEEnabled(param1:TextField, param2:Boolean) : void
      {
      }
      
      public static function setVerticalAlign(param1:TextField, param2:String) : void
      {
      }
      
      public static function getVerticalAlign(param1:TextField) : String
      {
         return "none";
      }
      
      public static function setTextAutoSize(param1:TextField, param2:String) : void
      {
      }
      
      public static function getTextAutoSize(param1:TextField) : String
      {
         return "none";
      }
      
      public static function setImageSubstitutions(param1:TextField, param2:Object) : void
      {
      }
      
      public static function updateImageSubstitution(param1:TextField, param2:String, param3:BitmapData) : void
      {
      }
   }
}

