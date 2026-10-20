package survarium.buttons
{
   import flash.text.StyleSheet;
   import scaleform.gfx.TextFieldEx;
   import survarium.controls.UnfocusedCheckBox;
   
   public class CheckBoxMultiline extends UnfocusedCheckBox
   {
      
      private var style:StyleSheet = new StyleSheet();
      
      public function CheckBoxMultiline()
      {
         super();
      }
      
      override protected function updateText() : void
      {
         if(textField)
         {
            textField.multiline = true;
            textField.styleSheet = this.style;
            TextFieldEx.setVerticalAlign(textField,TextFieldEx.VALIGN_CENTER);
         }
         super.updateText();
      }
      
      override protected function configUI() : void
      {
         var styleObject:Object;
         super.configUI();
         preventAutosizing = false;
         styleObject = new Object();
         styleObject.color = "#5b6261";
         this.style.setStyle("gray",styleObject);
         styleObject = new Object();
         styleObject.color = "#BCCBCB";
         this.style.setStyle("white",styleObject);
      }
   }
}

