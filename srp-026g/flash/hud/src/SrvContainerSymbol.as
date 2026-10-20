package
{
   import flash.display.MovieClip;
   
   public dynamic class SrvContainerSymbol extends MovieClip
   {
      
      public function SrvContainerSymbol()
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

