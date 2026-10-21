package
{
   import flash.display.MovieClip;
   
   public dynamic class BloodEffect extends MovieClip
   {
      
      public var layer_1:MovieClip;
      
      public var layer_2:MovieClip;
      
      public var layer_3:MovieClip;
      
      public var layer_4:MovieClip;
      
      public function BloodEffect()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function update(value:Number) : void
      {
         this.layer_1.alpha = Math.min(0.25,Math.max(value - 0.75,0)) / 0.25;
         this.layer_2.alpha = Math.min(0.3,Math.max(value - 0.5,0)) / 0.3;
         this.layer_3.alpha = Math.min(0.3,Math.max(value - 0.25,0)) / 0.3;
         this.layer_4.alpha = Math.min(0.3,Math.max(value,0)) / 0.3;
      }
      
      internal function frame1() : *
      {
      }
   }
}

