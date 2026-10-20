package
{
   import flash.display.MovieClip;
   
   public dynamic class RewardIcons extends MovieClip
   {
      
      public function RewardIcons()
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

