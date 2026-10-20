package
{
   import flash.display.MovieClip;
   import flash.events.Event;
   
   public dynamic class LoadingWheelsMess extends MovieClip
   {
      
      public var roll1:MovieClip;
      
      public var roll2:MovieClip;
      
      public var roll2Shadow:MovieClip;
      
      public var roll1Shadow:MovieClip;
      
      public function LoadingWheelsMess()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function onRenderEnterFrame(param1:Event) : void
      {
         var _loc2_:int = 8;
         this.roll1.rotation += _loc2_;
         this.roll2.rotation -= _loc2_;
         this.roll1Shadow.rotation += _loc2_;
         this.roll1Shadow.rotation += _loc2_;
      }
      
      public function playAnimation(param1:Boolean) : void
      {
         if(param1 && !hasEventListener(Event.ENTER_FRAME))
         {
            addEventListener(Event.ENTER_FRAME,this.onRenderEnterFrame);
         }
         else if(!param1 && hasEventListener(Event.ENTER_FRAME))
         {
            removeEventListener(Event.ENTER_FRAME,this.onRenderEnterFrame);
         }
      }
      
      internal function frame1() : *
      {
         this.playAnimation(true);
      }
   }
}

