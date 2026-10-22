package survarium.controls
{
   public class ItemRendererBase extends ItemRendererCore
   {
      
      public var iconRenderer:IconRenderer;
      
      public function ItemRendererBase()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.iconRenderer.center = true;
      }
      
      override protected function cleanData() : void
      {
         this.iconRenderer.content = null;
      }
      
      override protected function updateData() : void
      {
         this.iconRenderer.content = _data.icon;
      }
   }
}

