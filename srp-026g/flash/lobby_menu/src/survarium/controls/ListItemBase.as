package survarium.controls
{
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.ListItemRenderer;
   
   public class ListItemBase extends ListItemRenderer
   {
      
      public function ListItemBase()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
      }
      
      override public function setData(param1:Object) : void
      {
         this.data = param1;
         updateText();
      }
      
      protected function updateSlot() : void
      {
      }
      
      override protected function draw() : void
      {
         super.draw();
         mouseChildren = true;
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateSlot();
         }
      }
   }
}

