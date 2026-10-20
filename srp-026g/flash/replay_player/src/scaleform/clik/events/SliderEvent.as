package scaleform.clik.events
{
   import flash.events.Event;
   
   public class SliderEvent extends Event
   {
      
      public static const VALUE_CHANGE:String = "valueChange";
      
      public var value:Number = -1;
      
      public function SliderEvent(type:String, bubbles:Boolean = false, cancelable:Boolean = true, value:Number = -1)
      {
         super(type,bubbles,cancelable);
         this.value = value;
      }
      
      override public function clone() : Event
      {
         return new SliderEvent(type,bubbles,cancelable,this.value);
      }
      
      override public function toString() : String
      {
         return formatToString("SliderEvent","type","bubbles","cancelable","value");
      }
   }
}

