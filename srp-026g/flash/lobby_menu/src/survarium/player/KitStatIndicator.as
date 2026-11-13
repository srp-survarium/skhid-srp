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

   // A number of the kit (IKitStat: the movement speed, the regeneration) in a box of
   // its own on the weight's row, the same kind of box. Its frame is the weight's
   // (WeightFrame), its word and value a copy of the weight's own symbol, so they have
   // the weight label's and value's fonts, sizes, colours and baseline; its own tooltip
   // with the number's parts covers the whole box. Hidden when the exe gives none.
   public class KitStatIndicator
   {
      // Where the original's graphics put playerWeight on its frame (the frame's
      // shape at 397,851 in the profile, playerWeight at 419.45,856).
      private static const WIDGET_X:Number = 22.45;
      private static const WIDGET_Y:Number = 5;
      // The frame's opaque box in its bitmap (around it, the shadow).
      private static const BOX:Rectangle = new Rectangle(11,0,155,43);
      // The value ends as far from its box's right side as the word starts from its left
      // (the symbol has it 3.4 nearer the right side).
      private static const VALUE_SHIFT:Number = -3.4;
      private var _box:Sprite;
      private var _widget:WeightContainer_44;
      private var _tip:WeightTip;
      private var _weight:MovieClip;
      // The weight's own box in its parent: where `weight`'s frame is opaque.
      public static function boxOf(weight:MovieClip) : Rectangle
      {
         return new Rectangle(weight.x - WIDGET_X + BOX.x,weight.y - WIDGET_Y + BOX.y,BOX.width,BOX.height);
      }
      // The same room each side of a box's word and value (the weight's own and a copy's).
      public static function evenSides(widget:MovieClip) : void
      {
         widget.textFieldValue.x += VALUE_SHIFT;
      }
      // `weight`: the weight's widget, in its place on its frame. `name`: the box's. `left`:
      // where the box starts in the weight's parent, on the weight's row.
      public function KitStatIndicator(weight:MovieClip, name:String, left:Number)
      {
         _weight = weight;
         var frame:Bitmap = new Bitmap(new WeightFrame());
         _box = new Sprite();
         _box.name = name;
         _box.addChild(frame);
         _widget = new WeightContainer_44();
         _widget.x = WIDGET_X;
         _widget.y = WIDGET_Y;
         _widget.progress_indicator.visible = false;
         evenSides(_widget);
         _box.addChild(_widget);
         var hover:Sprite = new Sprite();
         _tip = new WeightTip();
         _tip.graphics.beginFill(0,0.01);
         _tip.graphics.drawRect(BOX.x,BOX.y,BOX.width,BOX.height);
         _tip.graphics.endFill();
         hover.addChild(_tip);
         _box.addChild(hover);
         // On the weight's row, under the same panels; on a whole pixel, as the frame is a bitmap.
         _box.x = Math.round(left - BOX.x);
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
      public function update(stat:IKitStat) : void
      {
         _box.visible = stat != null;
         _tip.updateEffects(stat);
         if(stat == null) return;
         // The weight's own fields' formats as the game draws them (font, size, bold),
         // copied at each update: no style sheet on the box's fields, whose markup
         // the game drew small (the owner's screenshot, 2026-10-30); the value's colour
         // the style sheet's colour of its style.
         copy(_widget.textFieldLabel,_weight.textFieldLabel,stat.label,null);
         var style:Object = MainScene.textStyleSheet != null ? MainScene.textStyleSheet.getStyle(stat.valueStyle) : null;
         copy(_widget.textFieldValue,_weight.textFieldValue,stat.value,style != null ? style.color : null);
      }
   }
}
