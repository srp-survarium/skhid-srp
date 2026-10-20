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
         var _loc1_:TextFormat = textField.getTextFormat();
         _loc1_.underline = true;
         textField.defaultTextFormat = _loc1_;
         super.updateText();
         textField.setTextFormat(_loc1_);
      }
   }
}

