package fpsHUD_fla
{
   import flash.display.MovieClip;
   
   public dynamic class radiation_120 extends MovieClip
   {
      
      public var infection_indicator:MovieClip;
      
      public function radiation_120()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
         stop();
      }
   }
}

