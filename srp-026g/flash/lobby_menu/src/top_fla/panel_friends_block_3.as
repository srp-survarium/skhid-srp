package top_fla
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   
   public dynamic class panel_friends_block_3 extends MovieClip
   {
      
      public var tx_online:TextField;
      
      public function panel_friends_block_3()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
         this.tx_online.autoSize = TextFieldAutoSize.LEFT;
         this.tx_online.defaultTextFormat = this.tx_online.getTextFormat();
      }
   }
}

