package
{
   import flash.display.MovieClip;
   
   public dynamic class queueSize extends MovieClip
   {
      
      public function queueSize()
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

