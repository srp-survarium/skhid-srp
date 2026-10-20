package
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   
   public dynamic class HitText extends MovieClip
   {
      
      public var container2:MovieClip;
      
      public const friction:Number = 0.98;
      
      public const gravity:Number = 0.8;
      
      public const dist:Number = 13;
      
      public const to_rad:Number = 0.017453292519943295;
      
      public var speedX:Number;
      
      public var speedY:Number;
      
      public function HitText()
      {
         super();
         addFrameScript(0,this.frame1,21,this.frame22);
      }
      
      public function step(e:Event) : void
      {
         speedX *= this.friction;
         speedY *= this.friction;
         speedY += this.gravity;
         x += this.speedX;
         y += this.speedY;
         if(this.speedY > 0)
         {
            this.container2.alpha -= 0.075;
            if(this.container2.alpha <= 0)
            {
               removeEventListener(Event.ENTER_FRAME,this.step);
               parent.removeChild(this);
            }
         }
      }
      
      public function startMove(dir:int, value:Number, killed:Boolean) : void
      {
         speedX = -this.dist * Math.cos(dir * this.to_rad);
         speedY = -this.dist * Math.sin(dir * this.to_rad);
         this.container2.textField.text = value.toFixed(1) + "%";
         if(killed)
         {
            this.container2.textField.textColor = 16724787;
         }
         addEventListener(Event.ENTER_FRAME,this.step);
      }
      
      internal function frame1() : *
      {
         stop();
      }
      
      internal function frame22() : *
      {
         stop();
         visible = false;
         if(parent)
         {
            parent.removeChild(this);
         }
      }
   }
}

