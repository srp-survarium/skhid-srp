package survarium.controls
{
   import flash.text.StyleSheet;
   
   public class ListItemRendererStyled extends ListItemNoFocusable
   {
      
      protected var styleSheet:StyleSheet;
      
      public function ListItemRendererStyled()
      {
         var style:Object;
         super();
         styleSheet = new StyleSheet();
         style = new Object();
         style.color = "#FFFFFF";
         style.fontSize = 11.5;
         this.styleSheet.setStyle("white",style);
         style = new Object();
         style.color = "#F4B717";
         style.fontSize = 11.5;
         this.styleSheet.setStyle("yellow",style);
         style = new Object();
         style.color = "#A97E0E";
         style.fontSize = 11.5;
         this.styleSheet.setStyle("dark_yellow",style);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         displayFocus = false;
      }
      
      override protected function updateText() : void
      {
         textField.styleSheet = this.styleSheet;
         super.updateText();
      }
   }
}

