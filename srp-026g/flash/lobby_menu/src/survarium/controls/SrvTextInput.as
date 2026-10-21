package survarium.controls
{
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.events.TextEvent;
   import flash.external.ExternalInterface;
   import scaleform.clik.controls.TextInput;
   import flash.ui.Keyboard;
   import survarium.constants.UiSound;
   
   public class SrvTextInput extends TextInput
   {
      
      public static const RESET_TEXT:String = "resetText";
      
      private var _last_input:int = 0;
      
      public function SrvTextInput()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         textField.addEventListener(Event.CHANGE,this.play_input_sound,false,0,true);
         textField.addEventListener(TextEvent.TEXT_INPUT,this.play_input_sound,false,0,true);
         textField.addEventListener(KeyboardEvent.KEY_DOWN,this.play_input_sound,false,0,true);
      }
      
      override protected function updateText() : void
      {
         if(_focused && _usingDefaultTextFormat)
         {
            textField.defaultTextFormat = _textFormat;
            _usingDefaultTextFormat = false;
            if(_displayAsPassword && !textField.displayAsPassword)
            {
               textField.displayAsPassword = true;
            }
         }
         if(_text != "")
         {
            if(_isHtml)
            {
               textField.htmlText = _text;
            }
            else
            {
               textField.text = _text;
            }
         }
         else
         {
            textField.text = "";
            if(!_focused && _defaultText != "")
            {
               if(_displayAsPassword)
               {
                  textField.displayAsPassword = false;
               }
               textField.text = _defaultText;
               dispatchEvent(new Event(RESET_TEXT));
               _usingDefaultTextFormat = true;
               if(defaultTextFormat != null)
               {
                  textField.setTextFormat(defaultTextFormat);
               }
            }
         }
      }
      
      private function play_input_sound(event:Event) : void
      {
         var keyEvent:KeyboardEvent;
         if(event.type == TextEvent.TEXT_INPUT)
         {
            _last_input = textField.caretIndex;
         }
         else if(event.type == Event.CHANGE)
         {
            if(this._last_input != textField.caretIndex)
            {
               this.playSound();
            }
            _last_input = textField.caretIndex;
         }
         else if(event.type == KeyboardEvent.KEY_DOWN)
         {
            keyEvent = event as KeyboardEvent;
            if(keyEvent.keyCode == Keyboard.DELETE && textField.caretIndex <= textField.length - 1)
            {
               ExternalInterface.call("sound_play",UiSound.BACKSPACE);
               _last_input = textField.caretIndex;
            }
            else if(keyEvent.keyCode == Keyboard.BACKSPACE && textField.caretIndex >= 1)
            {
               ExternalInterface.call("sound_play",UiSound.BACKSPACE);
               _last_input = textField.caretIndex - 1;
            }
            else if((keyEvent.keyCode == Keyboard.LEFT || keyEvent.keyCode == Keyboard.HOME) && textField.caretIndex >= 1)
            {
               this.playSound();
            }
            else if((keyEvent.keyCode == Keyboard.RIGHT || keyEvent.keyCode == Keyboard.END) && textField.caretIndex <= textField.length - 1)
            {
               this.playSound();
            }
         }
         else
         {
            this.playSound();
         }
      }
      
      private function playSound() : void
      {
         ExternalInterface.call("sound_play",UiSound.TYPE);
      }
   }
}

