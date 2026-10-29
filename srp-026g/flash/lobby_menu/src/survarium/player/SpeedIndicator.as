package survarium.player
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import survarium.data.LocalizationData;

   // The kit's movement speed, a second indicator left of the weight: its own
   // word ("Speed") and value in the weight's label and value styles, and its own
   // tooltip with the speed's parts. Hidden when the exe gives no speed.
   public class SpeedIndicator
   {
      // The gap that separates it from the weight, and its label from its value.
      private static const GAP:Number = 22;
      private static const SPACE:Number = 6;
      private var _widget:MovieClip;
      private var _label:TextField;
      private var _value:TextField;
      private var _hover:Sprite;
      private var _tip:WeightTip;
      public function SpeedIndicator(widget:MovieClip)
      {
         _widget = widget;
         _label = field(widget.textFieldLabel,"speedLabel");
         _value = field(widget.textFieldValue,"speedValue");
         _hover = new Sprite();
         _hover.name = "speedTip";
         _tip = new WeightTip();
         _hover.addChild(_tip);
         widget.addChild(_label);
         widget.addChild(_value);
         widget.addChild(_hover);
         _tip.validateNow();
      }
      private static function field(like:TextField, name:String) : TextField
      {
         var f:TextField = new TextField();
         f.name = name;
         f.selectable = false;
         f.mouseEnabled = false;
         f.embedFonts = like.embedFonts;
         f.defaultTextFormat = like.getTextFormat();
         f.multiline = false;
         f.wordWrap = false;
         f.autoSize = TextFieldAutoSize.RIGHT;
         return f;
      }
      public function update(speed:SpeedExplanation) : void
      {
         var shown:Boolean = speed != null;
         _label.visible = _value.visible = _hover.visible = shown;
         _tip.updateEffects(speed);
         if(!shown) return;
         _label.text = LocalizationData.getName("st_srp_speed_label");
         _value.styleSheet = MainScene.textStyleSheet;
         _value.text = speed.value;
         _value.x = -GAP - _value.width;
         _value.y = _widget.textFieldValue.y;
         _label.x = _value.x - SPACE - _label.width;
         _label.y = _widget.textFieldLabel.y;
         _tip.graphics.clear();
         _tip.graphics.beginFill(0,0.01);
         _tip.graphics.drawRect(_label.x,0,-GAP - _label.x,_widget.height);
         _tip.graphics.endFill();
      }
   }
}
