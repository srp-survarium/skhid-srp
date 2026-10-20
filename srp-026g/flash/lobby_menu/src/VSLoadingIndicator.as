package
{
   import flash.display.MovieClip;
   
   public dynamic class VSLoadingIndicator extends MovieClip
   {
      
      public function VSLoadingIndicator()
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

