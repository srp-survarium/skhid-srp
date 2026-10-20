package
{
   import flash.display.MovieClip;
   
   public dynamic class ArtifactStatusIndicator extends MovieClip
   {
      
      public var mask_:MovieClip;
      
      public function ArtifactStatusIndicator()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function set_progress(value:int) : *
      {
         this.mask_.gotoAndStop(239 - value * 239 / 100);
      }
      
      internal function frame1() : *
      {
         stop();
      }
   }
}

