package
{
   import flash.display.MovieClip;
   
   public dynamic class LoadingIndicatorFr extends MovieClip
   {
      
      public function LoadingIndicatorFr()
      {
         super();
         addFrameScript(32,this.frame33);
      }
      
      internal function frame33() : *
      {
         stop();
      }
   }
}

