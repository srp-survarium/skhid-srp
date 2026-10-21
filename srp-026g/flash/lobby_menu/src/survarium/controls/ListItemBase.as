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
      
      override public function setData(newData:Object) : void
      {
         this.data = newData;
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

