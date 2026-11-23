package survarium.menu.selector
{
   import flash.display.Graphics;
   import flash.display.Sprite;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;

   // The selector's look (the owner's key board, 2026-11-23: a dark board, each group its colour):
   // its colours, its text and its keycaps.
   public class Paint
   {

      public static const SHADE:uint = 0x0E1012;

      public static const PANEL:uint = 0x15181B;

      public static const KEY:uint = 0x1D2125;

      public static const KEY_TOP:uint = 0x262B30;

      public static const EDGE:uint = 0x2C3238;

      public static const INK:uint = 0xE8E4DA;

      public static const DIM:uint = 0x8A8F94;

      public static const FAINT:uint = 0x5A6066;

      public static const WARNING:uint = 0xF0B44C;

      public static const UNBIND:uint = 0xE06A5E;

      // the movie's embedded Calibri Bold (MessageApply's), its every language's glyphs
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

      // A one-line text, sized to what it holds.
      public static function text(size:Number, colour:uint, spacing:Number = 0) : TextField
      {
         var field:TextField = new TextField();
         field.embedFonts = true;
         field.selectable = false;
         field.mouseEnabled = false;
         field.multiline = false;
         field.wordWrap = false;
         field.autoSize = TextFieldAutoSize.LEFT;
         var format:TextFormat = Paint.format(size,colour);
         format.letterSpacing = spacing;
         field.defaultTextFormat = format;
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

      // A keycap holding `label` (`tint` its group's colour, or none: a free key), `height` tall,
      // at least as wide; its own sprite, its top left at 0,0.
      public static function keycap(label:String, height:Number, size:Number, tint:int = -1) : Sprite
      {
         var cap:Sprite = new Sprite();
         cap.mouseEnabled = cap.mouseChildren = false;
         var field:TextField = text(size,tint == -1 ? INK : mix(uint(tint),INK,0.35));
         field.text = label;
         var width:Number = Math.max(height,Math.ceil(field.textWidth) + 4 + Math.round(height * 0.55));
         var top:uint = tint == -1 ? KEY_TOP : mix(KEY_TOP,uint(tint),0.24);
         var bottom:uint = tint == -1 ? KEY : mix(KEY,uint(tint),0.14);
         var edge:uint = tint == -1 ? EDGE : mix(EDGE,uint(tint),0.55);
         var radius:Number = Math.max(5,Math.round(height * 0.2));
         var g:Graphics = cap.graphics;
         // its foot, the cap's darker underside
         g.beginFill(tint == -1 ? 0x0A0B0C : mix(0x0A0B0C,uint(tint),0.3));
         g.drawRoundRect(0,0,width,height,radius * 2,radius * 2);
         g.endFill();
         g.lineStyle(1,edge,1,true);
         g.beginFill(bottom);
         g.drawRoundRect(0.5,0.5,width - 1,height - Math.max(2,Math.round(height * 0.07)),radius * 2,radius * 2);
         g.endFill();
         g.lineStyle();
         g.beginFill(top,0.65);
         g.drawRoundRect(1.5,1.5,width - 3,(height - 3) * 0.55,(radius - 1) * 2,(radius - 1) * 2);
         g.endFill();
         field.x = Math.round((width - field.width) / 2);
         field.y = Math.round((height - Math.max(2,height * 0.07) - field.height) / 2);
         cap.addChild(field);
         return cap;
      }

      // A filled triangle pointing up or down, its box `size` wide at x, y.
      public static function arrow(g:Graphics, x:Number, y:Number, size:Number, up:Boolean, colour:uint) : void
      {
         g.beginFill(colour);
         if(up)
         {
            g.moveTo(x + size / 2,y);
            g.lineTo(x + size,y + size * 0.6);
            g.lineTo(x,y + size * 0.6);
         }
         else
         {
            g.moveTo(x,y);
            g.lineTo(x + size,y);
            g.lineTo(x + size / 2,y + size * 0.6);
         }
         g.endFill();
      }
   }
}
