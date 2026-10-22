package survarium.buttons
{
   public class ButtonBlink extends ButtonClick
   {
      
      public function ButtonBlink()
      {
         super();
         _stateMap = {
            up:["up"],
            over:["over"],
            down:["down"],
            release:["over"],
            out:["up"],
            disabled:["disabled"],
            selecting:["over"],
            toggle:["up"],
            kb_selecting:["up"],
            kb_release:["up"],
            kb_down:["down"],
            blink:["blink"],
            blink_down:["blink_down"]
         };
      }
      
      public function set blink(value:Boolean) : void
      {
         trace("blink",value);
         setState(value ? "blink" : "up");
      }
   }
}

