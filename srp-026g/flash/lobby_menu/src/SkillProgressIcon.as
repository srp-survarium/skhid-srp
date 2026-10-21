package
{
   import flash.display.MovieClip;
   
   public dynamic class SkillProgressIcon extends MovieClip
   {
      
      public function SkillProgressIcon()
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

