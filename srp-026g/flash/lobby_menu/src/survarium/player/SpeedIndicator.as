package survarium.player
{
   import flash.display.Bitmap;
   import flash.display.DisplayObjectContainer;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import panel_fla.WeightContainer_44;
   import survarium.data.LocalizationData;

   // The kit's movement speed: a box of its own left of the weight's, the same
   // kind of box. Its frame is the weight's (WeightFrame), its word and value a
   // second copy of the weight's own symbol, so they have the weight label's and
   // value's fonts, sizes, colours and baseline; its own tooltip with the speed's
   // parts covers the whole box. Hidden when the exe gives no speed.
   public class SpeedIndicator
   {
      // Where the original's graphics put playerWeight on its frame (the frame's
      // shape at 397,851 in the profile, playerWeight at 419.45,856).
      private static const WIDGET_X:Number = 22.45;
      private static const WIDGET_Y:Number = 5;
      // The frame's opaque box in its bitmap (around it, the shadow).
      private static const BOX:Rectangle = new Rectangle(11,0,155,43);
      private var _box:Sprite;
      private var _widget:WeightContainer_44;
      private var _tip:WeightTip;
      private var _weight:MovieClip;
      // `weight`: the weight's widget, in its place on its frame.
      public function SpeedIndicator(weight:MovieClip)
      {
         _weight = weight;
         var frame:Bitmap = new Bitmap(new WeightFrame());
         _box = new Sprite();
         _box.name = "speed";
         _box.addChild(frame);
         _widget = new WeightContainer_44();
         _widget.x = WIDGET_X;
         _widget.y = WIDGET_Y;
         _widget.progress_indicator.visible = false;
         _box.addChild(_widget);
         var hover:Sprite = new Sprite();
         _tip = new WeightTip();
         _tip.graphics.beginFill(0,0.01);
         _tip.graphics.drawRect(BOX.x,BOX.y,BOX.width,BOX.height);
         _tip.graphics.endFill();
         hover.addChild(_tip);
         _box.addChild(hover);
         // Its frame beside the weight's, on the weight's row, under the same panels.
         _box.x = weight.x - WIDGET_X - frame.width;
         _box.y = weight.y - WIDGET_Y;
         var panel:DisplayObjectContainer = weight.parent;
         panel.addChildAt(_box,panel.getChildIndex(weight));
         _tip.validateNow();
      }
      private static function copy(field:TextField, like:TextField, text:String, colour:String) : void
      {
         field.styleSheet = null;
         field.embedFonts = like.embedFonts;
         var format:TextFormat = like.length > 0 ? like.getTextFormat(0,like.length) : like.defaultTextFormat;
         if(colour != null) format.color = uint("0x" + colour.replace("#",""));
         field.defaultTextFormat = format;
         field.text = text;
         field.setTextFormat(format);
      }
      public function update(speed:SpeedExplanation) : void
      {
         _box.visible = speed != null;
         _tip.updateEffects(speed);
         if(speed == null) return;
         // The weight's own fields' formats as the game draws them (font, size, bold),
         // copied at each update: no style sheet on the speed's fields, whose markup
         // the game drew small (the owner's screenshot, 2026-10-30); the value's colour
         // the style sheet's colour of its style.
         copy(_widget.textFieldLabel,_weight.textFieldLabel,LocalizationData.getName("st_srp_speed_label"),null);
         var style:Object = MainScene.textStyleSheet != null ? MainScene.textStyleSheet.getStyle(speed.valueStyle) : null;
         copy(_widget.textFieldValue,_weight.textFieldValue,speed.value,style != null ? style.color : null);
      }
   }
}
