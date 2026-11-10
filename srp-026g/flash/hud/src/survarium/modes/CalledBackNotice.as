package survarium.modes
{
   import flash.display.Sprite;
   import flash.events.TimerEvent;
   import flash.filters.DropShadowFilter;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.Timer;

   // srp's: a watcher's room calls it back (the server's Recall note, exe patch 130): one line
   // under the top of the screen, the running language's words with the seconds left, counted
   // down each second until the server lets the game go. It takes no press and moves nothing.
   public class CalledBackNotice extends Sprite
   {
      // the words' place of the seconds
      public static const SECONDS:String = "%1$s";

      public var label:TextField = new TextField();

      private var _words:String = "";

      private var _left:int = 0;

      private var _timer:Timer = new Timer(1000);

      // The whole format of `source`, as ModeNotice takes it: its font is embedded in one cut only.
      public function CalledBackNotice(source:TextField)
      {
         name = "srpCalledBack";
         mouseEnabled = mouseChildren = false;
         label.mouseEnabled = label.selectable = false;
         var format:TextFormat = source.length > 0 ? source.getTextFormat() : source.defaultTextFormat;
         format.size = 18;
         format.color = 0xFFD23F;
         format.align = "left";
         label.defaultTextFormat = format;
         label.embedFonts = source.embedFonts;
         label.autoSize = TextFieldAutoSize.LEFT;
         label.multiline = label.wordWrap = false;
         label.filters = [new DropShadowFilter(1,90,0,1,3,3,2)];
         label.x = 16;
         label.y = 7;
         addChild(label);
         this._timer.addEventListener(TimerEvent.TIMER,this.onSecond);
         visible = false;
      }

      // The words, with SECONDS where the seconds go (none: the seconds are not said).
      public function set words(value:String) : void
      {
         this._words = value != null ? value : "";
      }

      // Shown for `seconds`, counted down; 0 hides it.
      public function begin(seconds:int, screenWidth:Number) : void
      {
         this._left = seconds;
         this._timer.reset();
         visible = seconds > 0 && this._words != "";
         if(visible)
         {
            this._timer.start();
            this.draw(screenWidth);
         }
      }

      public function reset() : void
      {
         this.begin(0,0);
      }

      private var _screenWidth:Number = 1280;

      private function onSecond(e:TimerEvent) : void
      {
         // at 0 it stays, saying 0: the server lets the game go then
         if(this._left > 0)
         {
            this._left--;
         }
         this.draw(this._screenWidth);
      }

      private function draw(screenWidth:Number) : void
      {
         this._screenWidth = screenWidth;
         label.text = this._words.split(SECONDS).join(String(this._left));
         var w:int = int(label.textWidth + 36);
         var h:int = int(label.textHeight + 18);
         graphics.clear();
         graphics.lineStyle(1,0xE0A64E,0.8);
         graphics.beginFill(0x0C0E10,0.82);
         graphics.drawRoundRect(0,0,w,h,8,8);
         graphics.endFill();
         x = int((screenWidth - w) / 2);
         y = 96;
      }
   }
}
