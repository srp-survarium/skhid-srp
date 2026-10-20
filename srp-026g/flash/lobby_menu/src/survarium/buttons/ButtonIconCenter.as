package survarium.buttons
{
   import flash.display.MovieClip;
   import scaleform.clik.utils.Constraints;
   
   public class ButtonIconCenter extends ButtonClick
   {
      
      public var icon:MovieClip;
      
      public function ButtonIconCenter()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(!constraintsDisabled && constraints)
         {
            constraints.addElement("icon",this.icon,Constraints.CENTER_H);
         }
         tooltipRendererName = "SrvProfileTipRendererStyled";
      }
      
      override public function set data(param1:Object) : void
      {
         if(param1 && param1.enabled != undefined)
         {
            enabled = param1.enabled;
         }
         super.data = param1;
      }
      
      override protected function updateAfterStateChange() : void
      {
         if(!initialized)
         {
            return;
         }
         if(constraints != null && !constraintsDisabled && this.icon != null)
         {
            constraints.updateElement("icon",this.icon);
         }
      }
      
      override protected function updateText() : void
      {
         super.updateText();
         trace("updateText");
         if(this.icon)
         {
            this.icon.gotoAndStop(_data ? _data.icon : 1);
         }
      }
   }
}

