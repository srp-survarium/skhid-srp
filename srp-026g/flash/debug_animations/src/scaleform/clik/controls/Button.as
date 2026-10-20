package scaleform.clik.controls
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.Timer;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ComponentEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   import scaleform.clik.utils.ConstrainedElement;
   import scaleform.clik.utils.Constraints;
   import scaleform.gfx.MouseEventEx;
   
   public class Button extends ToolTip
   {
      
      public var lockDragStateChange:Boolean = false;
      
      public var repeatDelay:Number = 500;
      
      public var repeatInterval:Number = 200;
      
      public var constraintsDisabled:Boolean = false;
      
      public var allowDeselect:Boolean = true;
      
      public var preventAutosizing:Boolean = false;
      
      protected var _toggle:Boolean = false;
      
      protected var _label:String;
      
      protected var _state:String;
      
      protected var _group:ButtonGroup;
      
      protected var _groupName:String;
      
      protected var _selected:Boolean = false;
      
      protected var _autoRepeat:Boolean = false;
      
      protected var _autoSize:String = "none";
      
      protected var _pressedByKeyboard:Boolean = false;
      
      protected var _isRepeating:Boolean = false;
      
      protected var _owner:UIComponent = null;
      
      protected var _stateMap:Object = {
         "up":["up"],
         "over":["over"],
         "down":["down"],
         "release":["release","over"],
         "out":["out","up"],
         "disabled":["disabled"],
         "selecting":["selecting","over"],
         "toggle":["toggle","up"],
         "kb_selecting":["kb_selecting","up"],
         "kb_release":["kb_release","out","up"],
         "kb_down":["kb_down","down"]
      };
      
      protected var _newFrame:String;
      
      protected var _newFocusIndicatorFrame:String;
      
      protected var _repeatTimer:Timer;
      
      protected var _mouseDown:int = 0;
      
      protected var _focusIndicatorLabelHash:Object;
      
      protected var _autoRepeatEvent:ButtonEvent;
      
      public var textField:TextField;
      
      public var defaultTextFormat:TextFormat;
      
      protected var _focusIndicator:MovieClip;
      
      protected var statesDefault:Vector.<String> = Vector.<String>([""]);
      
      protected var statesSelected:Vector.<String> = Vector.<String>(["selected_",""]);
      
      public function Button()
      {
         super();
         buttonMode = true;
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
         tabEnabled = true;
      }
      
      public function get data() : Object
      {
         return _data;
      }
      
      public function set data(param1:Object) : void
      {
         _data = param1;
      }
      
      public function get autoRepeat() : Boolean
      {
         return this._autoRepeat;
      }
      
      public function set autoRepeat(param1:Boolean) : void
      {
         _autoRepeat = param1;
      }
      
      override public function get enabled() : Boolean
      {
         return super.enabled;
      }
      
      override public function set enabled(param1:Boolean) : void
      {
         var _loc2_:String;
         super.enabled = param1;
         mouseChildren = false;
         if(super.enabled)
         {
            _loc2_ = this._focusIndicator == null && (_displayFocus || _focused) ? "over" : "up";
         }
         else
         {
            _loc2_ = "disabled";
         }
         this.setState(_loc2_);
      }
      
      override public function get focusable() : Boolean
      {
         return _focusable;
      }
      
      override public function set focusable(param1:Boolean) : void
      {
         super.focusable = param1;
      }
      
      public function get toggle() : Boolean
      {
         return this._toggle;
      }
      
      public function set toggle(param1:Boolean) : void
      {
         _toggle = param1;
      }
      
      public function get owner() : UIComponent
      {
         return this._owner;
      }
      
      public function set owner(param1:UIComponent) : void
      {
         _owner = param1;
      }
      
      public function get state() : String
      {
         return this._state;
      }
      
      public function get selected() : Boolean
      {
         return this._selected;
      }
      
      public function set selected(param1:Boolean) : void
      {
         var _loc2_:Boolean;
         if(this._selected == param1)
         {
            return;
         }
         _selected = param1;
         if(this.enabled)
         {
            if(!_focused)
            {
               this.setState("toggle");
            }
            else if(this._pressedByKeyboard && this._focusIndicator != null)
            {
               this.setState("kb_selecting");
            }
            else
            {
               this.setState("selecting");
            }
            if(this.owner)
            {
               _loc2_ = this._selected && this.owner != null && this.checkOwnerFocused();
               this.setState(_loc2_ && this._focusIndicator == null ? "selecting" : "toggle");
               displayFocus = _loc2_;
            }
         }
         else
         {
            this.setState("disabled");
         }
         validateNow();
         dispatchEvent(new Event(Event.SELECT));
      }
      
      public function get group() : ButtonGroup
      {
         return this._group;
      }
      
      public function set group(param1:ButtonGroup) : void
      {
         if(this._group != null)
         {
            this._group.removeButton(this);
         }
         _group = param1;
         if(this._group != null)
         {
            this._group.addButton(this);
         }
      }
      
      public function get groupName() : String
      {
         return this._groupName;
      }
      
      public function set groupName(param1:String) : void
      {
         if(_inspector && param1 == "")
         {
            return;
         }
         if(this._groupName == param1)
         {
            return;
         }
         if(param1 != null)
         {
            addEventListener(Event.ADDED,this.addToAutoGroup,false,0,true);
            addEventListener(Event.REMOVED,this.addToAutoGroup,false,0,true);
         }
         else
         {
            removeEventListener(Event.ADDED,this.addToAutoGroup,false);
            removeEventListener(Event.REMOVED,this.addToAutoGroup,false);
         }
         _groupName = param1;
         this.addToAutoGroup(null);
      }
      
      public function get label() : String
      {
         return this._label;
      }
      
      public function set label(param1:String) : void
      {
         if(this._label == param1)
         {
            return;
         }
         _label = param1;
         invalidateData();
      }
      
      public function get autoSize() : String
      {
         return this._autoSize;
      }
      
      public function set autoSize(param1:String) : void
      {
         if(param1 == this._autoSize)
         {
            return;
         }
         _autoSize = param1;
         invalidateData();
      }
      
      public function get focusIndicator() : MovieClip
      {
         return this._focusIndicator;
      }
      
      public function set focusIndicator(param1:MovieClip) : void
      {
         _focusIndicatorLabelHash = null;
         _focusIndicator = param1;
         _focusIndicatorLabelHash = UIComponent.generateLabelHash(this._focusIndicator);
      }
      
      override public function handleInput(param1:InputEvent) : void
      {
         if(param1.isDefaultPrevented())
         {
            return;
         }
         var _loc2_:InputDetails = param1.details;
         var _loc3_:uint = _loc2_.controllerIndex;
         switch(_loc2_.navEquivalent)
         {
            case NavigationCode.ENTER:
               if(_loc2_.value == InputValue.KEY_DOWN)
               {
                  this.handlePress(_loc3_);
                  param1.handled = true;
               }
               else if(_loc2_.value == InputValue.KEY_UP)
               {
                  if(this._pressedByKeyboard)
                  {
                     this.handleRelease(_loc3_);
                     param1.handled = true;
                  }
               }
               break;
         }
      }
      
      override public function toString() : String
      {
         return "[CLIK Button " + name + "]";
      }
      
      override protected function configUI() : void
      {
         if(!this.constraintsDisabled)
         {
            constraints.addElement("textField",this.textField,Constraints.ALL);
         }
         super.configUI();
         tabEnabled = _focusable && this.enabled && tabEnabled;
         mouseChildren = tabChildren = false;
         addEventListener(MouseEvent.ROLL_OVER,this.handleMouseRollOver,false,0,true);
         addEventListener(MouseEvent.ROLL_OUT,this.handleMouseRollOut,false,0,true);
         addEventListener(MouseEvent.MOUSE_DOWN,this.handleMousePress,false,0,true);
         addEventListener(MouseEvent.CLICK,this.handleMouseRelease,false,0,true);
         addEventListener(MouseEvent.DOUBLE_CLICK,this.handleMouseRelease,false,0,true);
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         if(this._focusIndicator != null && !_focused && this._focusIndicator.totalFrames == 1)
         {
            this.focusIndicator.visible = false;
         }
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
            if(this._newFocusIndicatorFrame)
            {
               this.focusIndicator.gotoAndPlay(this._newFocusIndicatorFrame);
               _newFocusIndicatorFrame = null;
            }
            this.updateAfterStateChange();
            dispatchEvent(new ComponentEvent(ComponentEvent.STATE_CHANGE));
            invalidate(InvalidationType.SIZE);
         }
         if(isInvalid(InvalidationType.DATA) || isInvalid(InvalidationType.STATE))
         {
            this.updateText();
            if(this.autoSize != TextFieldAutoSize.NONE)
            {
               invalidateSize();
            }
         }
         if(isInvalid(InvalidationType.SIZE))
         {
            if(!this.preventAutosizing)
            {
               this.alignForAutoSize();
               setActualSize(_width,_height);
            }
            if(!this.constraintsDisabled)
            {
               constraints.update(_width,_height);
            }
         }
      }
      
      protected function addToAutoGroup(param1:Event) : void
      {
         if(parent == null)
         {
            group = null;
            return;
         }
         var _loc2_:ButtonGroup = ButtonGroup.getGroup(this._groupName,parent);
         if(_loc2_ == this.group)
         {
            return;
         }
         group = _loc2_;
      }
      
      protected function checkOwnerFocused() : Boolean
      {
         var _loc1_:Boolean = false;
         var _loc2_:Object;
         if(this.owner != null)
         {
            _loc1_ = this._owner.focused != 0;
            if(_loc1_ == 0)
            {
               _loc2_ = this._owner.focusTarget;
               if(_loc2_ != null)
               {
                  _loc1_ = _loc2_ != 0;
               }
            }
         }
         return _loc1_;
      }
      
      protected function calculateWidth() : Number
      {
         var _loc1_:Number = actualWidth;
         var _loc2_:ConstrainedElement;
         if(!this.constraintsDisabled)
         {
            _loc2_ = constraints.getElement("textField");
            _loc1_ = Math.ceil(this.textField.textWidth + _loc2_.left + _loc2_.right + 5);
         }
         return _loc1_;
      }
      
      protected function alignForAutoSize() : void
      {
         var _loc1_:Number;
         if(!initialized || this._autoSize == TextFieldAutoSize.NONE || this.textField == null)
         {
            return;
         }
         _loc1_ = _width;
         var _loc2_:Number = _width = this.calculateWidth();
         var _loc3_:Number;
         var _loc4_:Number;
         switch(this._autoSize)
         {
            case TextFieldAutoSize.RIGHT:
               _loc3_ = x + _loc1_;
               x = _loc3_ - _loc2_;
               break;
            case TextFieldAutoSize.CENTER:
               _loc4_ = x + _loc1_ * 0.5;
               x = _loc4_ - _loc2_ * 0.5;
               break;
         }
      }
      
      protected function updateText() : void
      {
         if(this._label != null && this.textField != null)
         {
            this.textField.text = this._label;
         }
      }
      
      override protected function changeFocus() : void
      {
         var _loc1_:String;
         if(!this.enabled)
         {
            return;
         }
         if(this._focusIndicator == null)
         {
            this.setState((_focused) || _displayFocus ? "over" : "out");
            if(this._pressedByKeyboard && !_focused)
            {
               _pressedByKeyboard = false;
            }
         }
         else
         {
            if(this._focusIndicator.totalframes == 1)
            {
               this._focusIndicator.visible = _focused > 0;
            }
            else
            {
               _loc1_ = "state" + _focused;
               if(this._focusIndicatorLabelHash[_loc1_])
               {
                  _newFocusIndicatorFrame = "state" + _focused;
               }
               else
               {
                  _newFocusIndicatorFrame = (_focused) || _displayFocus ? "show" : "hide";
               }
               invalidateState();
            }
            if(this._pressedByKeyboard && !_focused)
            {
               this.setState("kb_release");
               _pressedByKeyboard = false;
            }
         }
      }
      
      protected function handleMouseRollOver(param1:MouseEvent) : void
      {
         var _loc2_:MouseEventEx = param1 as MouseEventEx;
         var _loc3_:uint = _loc2_ == null ? 0 : _loc2_.mouseIdx;
         if(param1.buttonDown)
         {
            dispatchEvent(new ButtonEvent(ButtonEvent.DRAG_OVER));
            if(!this.enabled)
            {
               return;
            }
            if(this.lockDragStateChange && Boolean(this._mouseDown << _loc3_ & 1))
            {
               return;
            }
            if(_focused || _displayFocus)
            {
               this.setState(this.focusIndicator == null ? "down" : "kb_down");
            }
            else
            {
               this.setState("over");
            }
         }
         else
         {
            if(!this.enabled)
            {
               return;
            }
            if(_focused || _displayFocus)
            {
               if(this._focusIndicator != null)
               {
                  this.setState("over");
               }
            }
            else
            {
               this.setState("over");
            }
         }
      }
      
      protected function handleMouseRollOut(param1:MouseEvent) : void
      {
         var _loc2_:MouseEventEx = param1 as MouseEventEx;
         var _loc3_:uint = _loc2_ == null ? 0 : _loc2_.mouseIdx;
         if(param1.buttonDown)
         {
            dispatchEvent(new ButtonEvent(ButtonEvent.DRAG_OUT));
            if(Boolean(this._mouseDown & 1 << _loc3_))
            {
               if(stage != null)
               {
                  stage.addEventListener(MouseEvent.MOUSE_UP,this.handleReleaseOutside,false,0,true);
               }
            }
            if(this.lockDragStateChange || !this.enabled)
            {
               return;
            }
            if(_focused || _displayFocus)
            {
               this.setState(this._focusIndicator == null ? "release" : "kb_release");
            }
            else
            {
               this.setState("out");
            }
         }
         else
         {
            if(!this.enabled)
            {
               return;
            }
            if(_focused || _displayFocus)
            {
               if(this._focusIndicator != null)
               {
                  this.setState("out");
               }
            }
            else
            {
               this.setState("out");
            }
         }
      }
      
      protected function handleMousePress(param1:MouseEvent) : void
      {
         var _loc2_:MouseEventEx = param1 as MouseEventEx;
         var _loc3_:uint = _loc2_ == null ? 0 : _loc2_.mouseIdx;
         var _loc4_:uint = _loc2_ == null ? 0 : _loc2_.buttonIdx;
         var _loc5_:ButtonEvent;
         if(_loc4_ != 0)
         {
            return;
         }
         _mouseDown |= 1 << _loc3_;
         if(this.enabled)
         {
            this.setState("down");
            if(this.autoRepeat && this._repeatTimer == null)
            {
               _autoRepeatEvent = new ButtonEvent(ButtonEvent.CLICK,true,false,_loc3_,_loc4_,false,true);
               _repeatTimer = new Timer(this.repeatDelay,1);
               this._repeatTimer.addEventListener(TimerEvent.TIMER_COMPLETE,this.beginRepeat,false,0,true);
               this._repeatTimer.start();
            }
            _loc5_ = new ButtonEvent(ButtonEvent.PRESS,true,false,_loc3_,_loc4_,false,false);
            dispatchEvent(_loc5_);
         }
      }
      
      protected function handleMouseRelease(param1:MouseEvent) : void
      {
         _autoRepeatEvent = null;
         if(!this.enabled)
         {
            return;
         }
         var _loc2_:MouseEventEx = param1 as MouseEventEx;
         var _loc3_:uint = _loc2_ == null ? 0 : _loc2_.mouseIdx;
         var _loc4_:uint = _loc2_ == null ? 0 : _loc2_.buttonIdx;
         var _loc5_:ButtonEvent;
         if(_loc4_ != 0)
         {
            return;
         }
         _mouseDown ^= 1 << _loc3_;
         if(this._mouseDown == 0 && this._repeatTimer)
         {
            this._repeatTimer.stop();
            this._repeatTimer.reset();
            this._repeatTimer.removeEventListener(TimerEvent.TIMER_COMPLETE,this.beginRepeat);
            this._repeatTimer.removeEventListener(TimerEvent.TIMER,this.handleRepeat);
            _repeatTimer = null;
         }
         this.setState("release");
         this.handleClick(_loc3_);
         if(!this._isRepeating)
         {
            _loc5_ = new ButtonEvent(ButtonEvent.CLICK,true,false,_loc3_,_loc4_,false,false);
            dispatchEvent(_loc5_);
         }
         _isRepeating = false;
      }
      
      protected function handleReleaseOutside(param1:MouseEvent) : void
      {
         _autoRepeatEvent = null;
         if(contains(param1.target as DisplayObject))
         {
            return;
         }
         var _loc2_:MouseEventEx = param1 as MouseEventEx;
         var _loc3_:uint = _loc2_ == null ? 0 : _loc2_.mouseIdx;
         var _loc4_:uint = _loc2_ == null ? 0 : _loc2_.buttonIdx;
         if(_loc4_ != 0)
         {
            return;
         }
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.handleReleaseOutside,false);
         _mouseDown ^= 1 << _loc3_;
         dispatchEvent(new ButtonEvent(ButtonEvent.RELEASE_OUTSIDE));
         if(!this.enabled)
         {
            return;
         }
         if(this.lockDragStateChange)
         {
            if(_focused || _displayFocus)
            {
               this.setState(this.focusIndicator == null ? "release" : "kb_release");
            }
            else
            {
               this.setState("kb_release");
            }
         }
      }
      
      protected function handlePress(param1:uint = 0) : void
      {
         if(!this.enabled)
         {
            return;
         }
         _pressedByKeyboard = true;
         this.setState(this._focusIndicator == null ? "down" : "kb_down");
         if(this.autoRepeat && this._repeatTimer == null)
         {
            _autoRepeatEvent = new ButtonEvent(ButtonEvent.CLICK,true,false,param1,0,true,true);
            _repeatTimer = new Timer(this.repeatDelay,1);
            this._repeatTimer.addEventListener(TimerEvent.TIMER_COMPLETE,this.beginRepeat,false,0,true);
            this._repeatTimer.start();
         }
         var _loc2_:ButtonEvent = new ButtonEvent(ButtonEvent.PRESS,true,false,param1,0,true,false);
         dispatchEvent(_loc2_);
      }
      
      protected function handleRelease(param1:uint = 0) : void
      {
         var _loc2_:ButtonEvent;
         if(!this.enabled)
         {
            return;
         }
         this.setState(this.focusIndicator == null ? "release" : "kb_release");
         if(this._repeatTimer)
         {
            this._repeatTimer.stop();
            this._repeatTimer.reset();
            this._repeatTimer.removeEventListener(TimerEvent.TIMER_COMPLETE,this.beginRepeat);
            this._repeatTimer.removeEventListener(TimerEvent.TIMER,this.handleRepeat);
            _repeatTimer = null;
         }
         this.handleClick(param1);
         _pressedByKeyboard = false;
         if(!this._isRepeating)
         {
            _loc2_ = new ButtonEvent(ButtonEvent.CLICK,true,false,param1,0,true,false);
            dispatchEvent(_loc2_);
         }
         _isRepeating = false;
      }
      
      protected function handleClick(param1:uint = 0) : void
      {
         if(this.toggle && (!this.selected || this.allowDeselect))
         {
            selected = !this.selected;
         }
      }
      
      protected function beginRepeat(param1:TimerEvent) : void
      {
         this._repeatTimer.delay = this.repeatInterval;
         this._repeatTimer.repeatCount = 0;
         this._repeatTimer.removeEventListener(TimerEvent.TIMER_COMPLETE,this.beginRepeat);
         this._repeatTimer.addEventListener(TimerEvent.TIMER,this.handleRepeat,false,0,true);
         this._repeatTimer.reset();
         this._repeatTimer.start();
      }
      
      protected function handleRepeat(param1:TimerEvent) : void
      {
         if(this._mouseDown == 0 && !this._pressedByKeyboard)
         {
            this._repeatTimer.stop();
            this._repeatTimer.reset();
            this._repeatTimer.removeEventListener(TimerEvent.TIMER_COMPLETE,this.beginRepeat);
            this._repeatTimer.removeEventListener(TimerEvent.TIMER,this.handleRepeat);
            _repeatTimer = null;
         }
         if(this._autoRepeatEvent)
         {
            _isRepeating = true;
            dispatchEvent(this._autoRepeatEvent);
         }
      }
      
      protected function setState(param1:String) : void
      {
         _state = param1;
         var _loc2_:Vector.<String> = this.getStatePrefixes();
         var _loc3_:Array = this._stateMap[param1];
         if(_loc3_ == null || _loc3_.length == 0)
         {
            return;
         }
         var _loc4_:uint = _loc2_.length;
         var _loc5_:uint = 0;
         var _loc6_:String;
         var _loc7_:uint;
         var _loc8_:uint;
         var _loc9_:String;
         while(_loc5_ < _loc4_)
         {
            _loc6_ = _loc2_[_loc5_];
            _loc7_ = _loc3_.length;
            _loc8_ = 0;
            while(_loc8_ < _loc7_)
            {
               _loc9_ = _loc6_ + _loc3_[_loc8_];
               if(_labelHash[_loc9_])
               {
                  _newFrame = _loc9_;
                  invalidateState();
                  return;
               }
               _loc8_++;
            }
            _loc5_++;
         }
      }
      
      protected function getStatePrefixes() : Vector.<String>
      {
         return this._selected ? this.statesSelected : this.statesDefault;
      }
      
      protected function updateAfterStateChange() : void
      {
         if(!initialized)
         {
            return;
         }
         if(constraints != null && !this.constraintsDisabled && this.textField != null)
         {
            constraints.updateElement("textField",this.textField);
         }
      }
   }
}

