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
   
   public class Slider extends UIComponent
   {
      
      public var liveDragging:Boolean = true;
      
      public var state:String = "default";
      
      public var offsetLeft:Number = 0;
      
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
      
      override public function get enabled() : Boolean
      {
         return super.enabled;
      }
      
      override public function set enabled(value:Boolean) : void
      {
         if(value == super.enabled)
         {
            return;
         }
         super.enabled = value;
         this.thumb.enabled = this.track.enabled = value;
      }
      
      override public function get focusable() : Boolean
      {
         return _focusable;
      }
      
      override public function set focusable(value:Boolean) : void
      {
         super.focusable = value;
         tabChildren = false;
      }
      
      public function get value() : Number
      {
         return this._value;
      }
      
      public function set value(value:Number) : void
      {
         _value = this.lockValue(value);
         dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,this._value));
         this.draw();
      }
      
      public function get maximum() : Number
      {
         return this._maximum;
      }
      
      public function set maximum(value:Number) : void
      {
         _maximum = value;
      }
      
      public function get minimum() : Number
      {
         return this._minimum;
      }
      
      public function set minimum(value:Number) : void
      {
         _minimum = value;
      }
      
      public function get position() : Number
      {
         return this._value;
      }
      
      public function set position(value:Number) : void
      {
         _value = value;
      }
      
      public function get snapping() : Boolean
      {
         return this._snapping;
      }
      
      public function set snapping(value:Boolean) : void
      {
         _snapping = value;
         this.invalidateSettings();
      }
      
      public function get snapInterval() : Number
      {
         return this._snapInterval;
      }
      
      public function set snapInterval(value:Number) : void
      {
         _snapInterval = value;
         this.invalidateSettings();
      }
      
      public function invalidateSettings() : void
      {
         invalidate(InvalidationType.SETTINGS);
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.isDefaultPrevented())
         {
            return;
         }
         var details:InputDetails = event.details;
         var index:uint = details.controllerIndex;
         var keyPress:Boolean = details.value == InputValue.KEY_DOWN || details.value == InputValue.KEY_HOLD;
         switch(details.navEquivalent)
         {
            case NavigationCode.RIGHT:
               if(keyPress)
               {
                  value += this._snapInterval;
                  event.handled = true;
               }
               break;
            case NavigationCode.LEFT:
               if(keyPress)
               {
                  value -= this._snapInterval;
                  event.handled = true;
               }
               break;
            case NavigationCode.HOME:
               if(!keyPress)
               {
                  value = this.minimum;
                  event.handled = true;
               }
               break;
            case NavigationCode.END:
               if(!keyPress)
               {
                  value = this.maximum;
                  event.handled = true;
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
         var lst_x:Number;
         if(isInvalid(InvalidationType.STATE))
         {
            gotoAndPlay(!this.enabled ? "disabled" : (_focused ? "focused" : "default"));
         }
         if(isInvalid(InvalidationType.SIZE))
         {
            lst_x = this.thumb.x;
            this.thumb.x = this.thumb.width;
            setActualSize(_width,_height);
            constraints.update(_width,_height);
            this.thumb.x = lst_x;
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
         var trackWidth:Number = _width - this.offsetLeft - this.offsetRight;
         this.thumb.x = (this._value - this._minimum) / (this._maximum - this._minimum) * trackWidth - this.thumb.width / 2 + this.offsetLeft;
      }
      
      protected function beginDrag(e:MouseEvent) : void
      {
         _thumbPressed = true;
         var lp:Point = globalToLocal(new Point(e.stageX,e.stageY));
         _dragOffset = {x:lp.x - this.thumb.x - this.thumb.width / 2};
         stage.addEventListener(MouseEvent.MOUSE_MOVE,this.doDrag,false,0,true);
         stage.addEventListener(MouseEvent.MOUSE_UP,this.endDrag,false,0,true);
      }
      
      protected function doDrag(e:MouseEvent) : void
      {
         var lp:Point = globalToLocal(new Point(e.stageX,e.stageY));
         var thumbPosition:Number = lp.x - this._dragOffset.x;
         var trackWidth:Number = _width - this.offsetLeft - this.offsetRight;
         var newValue:Number = this.lockValue((thumbPosition - this.offsetLeft) / trackWidth * (this._maximum - this._minimum) + this._minimum);
         if(this.value == newValue)
         {
            return;
         }
         _value = newValue;
         this.updateThumb();
         if(this.liveDragging)
         {
            dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,this._value));
         }
      }
      
      protected function endDrag(e:MouseEvent) : void
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
      
      protected function trackPress(e:MouseEvent) : void
      {
         _trackPressed = true;
         this.track.focused = _focused;
         var trackWidth:Number = _width - this.offsetLeft - this.offsetRight;
         var newValue:Number = this.lockValue((e.localX * scaleX - this.offsetLeft) / trackWidth * (this._maximum - this._minimum) + this._minimum);
         if(this.value == newValue)
         {
            return;
         }
         value = newValue;
         if(!this.liveDragging)
         {
            dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,this._value));
         }
         _trackDragMouseIndex = 0;
         _dragOffset = {x:0};
      }
      
      protected function lockValue(lvalue:Number) : Number
      {
         lvalue = Math.max(this._minimum,Math.min(this._maximum,lvalue));
         if(!this.snapping)
         {
            return lvalue;
         }
         var _loc2_:Number = Math.round(lvalue / this.snapInterval) * this.snapInterval;
         return _loc2_;
      }
      
      protected function scrollWheel(delta:Number) : void
      {
         if(_focused)
         {
            value -= delta * this._snapInterval;
            dispatchEvent(new Event(Event.CHANGE));
         }
      }
   }
}

