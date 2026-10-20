package
{
   import flash.display.MovieClip;
   
   public dynamic class ammoType extends MovieClip
   {
      
      public function ammoType()
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

