package
{
   import flash.display.MovieClip;
   
   public dynamic class ItemRoundCell extends MovieClip
   {
      
      public function ItemRoundCell()
      {
         super();
         addFrameScript(29,this.frame30);
      }
      
      internal function frame30() : *
      {
         gotoAndPlay("blink");
      }
   }
}

