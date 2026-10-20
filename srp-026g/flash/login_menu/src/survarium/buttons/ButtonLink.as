package survarium.buttons
{
   import flash.text.TextFormat;
   
   public class ButtonLink extends ButtonClick
   {
      
      public function ButtonLink()
      {
         super();
      }
      
      override protected function updateText() : void
      {
         var format:TextFormat = textField.getTextFormat();
         format.underline = true;
         textField.defaultTextFormat = format;
         super.updateText();
         textField.setTextFormat(format);
      }
   }
}

