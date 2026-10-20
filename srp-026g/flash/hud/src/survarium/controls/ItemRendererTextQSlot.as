package survarium.controls
{
   import flash.text.TextField;
   
   public class ItemRendererTextQSlot extends ItemRendererCore
   {
      
      public var textFieldCount:TextField;
      
      public var textFieldName:TextField;
      
      public function ItemRendererTextQSlot()
      {
         super();
      }
      
      override protected function updateData() : void
      {
         if(this.textFieldCount)
         {
            this.textFieldCount.text = "x" + String(_data.value);
         }
         this.textFieldName.text = _data.name;
      }
   }
}

