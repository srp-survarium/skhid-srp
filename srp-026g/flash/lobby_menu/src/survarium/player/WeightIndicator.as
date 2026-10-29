package survarium.player
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import survarium.Util;

   // Owns both the number's state and its explanation; a kit update replaces
   // them together. An old exe's two-argument callback has no effect details.
   public class WeightIndicator
   {
      private var _widget:MovieClip;
      private var _tip:WeightTip;
      private var _current:Number;
      private var _maximum:Number;
      private var _effects:WeightEffects;
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
         _widget.progress_indicator.maximum = _maximum;
         _widget.progress_indicator.value = _current;
         _tip.updateEffects(_effects);
      }
   }
}
