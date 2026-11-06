package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import scaleform.clik.controls.TipRenderer;
   import survarium.core.Icon;

   // srp: the main menu's tip (the Settings' hints are its only use). Its frame is drawn in the symbol's
   // look (a 1 px grey edge round a near-black fill) but opaque: the symbol's fill let the rows under it
   // show through the text. It is sized to the text with the same padding on all four sides; the
   // renderer itself is never scaled (the symbol's scale9 art did not stretch evenly).
   public class ProfileTipRenderer extends TipRenderer
   {

      // Between the frame's edge and the text (or the icon), each side.
      protected static const PADDING:Number = 6;

      // A TextField's own gutter around its text.
      protected static const GUTTER:Number = 2;

      // The symbol's edge, and its fill over black.
      protected static const EDGE:uint = 0x999999;

      protected static const FILL:uint = 0x020404;

      public var textField:TextField;

      public var spriteAnchor:MovieClip;

      protected var _icon:Icon;

      protected var frame:Shape;

      // The fields this text added beside the symbol's (its other lines, its signs).
      protected var parts:Array = [];

      // Between the sign column and its lines' texts.
      protected static const SIGN_GAP:Number = 4;

      public function ProfileTipRenderer()
      {
         super();
         this._icon = new Icon();
         this.spriteAnchor.addChild(this._icon);
         removeChildAt(0);
         this.frame = new Shape();
         addChildAt(this.frame,0);
         scale9Grid = null;
         this.textField.multiline = false;
         this.textField.wordWrap = false;
         this.textField.autoSize = TextFieldAutoSize.LEFT;
      }

      // A line of a hint: "+ text" or "- text" is a signed line (its sign in a column of its own, so the
      // texts after "+" and "-" start at one x whatever the glyphs' widths), any other a plain one.
      protected static function signOf(line:String) : String
      {
         var sign:String = line.charAt(0);
         return (sign == "+" || sign == "-") && line.charAt(1) == " " ? sign : null;
      }

      // A one-line field in the symbol's text field's look.
      protected function lineField(text:String) : TextField
      {
         var field:TextField = new TextField();
         field.embedFonts = this.textField.embedFonts;
         field.antiAliasType = this.textField.antiAliasType;
         field.gridFitType = this.textField.gridFitType;
         field.sharpness = this.textField.sharpness;
         field.thickness = this.textField.thickness;
         field.filters = this.textField.filters;
         field.defaultTextFormat = this.textField.defaultTextFormat;
         field.selectable = false;
         field.mouseEnabled = false;
         field.autoSize = TextFieldAutoSize.LEFT;
         field.text = text;
         addChild(field);
         this.parts.push(field);
         return field;
      }

      override protected function invalidateData() : void
      {
         var part:TextField;
         for each(part in this.parts)
         {
            removeChild(part);
         }
         this.parts = [];
         var format:TextFormat = this.textField.defaultTextFormat;
         format.align = TextFormatAlign.LEFT;
         this.textField.defaultTextFormat = format;
         var icon:Boolean = _data.icon is Icon;
         if(icon)
         {
            this._icon.applyImage(_data.icon);
         }
         this.spriteAnchor.visible = icon;
         this.spriteAnchor.x = PADDING;
         this.spriteAnchor.y = PADDING;
         var left:Number = PADDING + (icon ? this.spriteAnchor.width + PADDING : 0);
         // each line its own field (the first the symbol's), the signs first: their column's width
         var lines:Array = String(_data.name).split("\n");
         var signs:Array = [];
         var column:Number = 0;
         var i:int;
         for(i = 0; i < lines.length; ++i)
         {
            var sign:String = signOf(lines[i]);
            signs.push(sign == null ? null : this.lineField(sign));
            if(sign != null)
            {
               lines[i] = String(lines[i]).substr(2);
               column = Math.max(column,signs[i].textWidth);
            }
         }
         if(column > 0)
         {
            column += SIGN_GAP;
         }
         var right:Number = left;
         var y:Number = PADDING;
         for(i = 0; i < lines.length; ++i)
         {
            var line:TextField = i == 0 ? this.textField : this.lineField(lines[i]);
            if(i == 0)
            {
               this.textField.text = lines[0];
            }
            var at:Number = signs[i] == null ? left : left + column;
            line.x = at - GUTTER;
            line.y = y - GUTTER;
            if(signs[i] != null)
            {
               // centred in its column, on its line's baseline (the same format, the same y)
               signs[i].x = left + (column - SIGN_GAP - signs[i].textWidth) / 2 - GUTTER;
               signs[i].y = line.y;
            }
            right = Math.max(right,at + line.textWidth);
            y += line.textHeight;
         }
         var frameWidth:Number = Math.ceil(right + PADDING);
         var frameHeight:Number = Math.ceil(y + PADDING);
         this.frame.graphics.clear();
         this.frame.graphics.beginFill(EDGE);
         this.frame.graphics.drawRect(0,0,frameWidth,frameHeight);
         this.frame.graphics.beginFill(FILL);
         this.frame.graphics.drawRect(1,1,frameWidth - 2,frameHeight - 2);
         this.frame.graphics.endFill();
      }
   }
}
