package survarium.buttons
{
   import scaleform.clik.controls.Button;
   
   public class ButtonNoResize extends Button
   {
      
      public function ButtonNoResize()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
      }
   }
}

