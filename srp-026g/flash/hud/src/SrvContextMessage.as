package
{
   import flash.display.MovieClip;
   
   public dynamic class SrvContextMessage extends MovieClip
   {
      
      public var mc_text:MovieClip;
      
      public function SrvContextMessage()
      {
         super();
         addFrameScript(14,this.frame15,29,this.frame30);
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

