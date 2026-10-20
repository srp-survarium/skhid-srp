package
{
   import flash.display.MovieClip;
   
   public dynamic class LoadingIndicator extends MovieClip
   {
      
      public function LoadingIndicator()
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

