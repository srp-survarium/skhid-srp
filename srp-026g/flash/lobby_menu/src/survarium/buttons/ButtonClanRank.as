package survarium.buttons
{
   import flash.events.Event;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.constants.Events;
   
   public class ButtonClanRank extends ButtonDataLabel
   {
      
      public var btnChangeName:Button;
      
      public function ButtonClanRank()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         mouseChildren = true;
         textField.mouseEnabled = false;
         this.btnChangeName.addEventListener(ButtonEvent.CLICK,this.onChangeName);
      }
      
      private function onChangeName(param1:ButtonEvent) : void
      {
         dispatchEvent(new Event(Events.RENAME));
      }
      
      override protected function updateText() : void
      {
         super.updateText();
         this.btnChangeName.visible = false;
      }
   }
}

