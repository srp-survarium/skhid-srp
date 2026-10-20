package survarium.buttons
{
   import flash.display.MovieClip;
   
   public class ButtonStyledLabel extends ButtonClick
   {
      
      protected static const ENABLED_STATE:uint = 0;
      
      protected static const INVALID_STATE:uint = 1;
      
      protected static const DISABLED_STATE:uint = 2;
      
      public var invalidOverlay:MovieClip;
      
      public function ButtonStyledLabel()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         trace("ButtonStyledLabel::configUI");
         super.configUI();
         if(this.invalidOverlay)
         {
            this.invalidOverlay.mouseEnabled = false;
         }
         trace("ButtonStyledLabel::configUIFinished");
      }
      
      override public function set data(value:Object) : void
      {
         super.data = value;
         label = _data.label;
         if(this.invalidOverlay)
         {
            this.invalidOverlay.visible = _data.state == INVALID_STATE;
         }
         trace("ButtonStyledLabel",_data.enabled,_data.state);
         if(_data.enabled != undefined)
         {
            enabled = _data.enabled;
         }
         else
         {
            enabled = _data.state != DISABLED_STATE;
         }
      }
      
      override protected function updateText() : void
      {
         if(textField)
         {
            textField.styleSheet = MainScene.textStyleSheet;
            textField.text = _label;
            textField.alpha = enabled ? 1 : 0.5;
         }
      }
   }
}

