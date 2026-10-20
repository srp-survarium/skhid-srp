package
{
   import flash.display.MovieClip;
   
   public dynamic class LoadingIndicatorShared extends MovieClip
   {
      
      public function LoadingIndicatorShared()
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

