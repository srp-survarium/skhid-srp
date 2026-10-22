package top_fla
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   
   public dynamic class topRankLabel_60 extends MovieClip
   {
      
      public var textField:TextField;
      
      public function topRankLabel_60()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function set text(value:String) : void
      {
         this.textField.text = value;
      }
      
      internal function frame1() : *
      {
      }
   }
}

