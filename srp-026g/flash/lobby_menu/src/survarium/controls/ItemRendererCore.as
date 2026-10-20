package survarium.controls
{
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.ToolTip;
   
   public class ItemRendererCore extends ToolTip
   {
      
      public function ItemRendererCore()
      {
         super();
      }
      
      public function set data(param1:Object) : void
      {
         _data = param1;
         invalidateData();
      }
      
      public function get data() : Object
      {
         return _data;
      }
      
      public function updateState(param1:uint = 0) : void
      {
      }
      
      override protected function configUI() : void
      {
         super.configUI();
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            if(_data)
            {
               visible = true;
               this.updateData();
            }
            else
            {
               visible = false;
               this.cleanData();
            }
         }
      }
      
      protected function cleanData() : void
      {
      }
      
      protected function updateData() : void
      {
      }
   }
}

