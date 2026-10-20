package
{
   import flash.display.MovieClip;
   
   public dynamic class hitMarkContainer extends MovieClip
   {
      
      public var smbBloodMark:bloodMark;
      
      public function hitMarkContainer()
      {
         super();
         addFrameScript(0,this.frame1,119,this.frame120);
      }
      
      internal function frame1() : *
      {
         this.smbBloodMark.gotoAndStop(Math.floor(Math.random() * 3));
      }
      
      internal function frame120() : *
      {
         stop();
         this.parent.removeChild(this);
      }
   }
}

