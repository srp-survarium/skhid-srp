package survarium.controls
{
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.controls.ListItemRenderer;
   
   public class TaskRenderer extends ListItemRenderer
   {
      
      public function TaskRenderer()
      {
         super();
      }
      
      override public function set data(value:Object) : void
      {
         super.data = value;
         preventAutosizing = true;
         invalidateData();
      }
      
      override public function setData(newData:Object) : void
      {
         this.data = newData;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         constraints.removeAllElements();
         constraintsDisabled = true;
         preventAutosizing = true;
      }
      
      override protected function updateText() : void
      {
         if(_data)
         {
            textField.height = 22;
            textField.autoSize = TextFieldAutoSize.LEFT;
            textField.text = _data.text;
         }
         else
         {
            textField.text = "";
         }
      }
   }
}

