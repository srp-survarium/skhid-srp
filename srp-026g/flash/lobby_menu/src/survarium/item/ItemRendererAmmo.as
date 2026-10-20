package survarium.item
{
   import flash.text.TextField;
   import survarium.controls.ItemRendererBase;
   
   public class ItemRendererAmmo extends ItemRendererBase
   {
      
      public var textFieldCount:TextField;
      
      public function ItemRendererAmmo()
      {
         super();
      }
      
      override protected function updateData() : void
      {
         super.updateData();
         this.textFieldCount.text = _data.value;
      }
   }
}

