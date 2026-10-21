package survarium.controls
{
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import survarium.buttons.ButtonClick;
   import survarium.constants.UiSound;
   
   public class UnfocusedCheckBox extends ButtonClick
   {
      
      public function UnfocusedCheckBox()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
      }
      
      override protected function initialize() : void
      {
         super.initialize();
         _toggle = true;
      }
      
      override protected function handleMouseRollOver(event:MouseEvent) : void
      {
         super.handleMouseRollOver(event);
         if(enabled)
         {
            ExternalInterface.call("sound_play",UiSound.HOVER);
         }
      }
      
      override protected function handleMouseRelease(event:MouseEvent) : void
      {
         super.handleMouseRelease(event);
         if(enabled)
         {
            ExternalInterface.call("sound_play",UiSound.CLICK);
         }
      }
      
      override public function get autoRepeat() : Boolean
      {
         return false;
      }
      
      override public function set autoRepeat(value:Boolean) : void
      {
      }
      
      override public function get toggle() : Boolean
      {
         return true;
      }
      
      override public function set toggle(value:Boolean) : void
      {
      }
      
      override public function toString() : String
      {
         return "[CLIK CheckBox " + name + "]";
      }
   }
}

