// Compile with the native optimized profile.
package survarium.controls
{
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.Slider;
   
   public class SliderTrack extends Slider
   {
      
      public var trackValue:Button;
      
      public function SliderTrack()
      {
         super();
      }
      
      override protected function updateThumb() : void
      {
         super.updateThumb();
         this.trackValue.width = maximum ? (_width - offsetRight - offsetLeft) * ((value - minimum) / (maximum - minimum)) : 0;
      }
   }
}

