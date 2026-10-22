package
{
   import flash.display.MovieClip;
   
   public dynamic class MessageThinBackground extends MovieClip
   {
      
      public function MessageThinBackground()
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

