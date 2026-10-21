package vs_fla
{
   import flash.display.MovieClip;
   
   public dynamic class columnIcon_41 extends MovieClip
   {
      
      public var icon1:MovieClip;
      
      public var icon2:MovieClip;
      
      public var icon3:MovieClip;
      
      public var icon4:MovieClip;
      
      public function columnIcon_41()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
         this.icon1.gotoAndStop(1);
         this.icon2.gotoAndStop(2);
         this.icon3.gotoAndStop(3);
         this.icon4.gotoAndStop(4);
      }
   }
}

