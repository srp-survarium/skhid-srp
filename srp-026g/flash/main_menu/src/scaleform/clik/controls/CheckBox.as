package scaleform.clik.controls
{
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   
   public class CheckBox extends Button
   {
      
      public function CheckBox()
      {
         super();
      }
      
      override protected function initialize() : void
      {
         super.initialize();
         _toggle = true;
      }
      
      override protected function handleMouseRollOver(event:MouseEvent) : void
      {
         super.handleMouseRollOver(event);
         ExternalInterface.call("sound_play",1);
      }
      
      override protected function handleMouseRelease(event:MouseEvent) : void
      {
         super.handleMouseRelease(event);
         ExternalInterface.call("sound_play",0);
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

