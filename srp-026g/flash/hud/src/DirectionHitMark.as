package
{
   import flash.display.MovieClip;
   
   public dynamic class DirectionHitMark extends MovieClip
   {
      
      public function DirectionHitMark()
      {
         super();
         addFrameScript(48,this.frame49);
      }
      
      internal function frame49() : *
      {
         stop();
         this.parent.removeChild(this);
      }
   }
}

