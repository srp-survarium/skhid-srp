package fpsHUD_fla
{
   import flash.display.MovieClip;
   
   public dynamic class progress_oxygen_188 extends MovieClip
   {
      
      public var background:MovieClip;
      
      public var glow_thumb:MovieClip;
      
      public var progress_thumb:MovieClip;
      
      public var _progress:Number;
      
      public function progress_oxygen_188()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function set_progress(value:Number) : void
      {
         if(this._progress == value)
         {
            return;
         }
         if(value < this._progress)
         {
            this.glow_thumb.gotoAndPlay(2);
         }
         _progress = value;
         this.progress_thumb.width = this.background.width * value;
         this.glow_thumb.x = this.progress_thumb.width;
         this.glow_thumb.gotoAndPlay(2);
      }
      
      internal function frame1() : *
      {
         this.glow_thumb.gotoAndStop(1);
         _progress = -1;
      }
   }
}

