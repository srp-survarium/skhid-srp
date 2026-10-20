package survarium.buttons
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.utils.Constraints;
   
   public class ButtonProgress extends ButtonClick
   {
      
      public var textFieldValue:TextField;
      
      public var progress_track:MovieClip;
      
      protected var _progressMax:int = 0;
      
      protected var _progressValue:Number = 0;
      
      protected var _progressLabel:String = "";
      
      public function ButtonProgress()
      {
         super();
         if(this.progress_track)
         {
            _progressMax = this.progress_track.width;
         }
      }
      
      public function set progress(param1:Number) : void
      {
         if(this._progressValue != param1)
         {
            _progressValue = param1;
            invalidateData();
         }
      }
      
      public function set progressLabel(param1:String) : void
      {
         if(this._progressLabel != param1)
         {
            _progressLabel = param1;
            invalidateData();
         }
      }
      
      public function get progressLabel() : String
      {
         return this._progressLabel;
      }
      
      override protected function updateText() : void
      {
         trace("Progress::updateText");
         super.updateText();
         trace("Progress::updateText1");
         if(this.textFieldValue)
         {
            this.textFieldValue.defaultTextFormat = this.textFieldValue.getTextFormat();
            this.textFieldValue.text = this._progressLabel.toString();
         }
         if(this.progress_track)
         {
            this.progress_track.gotoAndStop((1 - this._progressValue) * 100);
         }
         alpha = enabled ? 1 : 0.4;
         trace("Progress::updateText2");
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(!constraints)
         {
            constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         }
         constraintsDisabled = true;
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.SIZE))
         {
            if(constraints)
            {
               constraints.update(_width,_height);
            }
         }
      }
   }
}

