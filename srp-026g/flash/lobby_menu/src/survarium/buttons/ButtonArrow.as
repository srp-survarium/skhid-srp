package survarium.buttons
{
   import flash.display.MovieClip;
   
   public class ButtonArrow extends ButtonClick
   {
      
      public var arrowIcon:MovieClip;
      
      public function ButtonArrow()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
      }
      
      override protected function updateText() : void
      {
         super.updateText();
         if(_data)
         {
            textField.text = _data.name;
            this.arrowIcon.visible = _data.options != undefined;
         }
         trace("updateText::",autoSize,width,_data ? _data.name : "null",textField.text);
      }
   }
}

