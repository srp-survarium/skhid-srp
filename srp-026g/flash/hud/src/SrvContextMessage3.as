package
{
   import flash.display.MovieClip;
   
   public dynamic class SrvContextMessage3 extends MovieClip
   {
      
      public var mc_text:MovieClip;
      
      public var fontSize:*;
      
      public function SrvContextMessage3()
      {
         super();
         addFrameScript(0,this.frame1,14,this.frame15,29,this.frame30);
      }
      
      internal function frame1() : *
      {
      }
      
      internal function frame15() : *
      {
         stop();
      }
      
      internal function frame30() : *
      {
         if(parent)
         {
            parent.removeChild(this);
         }
         visible = false;
      }
   }
}

