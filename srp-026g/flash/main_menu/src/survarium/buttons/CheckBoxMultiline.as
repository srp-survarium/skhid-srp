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
         var _loc1_:Object;
         super.configUI();
         preventAutosizing = false;
         _loc1_ = new Object();
         _loc1_.color = "#5b6261";
         this.style.setStyle("gray",_loc1_);
         _loc1_ = new Object();
         _loc1_.color = "#BCCBCB";
         this.style.setStyle("white",_loc1_);
      }
   }
}

