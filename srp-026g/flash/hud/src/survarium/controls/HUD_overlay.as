package survarium.controls
{
   import flash.display.MovieClip;
   
   public class HUD_overlay extends MovieClip
   {
      
      public var top_c:MovieClip;
      
      public var top_cr:MovieClip;
      
      public var bottom_cr:MovieClip;
      
      public function HUD_overlay()
      {
         super();
      }
      
      public function update(scale:Number) : void
      {
         trace("stage_layout_resize",scale);
         this.top_c.x = stage.stageWidth / scale / 2;
         this.top_cr.x = stage.stageWidth / scale - this.top_cr.width;
         this.bottom_cr.y = stage.stageHeight / scale - this.bottom_cr.height;
      }
   }
}

