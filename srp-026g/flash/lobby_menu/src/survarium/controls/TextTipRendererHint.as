package survarium.controls
{
   import scaleform.clik.utils.ConstrainedElement;
   
   public class TextTipRendererHint extends TextTipRenderer
   {
      
      public function TextTipRendererHint()
      {
         super();
      }
      
      override public function set data(param1:Object) : void
      {
         _data = param1;
         this.invalidateData();
      }
      
      override protected function invalidateData() : void
      {
         textField.wordWrap = false;
         textField.multiline = false;
         textField.text = _data.tipText;
         width = textField.textWidth;
         height = textField.textHeight;
         var _loc1_:ConstrainedElement = constraints.getElement("textField");
         var _loc2_:Number = Math.ceil(textField.textWidth + _loc1_.left + _loc1_.right + 5);
         var _loc3_:Number;
         _loc3_ = Math.ceil(textField.textHeight + _loc1_.top + _loc1_.bottom + 5);
         width = _loc2_;
         height = _loc3_;
         constraints.update(width,height);
      }
   }
}

