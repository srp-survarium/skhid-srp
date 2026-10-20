package fl.transitions.easing
{
   public class Strong
   {
      
      public function Strong()
      {
         super();
      }
      
      public static function easeIn(time:Number, begin:Number, change:Number, duration:Number) : Number
      {
         return change * (time = time / duration) * time * time * time * time + begin;
      }
      
      public static function easeOut(time:Number, begin:Number, change:Number, duration:Number) : Number
      {
         return change * ((time = time / duration - 1) * time * time * time * time + 1) + begin;
      }
      
      public static function easeInOut(time:Number, begin:Number, change:Number, duration:Number) : Number
      {
         if((time = time / (duration / 2)) < 1)
         {
            return change / 2 * time * time * time * time * time + begin;
         }
         return change / 2 * ((time = time - 2) * time * time * time * time + 2) + begin;
      }
   }
}

