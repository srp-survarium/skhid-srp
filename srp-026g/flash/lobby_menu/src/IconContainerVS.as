package
{
   import flash.display.MovieClip;
   
   public dynamic class IconContainerVS extends MovieClip
   {
      
      public function IconContainerVS()
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

