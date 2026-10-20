package
{
   import flash.display.MovieClip;
   
   public dynamic class SpriteHighlight extends MovieClip
   {
      
      public function SpriteHighlight()
      {
         super();
         addFrameScript(39,this.frame40);
      }
      
      internal function frame40() : *
      {
         gotoAndPlay("glow");
      }
   }
}

