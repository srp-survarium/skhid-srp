package survarium.controls
{
   import flash.text.TextField;
   import survarium.Util;
   
   public class ItemRendererText extends ItemRendererCore
   {
      
      public var textFieldLevel:TextField;
      
      public var textFieldName:TextField;
      
      public function ItemRendererText()
      {
         super();
      }
      
      override protected function updateData() : void
      {
         if(this.textFieldLevel)
         {
            this.textFieldLevel.text = Util.getRomanNumeral(_data.itemLevel);
         }
         this.textFieldName.text = _data.name;
      }
   }
}

