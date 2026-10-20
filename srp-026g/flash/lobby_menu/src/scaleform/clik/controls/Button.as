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
   
   [Event(name="SHOW",type="scaleform.clik.events.ComponentEvent")]
   [Event(name="HIDE",type="scaleform.clik.events.ComponentEvent")]
   [Event(name="FOCUS_IN",type="scaleform.clik.events.FocusHandlerEvent")]
   [Event(name="FOCUS_OUT",type="scaleform.clik.events.FocusHandlerEvent")]
   [Event(name="SELECT",type="flash.events.Event")]
   [Event(name="STATE_CHANGE",type="scaleform.clik.events.ComponentEvent")]
   [Event(name="PRESS",type="scaleform.clik.events.ButtonEvent")]
   [Event(name="CLICK",type="scaleform.clik.events.ButtonEvent")]
   [Event(name="DRAG_OVER",type="scaleform.clik.events.ButtonEvent")]
   [Event(name="DRAG_OUT",type="scaleform.clik.events.ButtonEvent")]
   [Event(name="RELEASE_OUTSIDE",type="scaleform.clik.events.ButtonEvent")]
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
         up:["up"],
         over:["over"],
         down:["down"],
         release:["release","over"],
         out:["out","up"],
         disabled:["disabled"],
         selecting:["selecting","over"],
         toggle:["toggle","up"],
         kb_selecting:["kb_selecting","up"],
         kb_release:["kb_release","out","up"],
         kb_down:["kb_down","down"]
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
      
      [Inspectable(type="string",defaultValue="")]
      public function get data() : Object
      {
         return _data;
      }
      
      public function set data(value:Object) : void
      {
         _data = value;
      }
      
      [Inspectable(defaultValue="false")]
      public function get autoRepeat() : Boolean
      {
         return this._autoRepeat;
      }
      
      public function set autoRepeat(value:Boolean) : void
      {
         _autoRepeat = value;
      }
      
      [Inspectable(defaultValue="true")]
      override public function get enabled() : Boolean
      {
         return super.enabled;
      }
      
      override public function set enabled(value:Boolean) : void
      {
         var state:String;
         super.enabled = value;
         mouseChildren = false;
         if(super.enabled)
         {
            state = this._focusIndicator == null && (_displayFocus || _focused) ? "over" : "up";
         }
         else
         {
            state = "disabled";
         }
         this.setState(state);
      }
      
      [Inspectable(defaultValue="true")]
      override public function get focusable() : Boolean
      {
         return _focusable;
      }
      
      override public function set focusable(value:Boolean) : void
      {
         super.focusable = value;
      }
      
      [Inspectable(defaultValue="false")]
      public function get toggle() : Boolean
      {
         return this._toggle;
      }
      
      public function set toggle(value:Boolean) : void
      {
         _toggle = value;
      }
      
      public function get owner() : UIComponent
      {
         return this._owner;
      }
      
      public function set owner(value:UIComponent) : void
      {
         _owner = value;
      }
      
      public function get state() : String
      {
         return this._state;
      }
      
      [Inspectable(defaultValue="false")]
      public function get selected() : Boolean
      {
         return this._selected;
      }
      
      public function set selected(value:Boolean) : void
      {
         var df:Boolean;
         if(this._selected == value)
         {
            return;
         }
         _selected = value;
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
               df = this._selected && this.owner != null && this.checkOwnerFocused();
               this.setState(df && this._focusIndicator == null ? "selecting" : "toggle");
               displayFocus = df;
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
      
      public function set group(value:ButtonGroup) : void
      {
         if(this._group != null)
         {
            this._group.removeButton(this);
         }
         _group = value;
         if(this._group != null)
         {
            this._group.addButton(this);
         }
      }
      
      public function get groupName() : String
      {
         return this._groupName;
      }
      
      public function set groupName(value:String) : void
      {
         if(_inspector && value == "")
         {
            return;
         }
         if(this._groupName == value)
         {
            return;
         }
         if(value != null)
         {
            addEventListener(Event.ADDED,this.addToAutoGroup,false,0,true);
            addEventListener(Event.REMOVED,this.addToAutoGroup,false,0,true);
         }
         else
         {
            removeEventListener(Event.ADDED,this.addToAutoGroup,false);
            removeEventListener(Event.REMOVED,this.addToAutoGroup,false);
         }
         _groupName = value;
         this.addToAutoGroup(null);
      }
      
      [Inspectable(defaultValue="")]
      public function get label() : String
      {
         return this._label;
      }
      
      public function set label(value:String) : void
      {
         if(this._label == value)
         {
            return;
         }
         _label = value;
         invalidateData();
      }
      
      [Inspectable(defaultValue="none",enumeration="none,left,right,center")]
      public function get autoSize() : String
      {
         return this._autoSize;
      }
      
      public function set autoSize(value:String) : void
      {
         if(value == this._autoSize)
         {
            return;
         }
         _autoSize = value;
         invalidateData();
      }
      
      public function get focusIndicator() : MovieClip
      {
         return this._focusIndicator;
      }
      
      public function set focusIndicator(value:MovieClip) : void
      {
         _focusIndicatorLabelHash = null;
         _focusIndicator = value;
         _focusIndicatorLabelHash = UIComponent.generateLabelHash(this._focusIndicator);
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.isDefaultPrevented())
         {
            return;
         }
         var details:InputDetails = event.details;
         var index:uint = details.controllerIndex;
         switch(details.navEquivalent)
         {
            case NavigationCode.ENTER:
               if(details.value == InputValue.KEY_DOWN)
               {
                  this.handlePress(index);
                  event.handled = true;
               }
               else if(details.value == InputValue.KEY_UP)
               {
                  if(this._pressedByKeyboard)
                  {
                     this.handleRelease(index);
                     event.handled = true;
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
         super.draw();
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
      
      protected function addToAutoGroup(event:Event) : void
      {
         if(parent == null)
         {
            group = null;
            return;
         }
         var g:ButtonGroup = ButtonGroup.getGroup(this._groupName,parent);
         if(g == this.group)
         {
            return;
         }
         group = g;
      }
      
      protected function checkOwnerFocused() : Boolean
      {
         var ownerFocused:Boolean = false;
         var ownerFocusTarget:Object;
         if(this.owner != null)
         {
            ownerFocused = this._owner.focused != 0;
            if(ownerFocused == 0)
            {
               ownerFocusTarget = this._owner.focusTarget;
               if(ownerFocusTarget != null)
               {
                  ownerFocused = ownerFocusTarget != 0;
               }
            }
         }
         return ownerFocused;
      }
      
      protected function calculateWidth() : Number
      {
         var w:Number = actualWidth;
         var element:ConstrainedElement;
         if(!this.constraintsDisabled)
         {
            element = constraints.getElement("textField");
            w = Math.ceil(this.textField.textWidth + element.left + element.right + 5);
         }
         return w;
      }
      
      protected function alignForAutoSize() : void
      {
         var oldWidth:Number;
         if(!initialized || this._autoSize == TextFieldAutoSize.NONE || this.textField == null)
         {
            return;
         }
         oldWidth = _width;
         var newWidth:Number = _width = this.calculateWidth();
         var oldRight:Number;
         var oldCenter:Number;
         switch(this._autoSize)
         {
            case TextFieldAutoSize.RIGHT:
               oldRight = x + oldWidth;
               x = oldRight - newWidth;
               break;
            case TextFieldAutoSize.CENTER:
               oldCenter = x + oldWidth * 0.5;
               x = oldCenter - newWidth * 0.5;
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
         var focusFrame:String;
         if(!this.enabled)
         {
            return;
         }
         if(this._focusIndicator == null)
         {
            this.setState(_focused || _displayFocus ? "over" : "out");
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
               focusFrame = "state" + _focused;
               if(this._focusIndicatorLabelHash[focusFrame])
               {
                  _newFocusIndicatorFrame = "state" + _focused;
               }
               else
               {
                  _newFocusIndicatorFrame = _focused || _displayFocus ? "show" : "hide";
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
      
      protected function handleMouseRollOver(event:MouseEvent) : void
      {
         var sfEvent:MouseEventEx = event as MouseEventEx;
         var mouseIdx:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
         if(event.buttonDown)
         {
            dispatchEvent(new ButtonEvent(ButtonEvent.DRAG_OVER));
            if(!this.enabled)
            {
               return;
            }
            if(this.lockDragStateChange && Boolean(this._mouseDown << mouseIdx & 1))
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
      
      protected function handleMouseRollOut(event:MouseEvent) : void
      {
         var sfEvent:MouseEventEx = event as MouseEventEx;
         var index:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
         if(event.buttonDown)
         {
            dispatchEvent(new ButtonEvent(ButtonEvent.DRAG_OUT));
            if(Boolean(this._mouseDown & 1 << index))
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
      
      protected function handleMousePress(event:MouseEvent) : void
      {
         var sfEvent:MouseEventEx = event as MouseEventEx;
         var mouseIdx:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
         var btnIdx:uint = sfEvent == null ? 0 : sfEvent.buttonIdx;
         var sfButtonEvent:ButtonEvent;
         if(btnIdx != 0)
         {
            return;
         }
         _mouseDown |= 1 << mouseIdx;
         if(this.enabled)
         {
            this.setState("down");
            if(this.autoRepeat && this._repeatTimer == null)
            {
               _autoRepeatEvent = new ButtonEvent(ButtonEvent.CLICK,true,false,mouseIdx,btnIdx,false,true);
               _repeatTimer = new Timer(this.repeatDelay,1);
               this._repeatTimer.addEventListener(TimerEvent.TIMER_COMPLETE,this.beginRepeat,false,0,true);
               this._repeatTimer.start();
            }
            sfButtonEvent = new ButtonEvent(ButtonEvent.PRESS,true,false,mouseIdx,btnIdx,false,false);
            dispatchEvent(sfButtonEvent);
         }
      }
      
      protected function handleMouseRelease(event:MouseEvent) : void
      {
         _autoRepeatEvent = null;
         if(!this.enabled)
         {
            return;
         }
         var sfEvent:MouseEventEx = event as MouseEventEx;
         var mouseIdx:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
         var btnIdx:uint = sfEvent == null ? 0 : sfEvent.buttonIdx;
         var sfButtonEvent:ButtonEvent;
         if(btnIdx != 0)
         {
            return;
         }
         _mouseDown ^= 1 << mouseIdx;
         if(this._mouseDown == 0 && this._repeatTimer)
         {
            this._repeatTimer.stop();
            this._repeatTimer.reset();
            this._repeatTimer.removeEventListener(TimerEvent.TIMER_COMPLETE,this.beginRepeat);
            this._repeatTimer.removeEventListener(TimerEvent.TIMER,this.handleRepeat);
            _repeatTimer = null;
         }
         this.setState("release");
         this.handleClick(mouseIdx);
         if(!this._isRepeating)
         {
            sfButtonEvent = new ButtonEvent(ButtonEvent.CLICK,true,false,mouseIdx,btnIdx,false,false);
            dispatchEvent(sfButtonEvent);
         }
         _isRepeating = false;
      }
      
      protected function handleReleaseOutside(event:MouseEvent) : void
      {
         _autoRepeatEvent = null;
         if(contains(event.target as DisplayObject))
         {
            return;
         }
         var sfEvent:MouseEventEx = event as MouseEventEx;
         var mouseIdx:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
         var btnIdx:uint = sfEvent == null ? 0 : sfEvent.buttonIdx;
         if(btnIdx != 0)
         {
            return;
         }
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.handleReleaseOutside,false);
         _mouseDown ^= 1 << mouseIdx;
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
      
      protected function handlePress(controllerIndex:uint = 0) : void
      {
         if(!this.enabled)
         {
            return;
         }
         _pressedByKeyboard = true;
         this.setState(this._focusIndicator == null ? "down" : "kb_down");
         if(this.autoRepeat && this._repeatTimer == null)
         {
            _autoRepeatEvent = new ButtonEvent(ButtonEvent.CLICK,true,false,controllerIndex,0,true,true);
            _repeatTimer = new Timer(this.repeatDelay,1);
            this._repeatTimer.addEventListener(TimerEvent.TIMER_COMPLETE,this.beginRepeat,false,0,true);
            this._repeatTimer.start();
         }
         var sfEvent:ButtonEvent = new ButtonEvent(ButtonEvent.PRESS,true,false,controllerIndex,0,true,false);
         dispatchEvent(sfEvent);
      }
      
      protected function handleRelease(controllerIndex:uint = 0) : void
      {
         var sfEvent:ButtonEvent;
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
         this.handleClick(controllerIndex);
         _pressedByKeyboard = false;
         if(!this._isRepeating)
         {
            sfEvent = new ButtonEvent(ButtonEvent.CLICK,true,false,controllerIndex,0,true,false);
            dispatchEvent(sfEvent);
         }
         _isRepeating = false;
      }
      
      protected function handleClick(controllerIndex:uint = 0) : void
      {
         if(this.toggle && (!this.selected || this.allowDeselect))
         {
            selected = !this.selected;
         }
      }
      
      protected function beginRepeat(event:TimerEvent) : void
      {
         this._repeatTimer.delay = this.repeatInterval;
         this._repeatTimer.repeatCount = 0;
         this._repeatTimer.removeEventListener(TimerEvent.TIMER_COMPLETE,this.beginRepeat);
         this._repeatTimer.addEventListener(TimerEvent.TIMER,this.handleRepeat,false,0,true);
         this._repeatTimer.reset();
         this._repeatTimer.start();
      }
      
      protected function handleRepeat(event:TimerEvent) : void
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
      
      protected function setState(state:String) : void
      {
         _state = state;
         var prefixes:Vector.<String> = this.getStatePrefixes();
         var states:Array = this._stateMap[state];
         if(states == null || states.length == 0)
         {
            return;
         }
         var l:uint = prefixes.length;
         var i:uint = 0;
         var prefix:String;
         var sl:uint;
         var j:uint;
         var thisLabel:String;
         while(i < l)
         {
            prefix = prefixes[i];
            sl = states.length;
            j = 0;
            while(j < sl)
            {
               thisLabel = prefix + states[j];
               if(_labelHash[thisLabel])
               {
                  _newFrame = thisLabel;
                  invalidateState();
                  return;
               }
               j++;
            }
            i++;
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

