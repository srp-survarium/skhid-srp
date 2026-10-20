package scaleform.clik.controls
{
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.geom.Point;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.events.SliderEvent;
   import scaleform.clik.ui.InputDetails;
   import scaleform.clik.utils.Constraints;
   
   [Event(name="change",type="flash.events.Event")]
   public class Slider extends UIComponent
   {
      
      [Inspectable(defaultValue="true")]
      public var liveDragging:Boolean = true;
      
      public var state:String = "default";
      
      [Inspectable(defaultValue="0",verbose="1")]
      public var offsetLeft:Number = 0;
      
      [Inspectable(defaultValue="0",verbose="1")]
      public var offsetRight:Number = 0;
      
      protected var _minimum:Number = 0;
      
      protected var _maximum:Number = 10;
      
      protected var _value:Number = 0;
      
      protected var _snapInterval:Number = 1;
      
      protected var _snapping:Boolean = false;
      
      protected var _dragOffset:Object;
      
      protected var _trackDragMouseIndex:Number;
      
      protected var _trackPressed:Boolean = false;
      
      protected var _thumbPressed:Boolean = false;
      
      public var thumb:Button;
      
      public var track:Button;
      
      public function Slider()
      {
         super();
      }
      
      override protected function preInitialize() : void
      {
         constraints = new Constraints(this,ConstrainMode.REFLOW);
      }
      
      override protected function initialize() : void
      {
         super.initialize();
         tabChildren = false;
         mouseEnabled = mouseChildren = this.enabled;
      }
      
      [Inspectable(defaultValue="true")]
      override public function get enabled() : Boolean
      {
         return super.enabled;
      }
      
      override public function set enabled(param1:Boolean) : void
      {
         if(param1 == super.enabled)
         {
            return;
         }
         super.enabled = param1;
         this.thumb.enabled = this.track.enabled = param1;
      }
      
      [Inspectable(defaultValue="true")]
      override public function get focusable() : Boolean
      {
         return _focusable;
      }
      
      override public function set focusable(param1:Boolean) : void
      {
         super.focusable = param1;
         tabChildren = false;
      }
      
      [Inspectable(defaultValue="0")]
      public function get value() : Number
      {
         return this._value;
      }
      
      public function set value(param1:Number) : void
      {
         _value = this.lockValue(param1);
         dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,this._value));
         this.draw();
      }
      
      [Inspectable(defaultValue="10")]
      public function get maximum() : Number
      {
         return this._maximum;
      }
      
      public function set maximum(param1:Number) : void
      {
         _maximum = param1;
      }
      
      [Inspectable(defaultValue="0")]
      public function get minimum() : Number
      {
         return this._minimum;
      }
      
      public function set minimum(param1:Number) : void
      {
         _minimum = param1;
      }
      
      public function get position() : Number
      {
         return this._value;
      }
      
      public function set position(param1:Number) : void
      {
         _value = param1;
      }
      
      [Inspectable(defaultValue="false")]
      public function get snapping() : Boolean
      {
         return this._snapping;
      }
      
      public function set snapping(param1:Boolean) : void
      {
         _snapping = param1;
         this.invalidateSettings();
      }
      
      [Inspectable(defaultValue="1")]
      public function get snapInterval() : Number
      {
         return this._snapInterval;
      }
      
      public function set snapInterval(param1:Number) : void
      {
         _snapInterval = param1;
         this.invalidateSettings();
      }
      
      public function invalidateSettings() : void
      {
         invalidate(InvalidationType.SETTINGS);
      }
      
      override public function handleInput(param1:InputEvent) : void
      {
         if(param1.isDefaultPrevented())
         {
            return;
         }
         var _loc2_:InputDetails = param1.details;
         var _loc3_:uint = _loc2_.controllerIndex;
         var _loc4_:Boolean = _loc2_.value == InputValue.KEY_DOWN || _loc2_.value == InputValue.KEY_HOLD;
         switch(_loc2_.navEquivalent)
         {
            case NavigationCode.RIGHT:
               if(_loc4_)
               {
                  value += this._snapInterval;
                  param1.handled = true;
               }
               break;
            case NavigationCode.LEFT:
               if(_loc4_)
               {
                  value -= this._snapInterval;
                  param1.handled = true;
               }
               break;
            case NavigationCode.HOME:
               if(!_loc4_)
               {
                  value = this.minimum;
                  param1.handled = true;
               }
               break;
            case NavigationCode.END:
               if(!_loc4_)
               {
                  value = this.maximum;
                  param1.handled = true;
               }
               break;
            default:
               break;
         }
      }
      
      override public function toString() : String
      {
         return "[CLIK Slider " + name + "]";
      }
      
      override protected function configUI() : void
      {
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         this.thumb.addEventListener(MouseEvent.MOUSE_DOWN,this.beginDrag,false,0,true);
         this.track.addEventListener(MouseEvent.MOUSE_DOWN,this.trackPress,false,0,true);
         tabEnabled = true;
         this.thumb.focusTarget = this.track.focusTarget = this;
         this.thumb.enabled = this.track.enabled = this.enabled;
         this.thumb.lockDragStateChange = true;
         constraints.addElement("track",this.track,Constraints.LEFT | Constraints.RIGHT);
      }
      
      override protected function draw() : void
      {
         var _loc1_:Number;
         if(isInvalid(InvalidationType.STATE))
         {
            gotoAndPlay(!this.enabled ? "disabled" : (_focused ? "focused" : "default"));
         }
         if(isInvalid(InvalidationType.SIZE))
         {
            _loc1_ = this.thumb.x;
            this.thumb.x = this.thumb.width;
            setActualSize(_width,_height);
            constraints.update(_width,_height);
            this.thumb.x = _loc1_;
         }
         this.updateThumb();
      }
      
      override protected function changeFocus() : void
      {
         super.changeFocus();
         invalidateState();
         if(this.enabled)
         {
            if(!this._thumbPressed)
            {
               this.thumb.displayFocus = _focused != 0;
            }
            if(!this._trackPressed)
            {
               this.track.displayFocus = _focused != 0;
            }
         }
      }
      
      protected function updateThumb() : void
      {
         if(!this.enabled)
         {
            return;
         }
         var _loc1_:Number = _width - this.offsetLeft - this.offsetRight;
         this.thumb.x = (this._value - this._minimum) / (this._maximum - this._minimum) * _loc1_ - this.thumb.width / 2 + this.offsetLeft;
      }
      
      protected function beginDrag(param1:MouseEvent) : void
      {
         _thumbPressed = true;
         var _loc2_:Point = globalToLocal(new Point(param1.stageX,param1.stageY));
         _dragOffset = {x:_loc2_.x - this.thumb.x - this.thumb.width / 2};
         stage.addEventListener(MouseEvent.MOUSE_MOVE,this.doDrag,false,0,true);
         stage.addEventListener(MouseEvent.MOUSE_UP,this.endDrag,false,0,true);
      }
      
      protected function doDrag(param1:MouseEvent) : void
      {
         var _loc2_:Point = globalToLocal(new Point(param1.stageX,param1.stageY));
         var _loc3_:Number = _loc2_.x - this._dragOffset.x;
         var _loc4_:Number = _width - this.offsetLeft - this.offsetRight;
         var _loc5_:Number = this.lockValue((_loc3_ - this.offsetLeft) / _loc4_ * (this._maximum - this._minimum) + this._minimum);
         if(this.value == _loc5_)
         {
            return;
         }
         _value = _loc5_;
         this.updateThumb();
         if(this.liveDragging)
         {
            dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,this._value));
         }
      }
      
      protected function endDrag(param1:MouseEvent) : void
      {
         stage.removeEventListener(MouseEvent.MOUSE_MOVE,this.doDrag,false);
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.endDrag,false);
         if(!this.liveDragging)
         {
            dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,this._value));
         }
         _trackDragMouseIndex = undefined;
         _thumbPressed = false;
         _trackPressed = false;
      }
      
      protected function trackPress(param1:MouseEvent) : void
      {
         _trackPressed = true;
         this.track.focused = _focused;
         var _loc2_:Number = _width - this.offsetLeft - this.offsetRight;
         var _loc3_:Number = this.lockValue((param1.localX * scaleX - this.offsetLeft) / _loc2_ * (this._maximum - this._minimum) + this._minimum);
         if(this.value == _loc3_)
         {
            return;
         }
         value = _loc3_;
         if(!this.liveDragging)
         {
            dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,this._value));
         }
         _trackDragMouseIndex = 0;
         _dragOffset = {x:0};
      }
      
      protected function lockValue(param1:Number) : Number
      {
         param1 = Math.max(this._minimum,Math.min(this._maximum,param1));
         if(!this.snapping)
         {
            return param1;
         }
         var _loc2_:Number = Math.round(param1 / this.snapInterval) * this.snapInterval;
         return _loc2_;
      }
      
      protected function scrollWheel(param1:Number) : void
      {
         if(_focused)
         {
            value -= param1 * this._snapInterval;
            dispatchEvent(new Event(Event.CHANGE));
         }
      }
   }
}

