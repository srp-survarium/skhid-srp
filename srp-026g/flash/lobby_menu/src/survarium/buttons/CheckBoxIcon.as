package survarium.buttons
{
   import flash.display.MovieClip;
   import scaleform.clik.controls.Button;
   import survarium.controls.UnfocusedCheckBox;
   
   public class CheckBoxIcon extends UnfocusedCheckBox
   {
      
      public var icon:MovieClip;
      
      public var lockIcon:Button;
      
      protected var _iconIndex:uint;
      
      public function CheckBoxIcon()
      {
         super();
      }
      
      public function set iconIndex(value:uint) : void
      {
         _iconIndex = value;
         this.icon.gotoAndStop(this._iconIndex + 1);
      }
      
      override protected function updateText() : void
      {
         super.updateText();
         if(textField && this.icon)
         {
            textField.x = this.icon.x + this.icon.width + 5;
         }
         if(this.lockIcon)
         {
            mouseChildren = true;
            mouseEnabled = true;
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(constraints)
         {
            constraints.removeAllElements();
         }
         constraintsDisabled = true;
         preventAutosizing = true;
         trace("CBI::configUI",this.lockIcon);
         if(this.lockIcon)
         {
            this.lockIcon.tooltipRendererName = "SrvProfileTipRenderer";
            this.icon.mouseEnabled = false;
         }
      }
   }
}

