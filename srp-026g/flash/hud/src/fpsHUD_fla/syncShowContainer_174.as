package fpsHUD_fla
{
   import flash.display.MovieClip;
   
   public dynamic class syncShowContainer_174 extends MovieClip
   {
      
      public var arrowsContainer:resyncContainer;
      
      public function syncShowContainer_174()
      {
         super();
         addFrameScript(57,this.frame58);
      }
      
      internal function frame58() : *
      {
         gotoAndStop(1);
         this.arrowsContainer.gotoAndStop(1);
      }
   }
}

