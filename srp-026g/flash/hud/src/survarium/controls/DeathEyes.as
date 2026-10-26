package survarium.controls
{
   import flash.display.GradientType;
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.geom.Matrix;
   import flash.utils.getTimer;

   // The eyes closing on the death screen (docs/notes/death-eyes.md): over the respawn countdown a
   // vignette narrows, the screen darkens and two soft lids close, slowly at first, with two slow
   // blinks near the end, shut when the countdown ends; open at once when it is gone (the respawn,
   // a rollback over the death, the hud's reset). The countdown is the hud's own: the exe's
   // root.set_respawn_time (0x236C30) with "<prefix> MM:SS" once a second, the seconds being the
   // respawn rule's time left rounded up (0x316C20), and "" at 0. On the stage below the hud's
   // root, in screen pixels, as the screen effects: the kits and the countdown stay over it and
   // take clicks, and hiding the interface (F11, Blackout) leaves it.
   public class DeathEyes extends Sprite
   {
      // Each blink: its middle and half width (shares of the countdown) and how far it closes the
      // eyes that are left open.
      public static const BLINKS:Array = [[0.62,0.05,0.45],[0.86,0.05,0.6]];

      // The countdown's ticks come once a second; the closing waits for one that is late (a world
      // that has stopped, the end of a match) instead of running on alone.
      public static const TICK_WAIT:int = 1100;

      // The lids' soft edge, and how much lower than their middle their corners are, as shares of
      // the screen's height; the edge's steps.
      protected static const FEATHER:Number = 0.1;

      protected static const CORNER:Number = 0.12;

      protected static const STEPS:int = 16;

      protected var _shade:Shape = new Shape();

      protected var _top:Shape = new Shape();

      protected var _bottom:Shape = new Shape();

      protected var _width:Number = 1280;

      protected var _height:Number = 720;

      // The countdown's last seconds (0: none), when it began, when it ends, when it last changed.
      protected var _seconds:int = 0;

      protected var _start:Number = 0;

      protected var _end:Number = 0;

      protected var _changed:Number = 0;

      protected var _progress:Number = 0;

      protected var _openness:Number = 1;

      public function DeathEyes()
      {
         super();
         mouseEnabled = false;
         mouseChildren = false;
         visible = false;
         addChild(this._shade);
         addChild(this._top);
         addChild(this._bottom);
         this.drawLids();
      }

      // How open the eyes are (1 open, 0 shut) at a share of the countdown.
      public static function opennessAt(p:Number) : Number
      {
         var open:Number;
         var blink:Array;
         var d:Number;
         if(p <= 0)
         {
            return 1;
         }
         if(p >= 1)
         {
            return 0;
         }
         open = 1 - Math.pow(p,1.8);
         for each(blink in BLINKS)
         {
            d = (p - blink[0]) / blink[1];
            if(d > -1 && d < 1)
            {
               open *= 1 - blink[2] * 0.5 * (1 + Math.cos(Math.PI * d));
            }
         }
         return open;
      }

      // The seconds of the countdown's text: the "MM:SS" it ends with, 0 for none. (By hand: no
      // other movie of the game uses RegExp.)
      public static function secondsOf(text:String) : int
      {
         var colon:int = text == null ? -1 : text.lastIndexOf(":");
         var from:int = colon;
         var to:int = colon + 1;
         if(colon < 0)
         {
            return 0;
         }
         while(from > 0 && isDigit(text.charAt(from - 1)))
         {
            from--;
         }
         while(to < text.length && isDigit(text.charAt(to)))
         {
            to++;
         }
         if(from == colon || to == colon + 1 || to != text.length)
         {
            return 0;
         }
         return int(text.substring(from,colon)) * 60 + int(text.substring(colon + 1,to));
      }

      protected static function isDigit(c:String) : Boolean
      {
         return c >= "0" && c <= "9";
      }

      public function get openness() : Number
      {
         return this._openness;
      }

      public function get progress() : Number
      {
         return this._progress;
      }

      // The screen's size in the stage's pixels.
      public function place(width:Number, height:Number) : void
      {
         if(width > 0 && height > 0 && (width != this._width || height != this._height))
         {
            this._width = width;
            this._height = height;
            this.drawLids();
            this.update();
         }
      }

      // The countdown's text, as the exe sets it; now: the movie's clock (getTimer).
      public function countdown(text:String, now:Number = NaN) : void
      {
         var seconds:int = secondsOf(text);
         if(isNaN(now))
         {
            now = getTimer();
         }
         if(seconds == 0)
         {
            this._seconds = 0;
            removeEventListener(Event.ENTER_FRAME,this.onFrame);
            this.update();
            return;
         }
         if(this._seconds == 0)
         {
            this._start = now;
            addEventListener(Event.ENTER_FRAME,this.onFrame);
         }
         if(seconds != this._seconds)
         {
            // a tick: the time left is the new seconds (rounded up, they change as it reaches them)
            this._end = Math.max(now + seconds * 1000,this._start + 1);
            this._changed = now;
            this._seconds = seconds;
         }
         this.update(now);
      }

      protected function onFrame(e:Event) : void
      {
         this.update();
      }

      public function update(now:Number = NaN) : void
      {
         if(isNaN(now))
         {
            now = getTimer();
         }
         if(this._seconds == 0)
         {
            this._progress = 0;
            this._openness = 1;
            visible = false;
            return;
         }
         now = Math.min(now,this._changed + TICK_WAIT);
         this._progress = Math.max(0,Math.min(1,(now - this._start) / (this._end - this._start)));
         this._openness = opennessAt(this._progress);
         this.draw();
         visible = true;
      }

      // The vignette narrowing and the darkening, the lids where the openness puts them.
      protected function draw() : void
      {
         var p:Number = this._progress;
         var closing:Number = 1 - this._openness;
         var dark:Number = 0.6 * Math.pow(p,1.5);
         var edge:Number = Math.min(1,dark + 0.85 * Math.sqrt(p));
         var core:Number = 255 * Math.max(0.05,0.55 - 0.45 * p);
         var box:Matrix = new Matrix();
         var g:Graphics = this._shade.graphics;
         var travel:Number = this._height * (0.5 + FEATHER + CORNER);
         box.createGradientBox(this._width * 1.3,this._height * 1.6,0,-this._width * 0.15,-this._height * 0.3);
         g.clear();
         g.beginGradientFill(GradientType.RADIAL,[0,0,0],[dark,dark,edge],[0,core,255],box);
         g.drawRect(0,0,this._width,this._height);
         g.endFill();
         this._top.y = this._height / 2 - this._openness * travel;
         this._bottom.y = this._height / 2 + this._openness * travel;
         // shut: nothing of the lids' soft edges shows between them
         this._top.visible = this._bottom.visible = closing > 0.001;
      }

      // A lid: black above its edge (y 0 in the middle, lower to the corners), then a soft edge of
      // steps, drawn widest first, whose alphas add up to a smooth fall to nothing; the bottom lid
      // is the top one turned over.
      protected function drawLids() : void
      {
         var lid:Shape;
         var g:Graphics;
         var i:int;
         var want:Number;
         var below:Number;
         var alpha:Number;
         var reach:Number;
         for each(lid in [this._top,this._bottom])
         {
            g = lid.graphics;
            g.clear();
            for(i = STEPS; i >= 1; i--)
            {
               // what the edge should be at step i (smooth from 1 to 0), and at the step past it
               want = smooth(i - 0.5);
               below = i == STEPS ? 0 : smooth(i + 0.5);
               alpha = 1 - (1 - want) / (1 - below);
               reach = this._height * FEATHER * i / STEPS;
               this.lidShape(g,reach,alpha);
            }
            this.lidShape(g,0,1);
            lid.scaleY = lid == this._bottom ? -1 : 1;
         }
      }

      protected static function smooth(step:Number) : Number
      {
         var t:Number = Math.max(0,Math.min(1,step / STEPS));
         return Math.min(0.999,1 - t * t * (3 - 2 * t));
      }

      // The lid down to its edge moved by reach: a curve from corner to corner through the middle.
      protected function lidShape(g:Graphics, reach:Number, alpha:Number) : void
      {
         var corner:Number = this._height * CORNER;
         var over:Number = this._width * 0.05;
         g.beginFill(0,alpha);
         g.moveTo(-over,-this._height * 1.5);
         g.lineTo(this._width + over,-this._height * 1.5);
         g.lineTo(this._width + over,corner + reach);
         // a quadratic whose middle is at reach: its control point as far above it as the corners below
         g.curveTo(this._width / 2,reach - corner,-over,corner + reach);
         g.lineTo(-over,-this._height * 1.5);
         g.endFill();
      }
   }
}
