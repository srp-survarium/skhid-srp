package survarium.menu.selector
{
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;

   // The selector's own texts in the menu's look: the Settings window's Calibri Bold and its inks
   // (the title's, a stock key row's label), the kinds' colours only as accents.
   public class Paint
   {

      // the Settings window's title (textFieldLabel) and the tab's headings
      public static const TITLE:uint = 0xC6D0D1;

      // a stock key row's label (SrvBindKeyListRender's textField)
      public static const LABEL:uint = 0xABACAE;

      public static const DIM:uint = 0x7C8084;

      public static const RULE:uint = 0x3A3F44;

      public static const WARNING:uint = 0xE3A24A;

      // the movie's embedded Calibri Bold (the stock rows' and MessageApply's), its every language's glyphs
      public static const FONT:String = "Calibri";

      public function Paint()
      {
         super();
      }

      // `a` with `part` (0..1) of `b`.
      public static function mix(a:uint, b:uint, part:Number) : uint
      {
         var r:uint = Math.round((a >> 16 & 0xFF) * (1 - part) + (b >> 16 & 0xFF) * part);
         var g:uint = Math.round((a >> 8 & 0xFF) * (1 - part) + (b >> 8 & 0xFF) * part);
         var bl:uint = Math.round((a & 0xFF) * (1 - part) + (b & 0xFF) * part);
         return r << 16 | g << 8 | bl;
      }

      public static function format(size:Number, colour:uint) : TextFormat
      {
         return new TextFormat(FONT,size,colour,true);
      }

      // A one-line text, sized to what it holds, with the stock texts' `filters` (their shadow).
      public static function text(size:Number, colour:uint, filters:Array = null) : TextField
      {
         var field:TextField = new TextField();
         field.embedFonts = true;
         field.selectable = false;
         field.mouseEnabled = false;
         field.multiline = false;
         field.wordWrap = false;
         field.autoSize = TextFieldAutoSize.LEFT;
         field.defaultTextFormat = format(size,colour);
         if(filters != null)
         {
            field.filters = filters;
         }
         return field;
      }

      // `field` holds `value`, cut with an ellipsis to `width` at most.
      public static function fit(field:TextField, value:String, width:Number) : void
      {
         field.text = value;
         if(field.textWidth + 4 <= width)
         {
            return;
         }
         var low:int = 0;
         var high:int = value.length;
         while(low < high)
         {
            var middle:int = (low + high + 1) >> 1;
            field.text = value.substr(0,middle) + "…";
            if(field.textWidth + 4 <= width)
            {
               low = middle;
            }
            else
            {
               high = middle - 1;
            }
         }
         field.text = value.substr(0,low).replace(/\s+$/,"") + "…";
      }
   }
}
