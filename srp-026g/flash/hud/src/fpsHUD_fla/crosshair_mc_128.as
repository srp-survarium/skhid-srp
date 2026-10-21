package fpsHUD_fla
{
   import flash.display.MovieClip;
   
   public dynamic class crosshair_mc_128 extends MovieClip
   {
      
      public function crosshair_mc_128()
      {
         super();
         addFrameScript(499,this.frame500);
      }
      
      internal function frame500() : *
      {
         stop();
      }
   }
}

