package
{
   import flash.display.MovieClip;
   
   public dynamic class mouseSymbol extends MovieClip
   {
      
      public function mouseSymbol()
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

