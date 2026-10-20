package survarium.buttons
{
   import scaleform.gfx.TextFieldEx;
   
   public class ButtonShort extends ButtonClick
   {
      
      public function ButtonShort()
      {
         super();
      }
      
      override protected function updateText() : void
      {
         trace("shortUpdateText",_label);
         if(textField)
         {
            TextFieldEx.setVerticalAlign(textField,TextFieldEx.VALIGN_CENTER);
            textField.multiline = true;
            textField.defaultTextFormat = textField.getTextFormat();
         }
         super.updateText();
      }
   }
}

