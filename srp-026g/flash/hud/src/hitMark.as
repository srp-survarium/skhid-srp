package
{
   import flash.display.MovieClip;
   
   public dynamic class hitMark extends MovieClip
   {
      
      public function hitMark()
      {
         super();
         addFrameScript(9,this.frame10);
      }
      
      internal function frame10() : *
      {
         stop();
         this.parent.removeChild(this);
      }
   }
}

