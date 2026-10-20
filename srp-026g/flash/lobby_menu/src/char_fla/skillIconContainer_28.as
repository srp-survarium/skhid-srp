package char_fla
{
   import flash.display.MovieClip;
   
   public dynamic class skillIconContainer_28 extends MovieClip
   {
      
      public var maskSprite:MovieClip;
      
      public function skillIconContainer_28()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
         mask = this.maskSprite;
      }
   }
}

