package
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   
   public dynamic class SrvContextInfoPregame extends MovieClip
   {
      
      public var premiumMembers:SrvContextInfoPregamePlayers;
      
      public var contextText:TextField;
      
      public function SrvContextInfoPregame()
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

