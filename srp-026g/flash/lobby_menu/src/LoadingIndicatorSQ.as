package
{
   import flash.display.MovieClip;
   
   public dynamic class LoadingIndicatorSQ extends MovieClip
   {
      
      public function LoadingIndicatorSQ()
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

