package survarium.controls
{
   import scaleform.clik.controls.Label;
   import scaleform.clik.utils.ConstrainedElement;
   
   public class SrvLabel extends Label
   {
      
      public function SrvLabel()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         textField.defaultTextFormat = textField.getTextFormat();
      }
      
      protected function calculateHeight() : Number
      {
         if(constraints == null || textField == null)
         {
            return actualHeight;
         }
         if(!constraintsDisabled)
         {
            element = constraints.getElement("textField");
         }
         var _loc1_:Number = Math.ceil(textField.textHeight + element.top + element.bottom + 5);
         return _loc1_;
         var element:ConstrainedElement;
      }
      
      override protected function alignForAutoSize() : void
      {
         super.alignForAutoSize();
         _height = this.calculateHeight();
      }
   }
}

