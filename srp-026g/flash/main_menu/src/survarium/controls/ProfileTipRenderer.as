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

      public function ProfileTipRenderer()
      {
         super();
         this._icon = new Icon();
         this.spriteAnchor.addChild(this._icon);
         removeChildAt(0);
         this.frame = new Shape();
         addChildAt(this.frame,0);
         scale9Grid = null;
         this.textField.multiline = true;
         this.textField.wordWrap = false;
         this.textField.autoSize = TextFieldAutoSize.LEFT;
      }

      override protected function invalidateData() : void
      {
         this.textField.text = _data.name;
         var lines:TextFormat = new TextFormat();
         lines.align = TextFormatAlign.LEFT;
         this.textField.setTextFormat(lines);
         var icon:Boolean = _data.icon is Icon;
         if(icon)
         {
            this._icon.applyImage(_data.icon);
         }
         this.spriteAnchor.visible = icon;
         this.spriteAnchor.x = PADDING;
         this.spriteAnchor.y = PADDING;
         var left:Number = PADDING + (icon ? this.spriteAnchor.width + PADDING : 0);
         this.textField.x = left - GUTTER;
         this.textField.y = PADDING - GUTTER;
         var frameWidth:Number = Math.ceil(left + this.textField.textWidth + PADDING);
         var frameHeight:Number = Math.ceil(this.textField.textHeight + 2 * PADDING);
         this.frame.graphics.clear();
         this.frame.graphics.beginFill(EDGE);
         this.frame.graphics.drawRect(0,0,frameWidth,frameHeight);
         this.frame.graphics.beginFill(FILL);
         this.frame.graphics.drawRect(1,1,frameWidth - 2,frameHeight - 2);
         this.frame.graphics.endFill();
      }
   }
}
