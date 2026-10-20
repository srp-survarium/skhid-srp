package survarium.controls
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   
   public class NumericStepperLabel extends NumericStepperSrv
   {
      
      public var icon:MovieClip;
      
      public var textFieldLabel:TextField;
      
      protected var _label:String;
      
      public function NumericStepperLabel()
      {
         super();
      }
      
      public function set label(param1:String) : void
      {
         if(this._label == param1)
         {
            return;
         }
         _label = param1;
         invalidateData();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
      }
      
      override protected function updateLabel() : void
      {
         super.updateLabel();
         trace("numChildren",numChildren);
         this.textFieldLabel.text = this._label;
         this.textFieldLabel.x = this.icon.x + (this.icon.currentFrame == 1 ? 17 : 22);
      }
   }
}

