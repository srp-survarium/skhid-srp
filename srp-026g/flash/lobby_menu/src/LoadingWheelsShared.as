package
{
   import flash.display.MovieClip;
   import flash.events.Event;
   
   public dynamic class LoadingWheelsShared extends MovieClip
   {
      
      public var roll1:MovieClip;
      
      public var roll2:MovieClip;
      
      public var roll2Shadow:MovieClip;
      
      public var roll1Shadow:MovieClip;
      
      public function LoadingWheelsShared()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function onRenderEnterFrame(event:Event) : void
      {
         var step:int = 8;
         this.roll1.rotation += step;
         this.roll2.rotation -= step;
         this.roll1Shadow.rotation += step;
         this.roll1Shadow.rotation += step;
      }
      
      public function playAnimation(play:Boolean) : void
      {
         if(play && !hasEventListener(Event.ENTER_FRAME))
         {
            addEventListener(Event.ENTER_FRAME,this.onRenderEnterFrame);
         }
         else if(!play && hasEventListener(Event.ENTER_FRAME))
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

