package survarium.controls
{
   import flash.text.StyleSheet;
   import flash.text.TextField;
   import scaleform.clik.controls.DropdownMenu;
   import scaleform.clik.utils.Constraints;
   
   public class DropdownMenuStyled extends DropdownMenu
   {
      
      public var textFieldLabel:TextField;
      
      protected var styleSheet:StyleSheet;
      
      public function DropdownMenuStyled()
      {
         var styleObject:Object;
         super();
         styleSheet = new StyleSheet();
         styleObject = new Object();
         styleObject.color = "#F4B717";
         styleObject.size = 14;
         this.styleSheet.setStyle("yellow",styleObject);
         styleObject = new Object();
         styleObject.color = "#A97E0E";
         styleObject.size = 14;
         styleObject.alpha = 0.5;
         this.styleSheet.setStyle("dark_yellow",styleObject);
      }
      
      override public function set label(value:String) : void
      {
         super.label = value;
         if(this.textFieldLabel)
         {
            this.textFieldLabel.text = value;
         }
      }
      
      override protected function configUI() : void
      {
         if(!constraintsDisabled && this.textFieldLabel)
         {
            constraints.addElement("textFieldLabel",this.textFieldLabel,Constraints.LEFT | Constraints.RIGHT);
         }
         if(!constraintsDisabled && textField)
         {
            constraints.addElement("textField",textField,Constraints.LEFT | Constraints.RIGHT);
         }
         super.configUI();
      }
      
      override protected function updateAfterStateChange() : void
      {
         super.updateAfterStateChange();
         if(constraints != null && !constraintsDisabled && this.textFieldLabel != null)
         {
            constraints.updateElement("textFieldLabel",this.textFieldLabel);
         }
      }
      
      override protected function updateText() : void
      {
         textField.styleSheet = this.styleSheet;
         super.updateText();
         alpha = enabled ? 1 : 0.7;
      }
   }
}

