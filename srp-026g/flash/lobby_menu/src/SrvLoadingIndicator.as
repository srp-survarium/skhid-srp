package
{
   import flash.display.MovieClip;
   
   public dynamic class SrvLoadingIndicator extends MovieClip
   {
      
      public function SrvLoadingIndicator()
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

