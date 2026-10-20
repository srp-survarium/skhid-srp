package scaleform.clik.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.TextField;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ComponentEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   import scaleform.clik.utils.Constraints;
   
   public class NumericStepper extends UIComponent
   {
      
      public var stepSize:Number = 1;
      
      public var constraintsDisabled:Boolean = false;
      
      protected var _maximum:Number = 10;
      
      protected var _minimum:Number = 0;
      
      protected var _stepSize:Number;
      
      protected var _value:Number = 0;
      
      protected var _labelFunction:Function;
      
      protected var state:String = "default";
      
      protected var _newFrame:String;
      
      public var textField:TextField;
      
      public var nextBtn:Button;
      
      public var prevBtn:Button;
      
      public var container:MovieClip;
      
      public var preventAutosizing:Boolean = true;
      
      public function NumericStepper()
      {
         super();
      }
      
      override protected function preInitialize() : void
      {
         if(!this.constraintsDisabled)
         {
            constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         }
      }
      
      override protected function initialize() : void
      {
         super.initialize();
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
         mouseEnabled = tabEnabled = value;
         gotoAndPlay(value ? (_focused > 0 ? "focused" : "default") : "disabled");
         if(!initialized)
         {
            return;
         }
         this.updateAfterStateChange();
         this.prevBtn.enabled = this.nextBtn.enabled = value;
      }
      
      override public function get focusable() : Boolean
      {
         return _focusable;
      }
      
      override public function set focusable(value:Boolean) : void
      {
         super.focusable = value;
      }
      
      public function get maximum() : Number
      {
         return this._maximum;
      }
      
      public function set maximum(value:Number) : void
      {
         _maximum = value;
         value = this._value;
      }
      
      public function get minimum() : Number
      {
         return this._minimum;
      }
      
      public function set minimum(value:Number) : void
      {
         _minimum = value;
         value = this._value;
      }
      
      public function get value() : Number
      {
         return this._value;
      }
      
      public function set value(v:Number) : void
      {
         v = this.lockValue(v);
         if(v == this._value)
         {
            return;
         }
         var previousValue:Number = this._value;
         _value = v;
         if(initialized)
         {
            dispatchEventAndSound(new IndexEvent(IndexEvent.INDEX_CHANGE,true,false,this.value,previousValue,null));
         }
         invalidate();
      }
      
      public function get labelFunction() : Function
      {
         return this._labelFunction;
      }
      
      public function set labelFunction(value:Function) : void
      {
         _labelFunction = value;
         this.updateLabel();
      }
      
      public function increment() : void
      {
         this.onNext(null);
      }
      
      public function decrement() : void
      {
         this.onPrev(null);
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
               if(this._value < this._maximum)
               {
                  if(keyPress)
                  {
                     this.onNext(null);
                  }
                  event.handled = true;
               }
               break;
            case NavigationCode.LEFT:
               if(this._value > this._minimum)
               {
                  if(keyPress)
                  {
                     this.onPrev(null);
                  }
                  event.handled = true;
               }
               break;
            case NavigationCode.HOME:
               if(!keyPress)
               {
                  value = this._minimum;
               }
               event.handled = true;
               break;
            case NavigationCode.END:
               if(!keyPress)
               {
                  value = this._maximum;
               }
               event.handled = true;
            break;
         }
      }
      
      override public function toString() : String
      {
         return "[CLIK NumericStepper " + name + "]";
      }
      
      override protected function configUI() : void
      {
         if(!this.constraintsDisabled)
         {
            constraints.addElement("textField",this.textField,Constraints.LEFT | Constraints.RIGHT);
         }
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         this.nextBtn.addEventListener(ButtonEvent.CLICK,this.onNext,false,0,true);
         this.prevBtn.addEventListener(ButtonEvent.CLICK,this.onPrev,false,0,true);
         tabEnabled = _focusable;
         tabChildren = false;
         if(this.textField != null)
         {
            this.textField.tabEnabled = this.textField.mouseEnabled = false;
         }
         if(this.container != null)
         {
            this.container.tabEnabled = this.container.mouseEnabled = false;
         }
         this.prevBtn.autoRepeat = this.nextBtn.autoRepeat = true;
         this.prevBtn.focusable = this.nextBtn.focusable = false;
         this.prevBtn.focusTarget = this.nextBtn.focusTarget = this;
         this.prevBtn.tabEnabled = this.nextBtn.tabEnabled = false;
         this.prevBtn.mouseEnabled = this.nextBtn.mouseEnabled = true;
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.STATE))
         {
            if(this._newFrame)
            {
               gotoAndPlay(this._newFrame);
               _newFrame = null;
            }
            this.updateAfterStateChange();
            dispatchEvent(new ComponentEvent(ComponentEvent.STATE_CHANGE));
         }
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateLabel();
            this.prevBtn.enabled = this._value > this._minimum;
            this.nextBtn.enabled = this._value < this._maximum;
         }
         if(isInvalid(InvalidationType.SIZE))
         {
            if(!this.preventAutosizing)
            {
               setActualSize(_width,_height);
            }
            if(!this.constraintsDisabled)
            {
               constraints.update(_width,_height);
            }
         }
      }
      
      override protected function changeFocus() : void
      {
         if(_focused || _displayFocus)
         {
            this.setState("focused","default");
         }
         else
         {
            this.setState("default");
         }
         this.updateAfterStateChange();
         this.prevBtn.displayFocus = this.nextBtn.displayFocus = _focused > 0;
      }
      
      protected function handleDataChange(event:Event) : void
      {
         invalidate(InvalidationType.DATA);
      }
      
      protected function updateAfterStateChange() : void
      {
         invalidateSize();
         this.updateLabel();
         if(this.textField != null)
         {
            this.textField.tabEnabled = this.textField.mouseEnabled = false;
         }
         if(this.container != null)
         {
            this.container.tabEnabled = this.container.mouseEnabled = false;
         }
         if(constraints != null && !this.constraintsDisabled)
         {
            constraints.updateElement("textField",this.textField);
         }
      }
      
      protected function updateLabel() : void
      {
         var label:String = this._value.toString();
         if(this._labelFunction != null)
         {
            label = this._labelFunction(this._value);
         }
         this.textField.text = label;
      }
      
      protected function onNext(event:ButtonEvent) : void
      {
         value = this._value + this.stepSize;
      }
      
      protected function onPrev(event:ButtonEvent) : void
      {
         value = this._value - this.stepSize;
      }
      
      protected function setState(... states) : void
      {
         if(states.length == 1)
         {
            onlyState = states[0].toString();
            if(this.state != onlyState && (_labelHash[onlyState]))
            {
               state = _newFrame = onlyState;
               invalidateState();
            }
            return;
         }
         var l:uint = (states.length);
         for(var i:uint = 0; i < l; i++)
         {
            var onlyState:String;
            var thisState:String;
            thisState = states[i].toString();
            if(_labelHash[thisState])
            {
               state = _newFrame = thisState;
               invalidateState();
               break;
            }
         }
      }
      
      protected function lockValue(value:Number) : Number
      {
         var _loc2_:Number = Math.max(this._minimum,Math.min(this._maximum,this.stepSize * Math.round(value / this.stepSize)));
         return _loc2_;
      }
   }
}

