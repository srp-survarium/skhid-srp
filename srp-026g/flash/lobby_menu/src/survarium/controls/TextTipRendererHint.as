package survarium.controls
{
   import scaleform.clik.utils.ConstrainedElement;
   
   public class TextTipRendererHint extends TextTipRenderer
   {
      
      public function TextTipRendererHint()
      {
         super();
      }
      
      override public function set data(value:Object) : void
      {
         _data = value;
         this.invalidateData();
      }
      
      override protected function invalidateData() : void
      {
         textField.wordWrap = false;
         textField.multiline = false;
         textField.text = _data.tipText;
         width = textField.textWidth;
         height = textField.textHeight;
         var element:ConstrainedElement = constraints.getElement("textField");
         var newWidth:Number = Math.ceil(textField.textWidth + element.left + element.right + 5);
         var newHeight:Number;
         newHeight = Math.ceil(textField.textHeight + element.top + element.bottom + 5);
         width = newWidth;
         height = newHeight;
         constraints.update(width,height);
      }
   }
}

