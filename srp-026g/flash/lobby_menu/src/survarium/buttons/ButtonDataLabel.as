package survarium.buttons
{
   import scaleform.clik.controls.ListItemRenderer;
   
   public class ButtonDataLabel extends ListItemRenderer
   {
      
      public function ButtonDataLabel()
      {
         super();
      }
      
      override protected function updateText() : void
      {
         trace("BDL::",textField,_data ? _data.name : null,alpha,visible);
         if(textField && _data)
         {
            textField.text = _data.name;
         }
      }
   }
}

