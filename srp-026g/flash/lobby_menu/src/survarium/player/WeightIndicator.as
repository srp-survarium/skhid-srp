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
      // The kit's regeneration, then its movement speed: each its own indicator left of the
      // weight, the three boxes the same distance apart.
      private var _speed:KitStatIndicator;
      private var _regeneration:KitStatIndicator;
      private var _regenerationStat:RegenerationExplanation;
      // `left`: where the row's first box starts in the widget's parent.
      public function WeightIndicator(widget:MovieClip, left:Number)
      {
         _widget = widget;
         KitStatIndicator.evenSides(widget);
         _regeneration = new KitStatIndicator(widget,"regeneration",left);
         _speed = new KitStatIndicator(widget,"speed",(left + KitStatIndicator.boxOf(widget).x) / 2);
         var holder:Sprite = new Sprite();
         _tip = new WeightTip();
         _tip.graphics.beginFill(0,0.01);
         _tip.graphics.drawRect(0,0,widget.width,widget.height);
         _tip.graphics.endFill();
         holder.addChild(_tip);
         widget.addChild(holder);
         _tip.validateNow();
      }
      // `regeneration`: null until the exe gives the kit's modifiers.
      public function update(current:Number, maximum:Number, effects:WeightEffects, regeneration:RegenerationExplanation = null) : void
      {
         _current = current;
         _maximum = maximum;
         _effects = effects;
         _regenerationStat = regeneration;
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
         _speed.update(_effects != null ? _effects.speedExplanation : null);
         _regeneration.update(_regenerationStat);
      }
   }
}
