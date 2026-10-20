package
{
   import flash.display.MovieClip;
   
   public dynamic class SrvWarningMessage extends MovieClip
   {
      
      public var mc_text:MovieClip;
      
      public function SrvWarningMessage()
      {
         super();
         addFrameScript(8,this.frame9,16,this.frame17);
      }
      
      internal function frame9() : *
      {
         stop();
      }
      
      internal function frame17() : *
      {
         if(parent)
         {
            parent.removeChild(this);
         }
         visible = false;
      }
   }
}

