package
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   
   public dynamic class SrvContextInfo extends MovieClip
   {
      
      public var progress_indicator:ArtifactStatusIndicatorLine;
      
      public var textPoints:TextField;
      
      public var contextText:TextField;
      
      public function SrvContextInfo()
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

