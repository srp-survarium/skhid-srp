package survarium.controls
{
   import flash.text.StyleSheet;
   import scaleform.clik.utils.Constraints;
   
   public class ListItemRendererStyled extends ListItemNoFocusable
   {
      
      protected var styleSheet:StyleSheet;
      
      public function ListItemRendererStyled()
      {
         var _loc1_:Object;
         super();
         styleSheet = new StyleSheet();
         _loc1_ = new Object();
         _loc1_.color = "#FFFFFF";
         _loc1_.fontSize = 11.5;
         this.styleSheet.setStyle("white",_loc1_);
         _loc1_ = new Object();
         _loc1_.color = "#F4B717";
         _loc1_.fontSize = 11.5;
         this.styleSheet.setStyle("yellow",_loc1_);
         _loc1_ = new Object();
         _loc1_.color = "#A97E0E";
         _loc1_.fontSize = 11.5;
         this.styleSheet.setStyle("dark_yellow",_loc1_);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(!constraintsDisabled && textField)
         {
            constraints.addElement("textField",textField,Constraints.LEFT | Constraints.RIGHT);
         }
         displayFocus = false;
      }
      
      override protected function updateText() : void
      {
         textField.styleSheet = this.styleSheet;
         super.updateText();
      }
   }
}

