package survarium.controls
{
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import survarium.buttons.ButtonClick;
   
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
      
      override protected function handleMouseRollOver(param1:MouseEvent) : void
      {
         super.handleMouseRollOver(param1);
         if(enabled)
         {
            ExternalInterface.call("sound_play",1);
         }
      }
      
      override protected function handleMouseRelease(param1:MouseEvent) : void
      {
         super.handleMouseRelease(param1);
         if(enabled)
         {
            ExternalInterface.call("sound_play",0);
         }
      }
      
      override public function get autoRepeat() : Boolean
      {
         return false;
      }
      
      override public function set autoRepeat(param1:Boolean) : void
      {
      }
      
      override public function get toggle() : Boolean
      {
         return true;
      }
      
      override public function set toggle(param1:Boolean) : void
      {
      }
      
      override public function toString() : String
      {
         return "[CLIK CheckBox " + name + "]";
      }
   }
}

