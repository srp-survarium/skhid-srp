package scaleform.clik.controls
{
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.core.UIComponent;
   
   public class StatusIndicator extends UIComponent
   {
      
      protected var _maximum:Number = 10;
      
      protected var _minimum:Number = 0;
      
      protected var _value:Number = 0;
      
      public function StatusIndicator()
      {
         super();
      }
      
      override protected function initialize() : void
      {
         super.initialize();
      }
      
      [Inspectable(defaultValue="10")]
      public function get maximum() : Number
      {
         return this._maximum;
      }
      
      public function set maximum(value:Number) : void
      {
         _maximum = value;
      }
      
      [Inspectable(defaultValue="0")]
      public function get minimum() : Number
      {
         return this._minimum;
      }
      
      public function set minimum(value:Number) : void
      {
         if(this._minimum == value)
         {
            return;
         }
         _minimum = value;
         this.updatePosition();
      }
      
      [Inspectable(defaultValue="0")]
      public function get value() : Number
      {
         return this._value;
      }
      
      public function set value(value:Number) : void
      {
         var range:Number = Math.max(this._minimum,Math.min(this._maximum,value));
         if(this._value == range)
         {
            return;
         }
         _value = range;
         this.updatePosition();
      }
      
      public function get position() : Number
      {
         return this._value;
      }
      
      public function set position(value:Number) : void
      {
         this.value = value;
      }
      
      override public function toString() : String
      {
         return "[CLIK StatusIndicator " + name + "]";
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         tabEnabled = focusable = false;
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.SIZE))
         {
            setActualSize(_width,_height);
         }
         if(isInvalid(InvalidationType.DATA))
         {
            this.updatePosition();
         }
      }
      
      protected function updatePosition() : void
      {
         if(!enabled)
         {
            return;
         }
         var percent:Number = (this._value - this._minimum) / (this._maximum - this._minimum);
         gotoAndStop(Math.max(1,Math.round(percent * totalFrames)));
      }
   }
}

