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
      
      override public function set data(param1:Object) : void
      {
         super.data = param1;
         preventAutosizing = true;
         invalidateData();
      }
      
      override public function setData(param1:Object) : void
      {
         this.data = param1;
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

