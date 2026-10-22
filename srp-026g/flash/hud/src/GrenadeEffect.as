package
{
   import flash.display.MovieClip;
   
   public dynamic class GrenadeEffect extends MovieClip
   {
      
      public var mud_layer:MovieClip;
      
      public var flare_layer:MovieClip;
      
      public function GrenadeEffect()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function update(value:Number) : void
      {
         value = Math.min(value,1);
         this.flare_layer.alpha = value;
         this.mud_layer.alpha = value;
      }
      
      internal function frame1() : *
      {
      }
   }
}

