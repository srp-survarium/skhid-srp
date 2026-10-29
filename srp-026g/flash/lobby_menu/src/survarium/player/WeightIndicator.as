package survarium.player
{
   import flash.display.MovieClip;
   import flash.display.Bitmap;
   import flash.display.Sprite;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import survarium.Util;
   import survarium.data.IconContainer;

   // Owns both the number's state and its explanation; a kit update replaces
   // them together. An old exe's two-argument callback has no effect details.
   public class WeightIndicator
   {
      private var _widget:MovieClip;
      private var _tip:WeightTip;
      private var _current:Number;
      private var _maximum:Number;
      private var _effects:WeightEffects;
      // The kit's movement speed, beside the weight on its left: the game's speed icon
      // (modifier atlas 11, as the tooltip's rows) and the value, in the value's font.
      private var _speed:TextField;
      private var _speedIcon:Bitmap;
      public function WeightIndicator(widget:MovieClip)
      {
         _widget = widget;
         var holder:Sprite = new Sprite();
         _tip = new WeightTip();
         _tip.graphics.beginFill(0,0.01);
         _tip.graphics.drawRect(0,0,widget.width,widget.height);
         _tip.graphics.endFill();
         holder.addChild(_tip);
         widget.addChild(holder);
         _speed = new TextField();
         _speed.selectable = false;
         _speed.mouseEnabled = false;
         _speed.embedFonts = widget.textFieldValue.embedFonts;
         _speed.defaultTextFormat = widget.textFieldValue.getTextFormat();
         _speed.autoSize = TextFieldAutoSize.RIGHT;
         _speed.multiline = false;
         _speed.wordWrap = false;
         // Under the tooltip's hover area, which stays the widget's last child.
         widget.addChildAt(_speed,widget.getChildIndex(holder));
         _speedIcon = new Bitmap();
         _speedIcon.smoothing = true;
         widget.addChildAt(_speedIcon,widget.getChildIndex(holder));
         _tip.validateNow();
      }
      public function update(current:Number, maximum:Number, effects:WeightEffects) : void
      {
         _current = current;
         _maximum = maximum;
         _effects = effects;
         refresh();
      }
      public function refresh() : void
      {
         var style:String = _current >= _maximum ? "red" :
            _effects != null && _effects.slowsMovement ? WeightEffects.WARNING_STYLE : "highlight";
         _widget.textFieldValue.styleSheet = MainScene.textStyleSheet;
         _widget.textFieldValue.text = Util.styledChar(_current.toFixed(2),style) + " / " + Util.getNumberString(_maximum);
         _speed.styleSheet = MainScene.textStyleSheet;
         _speed.text = _effects != null ? _effects.speedText : "";
         _speed.x = -_speed.width - 10;
         _speed.y = _widget.textFieldValue.y;
         var icons:IconContainer = IconContainer.getInstance("modifiers");
         _speedIcon.bitmapData = _speed.text != "" && icons != null && icons.getIcon(11).bitmap != null ? icons.getIcon(11).bitmap.bitmapData : null;
         if(_speedIcon.bitmapData != null)
         {
            // One line's height, whatever the weight's own field does.
            var line:Number = _speed.getLineMetrics(0).height;
            _speedIcon.height = _speedIcon.width = Math.round(line + 2);
            _speedIcon.x = _speed.x - _speedIcon.width;
            _speedIcon.y = _speed.y + (line + 4 - _speedIcon.height) / 2;
         }
         _widget.progress_indicator.maximum = _maximum;
         _widget.progress_indicator.value = _current;
         _tip.updateEffects(_effects);
      }
   }
}
