package survarium.buttons
{
   import scaleform.clik.controls.Button;
   import survarium.constants.Colors;
   
   public class PlayerViewButton extends Button
   {
      
      public var btnFirstView:Button;
      
      public var btnThirdView:Button;
      
      public function PlayerViewButton()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         mouseChildren = true;
      }
      
      override protected function updateText() : void
      {
         if(_data)
         {
            textField.textColor = _data.team == 2 ? Colors.ENEMY : Colors.OTHER_PLAYER;
            textField.text = _data.name;
         }
      }
   }
}

