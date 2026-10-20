package panel_fla
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   
   public dynamic class timerLabel_25 extends MovieClip
   {
      
      public var icon:MovieClip;
      
      public var textField:TextField;
      
      public function timerLabel_25()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function setTime(param1:String) : void
      {
         this.textField.text = param1;
         this.icon.x = this.textField.getLineMetrics(0).x - this.icon.width - 5;
      }
      
      internal function frame1() : *
      {
      }
   }
}

