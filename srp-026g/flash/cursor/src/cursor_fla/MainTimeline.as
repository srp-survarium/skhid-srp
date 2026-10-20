package cursor_fla
{
   import flash.display.MovieClip;
   import flash.events.MouseEvent;
   
   public dynamic class MainTimeline extends MovieClip
   {
      
      public var mouse_mc:mouseSymbol;
      
      public function MainTimeline()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function onMouseMove(param1:MouseEvent) : void
      {
         this.mouse_mc.x = mouseX;
         this.mouse_mc.y = mouseY;
      }
      
      public function setCursor(param1:uint) : void
      {
         this.mouse_mc.gotoAndStop(param1);
      }
      
      internal function frame1() : *
      {
         this.mouse_mc.x = mouseX;
         this.mouse_mc.y = mouseY;
         stage.addEventListener(MouseEvent.MOUSE_MOVE,this.onMouseMove);
      }
   }
}

