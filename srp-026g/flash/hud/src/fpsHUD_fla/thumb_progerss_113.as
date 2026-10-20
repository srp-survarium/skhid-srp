package fpsHUD_fla
{
   import flash.display.MovieClip;
   
   public dynamic class thumb_progerss_113 extends MovieClip
   {
      
      public function thumb_progerss_113()
      {
         super();
         addFrameScript(0,this.frame1,39,this.frame40);
      }
      
      internal function frame1() : *
      {
         stop();
      }
      
      internal function frame40() : *
      {
         gotoAndStop(1);
      }
   }
}

