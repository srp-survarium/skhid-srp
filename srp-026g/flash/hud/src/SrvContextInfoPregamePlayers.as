package
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   
   public dynamic class SrvContextInfoPregamePlayers extends MovieClip
   {
      
      public var background:MovieClip;
      
      public var textField:TextField;
      
      public function SrvContextInfoPregamePlayers()
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

