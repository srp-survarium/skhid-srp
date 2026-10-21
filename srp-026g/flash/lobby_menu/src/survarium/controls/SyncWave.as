package survarium.controls
{
   import flash.display.Graphics;
   import flash.display.MovieClip;
   import flash.events.Event;
   
   public class SyncWave extends MovieClip
   {
      
      public var size:uint = 1;
      
      protected var mathTargets:Array = [30,-10,30,35,14,40,-7,27,30];
      
      protected var container:MovieClip = new MovieClip();
      
      protected var container2:MovieClip = new MovieClip();
      
      protected var chift:Number = 0;
      
      protected var chift2:Number = 0;
      
      protected var factor2:Number = 0.3141592653589793;
      
      protected var steps:Number = 0;
      
      protected var current_factor:Number = 0;
      
      public function SyncWave()
      {
         super();
         addChild(this.container2);
         this.container2.addChild(this.container);
         this.container.y = 70;
         this.container.x = 6;
         addEventListener(Event.EXIT_FRAME,this.redraw);
      }
      
      public function redraw(event:Event = null) : void
      {
         chift += this.factor2;
         chift2 = this.chift;
         var g:Graphics = this.container.graphics;
         g.clear();
         g.lineStyle(this.size,6946681,1,false);
         var segments:uint = 40;
         var segmentWidth:uint = int(width / segments);
         var waveStep:Number = Math.PI * 8 / segments;
         g.moveTo(0,0);
         var prevAmp:Number = 1;
         var envelopeStep:Number = Math.PI / segments;
         steps = this.chift * Math.PI;
         var i:uint = 0;
         var targetIndex:int;
         var fromAmp:Number;
         var toAmp:Number;
         var amp:Number;
         var smoothAmp:Number;
         while(i < segments)
         {
            targetIndex = int(this.steps % 80 / 10);
            fromAmp = this.mathTargets[targetIndex];
            toAmp = this.mathTargets[targetIndex + 1];
            amp = this.steps % 10 / 10 * (toAmp - fromAmp) + fromAmp;
            smoothAmp = (amp + prevAmp) / 2;
            prevAmp = amp;
            chift2 += this.factor2 / 16;
            steps += width / 160;
            g.lineTo(i * segmentWidth - 5 * Math.cos(smoothAmp / 8),(Math.sin(i * envelopeStep) + 0.4) * 1 * Math.cos(i * waveStep + this.steps) * smoothAmp * 1);
            i++;
         }
      }
      
      public function clean() : void
      {
         removeEventListener(Event.EXIT_FRAME,this.redraw);
         var g:Graphics = this.container.graphics;
         g.clear();
      }
   }
}

