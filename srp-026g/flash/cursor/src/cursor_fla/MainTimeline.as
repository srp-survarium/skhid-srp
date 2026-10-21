package cursor_fla
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   
   public dynamic class MainTimeline extends MovieClip
   {
      
      public var mouse_mc:mouseSymbol;
      
      public function MainTimeline()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function onMouseMove(event:MouseEvent) : void
      {
         this.mouse_mc.x = mouseX;
         this.mouse_mc.y = mouseY;
      }
      
      public function setCursor(frame:uint) : void
      {
         this.mouse_mc.gotoAndStop(frame);
      }
      
      // The cursor's pictures were drawn 1:1 (about 23 x 30 px, half their 1080p
      // size on a 4K screen); the lobby and the HUD scale themselves from 1920 x
      // 1080 (by the width up to an aspect of 1.85, by the height above). The
      // cursor takes their scale, never below 1: at 1080p and below it is as it
      // was. The symbol's origin is the hot spot (it is put at the mouse), and a
      // scale keeps the origin in place.
      public function onStageResize(event:Event = null) : void
      {
         var w:Number = stage.stageWidth;
         var h:Number = stage.stageHeight;
         var factor:Number = w / h < 1.85 ? w / 1920 : h / 1080;
         if(!(factor > 1))
         {
            factor = 1;
         }
         this.mouse_mc.scaleX = this.mouse_mc.scaleY = factor;
      }
      
      internal function frame1() : *
      {
         this.mouse_mc.x = mouseX;
         this.mouse_mc.y = mouseY;
         stage.addEventListener(MouseEvent.MOUSE_MOVE,this.onMouseMove);
         stage.addEventListener(Event.RESIZE,this.onStageResize);
         this.onStageResize();
      }
   }
}
