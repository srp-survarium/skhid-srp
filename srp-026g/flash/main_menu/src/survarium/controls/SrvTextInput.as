package survarium.controls
{
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.events.TextEvent;
   import flash.external.ExternalInterface;
   import scaleform.clik.controls.TextInput;

   config namespace KEY;
   KEY const BACKSPACE:int = 8;
   KEY const END:int = 35;
   KEY const HOME:int = 36;
   KEY const LEFT:int = 37;
   KEY const RIGHT:int = 39;
   KEY const DELETE:int = 46;
   config namespace SOUND;
   SOUND const TYPE:int = 4;
   SOUND const BACKSPACE:int = 8;
   
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
            if(keyEvent.keyCode == KEY::DELETE && textField.caretIndex <= textField.length - 1)
            {
               ExternalInterface.call("sound_play",SOUND::BACKSPACE);
               _last_input = textField.caretIndex;
            }
            else if(keyEvent.keyCode == KEY::BACKSPACE && textField.caretIndex >= 1)
            {
               ExternalInterface.call("sound_play",SOUND::BACKSPACE);
               _last_input = textField.caretIndex - 1;
            }
            else if((keyEvent.keyCode == KEY::LEFT || keyEvent.keyCode == KEY::HOME) && textField.caretIndex >= 1)
            {
               this.playSound();
            }
            else if((keyEvent.keyCode == KEY::RIGHT || keyEvent.keyCode == KEY::END) && textField.caretIndex <= textField.length - 1)
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
         ExternalInterface.call("sound_play",SOUND::TYPE);
      }
   }
}

