package
{
   import flash.display.MovieClip;
   
   public dynamic class ArtifactStatusIndicatorLine extends MovieClip
   {
      
      public var mask_:MovieClip;
      
      public function ArtifactStatusIndicatorLine()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function set_progress(value:int) : *
      {
         this.mask_.width = 111 * (value / 100);
      }
      
      internal function frame1() : *
      {
         stop();
      }
   }
}

