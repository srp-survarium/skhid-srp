package survarium.controls
{
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextFieldAutoSize;
   import survarium.buttons.ButtonClick;
   import survarium.constants.UiSound;
   
   public class UnfocusedCheckBox extends ButtonClick
   {
      
      // fitLabel's: every state's label as long as its text.
      private var _labelFits:Boolean = false;

      public function UnfocusedCheckBox()
      {
         super();
      }

      // The label as long as its text, on one line, in every state: the symbol's fixed width cut
      // longer ones ("Automatikus vásá…"). Each state's frame may hold a text field of its own, so it
      // is sized again at every state change. For a box whose row has room to its right (Autobuy, the
      // shop's "Hide owned").
      public function fitLabel() : void
      {
         _labelFits = true;
         this.sizeLabel();
      }

      override protected function updateAfterStateChange() : void
      {
         super.updateAfterStateChange();
         this.sizeLabel();
      }

      private function sizeLabel() : void
      {
         if(this._labelFits && textField != null)
         {
            textField.wordWrap = false;
            textField.autoSize = TextFieldAutoSize.LEFT;
         }
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

