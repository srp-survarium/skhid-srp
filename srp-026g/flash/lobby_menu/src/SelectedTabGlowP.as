package
{
   import flash.display.MovieClip;
   
   public dynamic class SelectedTabGlowP extends MovieClip
   {
      
      public function SelectedTabGlowP()
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

