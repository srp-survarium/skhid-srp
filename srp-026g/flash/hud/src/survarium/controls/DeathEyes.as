package survarium.controls
{
   import flash.display.GradientType;
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.geom.Matrix;
   import flash.utils.getTimer;

   // The view fading on the death screen (docs/notes/death-eyes.md): over the respawn countdown
   // the view slowly loses brightness and colour (a dark grey wash) under a soft vignette that
   // narrows, with three faint dims like a slowing heartbeat; at the countdown's end it is at its
   // darkest, the scene still faintly visible, and it is clear again at once when the countdown is
   // gone (the respawn, a rollback over the death, the hud's reset). The countdown is the hud's
   // own: the exe's root.set_respawn_time (0x236C30) with "<prefix> MM:SS" once a second, the
   // seconds being the respawn rule's time left rounded up (0x316C20), and "" at 0. On the stage
   // below the hud's root, in screen pixels, over the screen effects: the kits and the countdown
   // stay over it and take clicks, and hiding the interface (F11, Blackout) leaves it.
   public class DeathEyes extends Sprite
   {
      // Each dim: its middle and half width (shares of the countdown) and how dark it goes.
      public static const DIMS:Array = [[0.22,0.03,0.1],[0.48,0.035,0.1],[0.74,0.04,0.12]];

      // The countdown's ticks come once a second; the fading waits for one that is late (a world
      // that has stopped, the end of a match) instead of running on alone.
      public static const TICK_WAIT:int = 1100;

      // The wash at the countdown's end, and its colour.
      protected static const WASH:Number = 0.6;

      protected static const WASH_COLOUR:uint = 0x161616;

      protected var _shade:Shape = new Shape();

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
      }

      // How dark the dims make the view at a share of the countdown.
      public static function dimAt(p:Number) : Number
      {
         var dim:Array;
         var d:Number;
         var sum:Number = 0;
         for each(dim in DIMS)
         {
            d = (p - dim[0]) / dim[1];
            if(d > -1 && d < 1)
            {
               sum += dim[2] * 0.5 * (1 + Math.cos(Math.PI * d));
            }
         }
         return sum;
      }

      // The wash at a share of the countdown.
      public static function washAt(p:Number) : Number
      {
         return WASH * Math.pow(Math.max(0,Math.min(1,p)),1.2);
      }

      // How much of the view shows in the middle of the screen (1 all, before the death) at a share
      // of the countdown: under the wash and the dims, the vignette's clear middle.
      public static function opennessAt(p:Number) : Number
      {
         p = Math.max(0,Math.min(1,p));
         return (1 - washAt(p)) * (1 - dimAt(p));
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
         }
         // every frame until the next tick is late (update stops it then); again at each tick
         addEventListener(Event.ENTER_FRAME,this.onFrame);
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
         if(now >= this._changed + TICK_WAIT)
         {
            // the countdown is late: this picture until its next tick, drawn once
            now = this._changed + TICK_WAIT;
            removeEventListener(Event.ENTER_FRAME,this.onFrame);
         }
         this._progress = Math.max(0,Math.min(1,(now - this._start) / (this._end - this._start)));
         this._openness = opennessAt(this._progress);
         this.draw();
         visible = true;
      }

      // The wash, then the vignette (clear in its middle but for the dims, dark from its rim out),
      // narrowing as the countdown runs, slightly darker toward the top.
      protected function draw() : void
      {
         var p:Number = this._progress;
         var dim:Number = Math.min(1,dimAt(p));
         var core:Number = 255 * (0.58 - 0.33 * p);
         var rim:Number = Math.max(dim,Math.min(1,0.85 * Math.sqrt(p)));
         var box:Matrix = new Matrix();
         var g:Graphics = this._shade.graphics;
         g.clear();
         g.beginFill(WASH_COLOUR,washAt(p));
         g.drawRect(0,0,this._width,this._height);
         g.endFill();
         box.createGradientBox(this._width * 1.3,this._height * 1.6,0,-this._width * 0.15,-this._height * 0.3);
         g.beginGradientFill(GradientType.RADIAL,[0,0,0],[dim,dim,rim],[0,core,Math.min(255,core + 107)],box);
         g.drawRect(0,0,this._width,this._height);
         g.endFill();
         box.createGradientBox(this._width,this._height * 0.6,Math.PI / 2,0,0);
         g.beginGradientFill(GradientType.LINEAR,[0,0],[0.2 * p,0],[0,255],box);
         g.drawRect(0,0,this._width,this._height * 0.6);
         g.endFill();
      }
   }
}
