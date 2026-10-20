package fpsHUD_fla
{
   import flash.display.MovieClip;
   import flash.events.Event;
   
   public dynamic class bars_container_130 extends MovieClip
   {
      
      public var barsContainer:MovieClip;
      
      public function bars_container_130()
      {
         super();
         addFrameScript(19,this.frame20);
      }
      
      internal function frame20() : *
      {
         stop();
         dispatchEvent(new Event(Event.COMPLETE));
      }
   }
}

