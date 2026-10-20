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
         var _loc1_:Object;
         super();
         styleSheet = new StyleSheet();
         _loc1_ = new Object();
         _loc1_.color = "#F4B717";
         _loc1_.size = 14;
         this.styleSheet.setStyle("yellow",_loc1_);
         _loc1_ = new Object();
         _loc1_.color = "#A97E0E";
         _loc1_.size = 14;
         _loc1_.alpha = 0.5;
         this.styleSheet.setStyle("dark_yellow",_loc1_);
      }
      
      override public function set label(param1:String) : void
      {
         super.label = param1;
         if(this.textFieldLabel)
         {
            this.textFieldLabel.text = param1;
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

