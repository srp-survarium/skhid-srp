package survarium.controls
{
   import flash.display.MovieClip;
   
   public class SyncClip extends MovieClip
   {
      
      public var layer0:MovieClip;
      
      public var layer1:MovieClip;
      
      public function SyncClip()
      {
         super();
         this.layer1.size = 5;
         this.layer0.redraw();
         this.layer1.redraw();
         trace("redraw");
      }
      
      public function clean() : void
      {
         this.layer0.clean();
         this.layer1.clean();
      }
   }
}

