package
{
   import flash.display.MovieClip;
   
   public dynamic class SrvAsyncSymbol extends MovieClip
   {
      
      public function SrvAsyncSymbol()
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

