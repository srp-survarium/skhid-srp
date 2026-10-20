package survarium.controls
{
   import scaleform.clik.controls.ListItemRenderer;
   
   public class ListItemNoFocusable extends ListItemRenderer
   {
      
      public function ListItemNoFocusable()
      {
         super();
      }
      
      public function ListItemBase() : *
      {
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         displayFocus = false;
      }
   }
}

