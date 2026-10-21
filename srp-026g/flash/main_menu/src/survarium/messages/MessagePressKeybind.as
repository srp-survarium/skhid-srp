package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import survarium.constants.UiSound;
   
   public class MessagePressKeybind extends MessageBase
   {
      
      public var textFieldAction:TextField;
      
      public var textField:TextField;
      
      public var Icon:MovieClip;
      
      public function MessagePressKeybind()
      {
         super();
         ExternalInterface.call("sound_play",UiSound.MENU_MESSAGE);
      }
      
      override protected function cleanUp(event:Event = null) : void
      {
         super.cleanUp();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(_data)
         {
            this.textField.text = _data.textLabel;
            this.textFieldAction.text = _data.textActionLabel;
            this.Icon.x = this.textFieldAction.x + this.textFieldAction.getLineMetrics(0).x - this.Icon.width - 7;
         }
      }
   }
}

