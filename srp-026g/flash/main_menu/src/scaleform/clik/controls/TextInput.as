package scaleform.clik.controls
{
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.MouseEvent;
   import flash.text.TextField;
   import flash.text.TextFieldType;
   import flash.text.TextFormat;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ComponentEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.managers.FocusHandler;
   import scaleform.clik.ui.InputDetails;
   import scaleform.clik.utils.Constraints;
   import scaleform.gfx.Extensions;
   import scaleform.gfx.FocusManager;
   import scaleform.gfx.MouseEventEx;
   
   [Event(name="stateChange",type="scaleform.clik.events.ComponentEvent")]
   public class TextInput extends UIComponent
   {
      
      public var defaultTextFormat:TextFormat;
      
      public var constraintsDisabled:Boolean = false;
      
      protected var _text:String = "";
      
      protected var _displayAsPassword:Boolean = false;
      
      protected var _maxChars:uint = 0;
      
      protected var _editable:Boolean = true;
      
      protected var _selectable:Boolean = true;
      
      protected var _actAsButton:Boolean = false;
      
      protected var _alwaysShowSelection:Boolean = false;
      
      protected var _isHtml:Boolean = false;
      
      protected var _state:String = "default";
      
      protected var _newFrame:String;
      
      protected var _textFormat:TextFormat;
      
      protected var _usingDefaultTextFormat:Boolean = true;
      
      protected var _defaultText:String = "";
      
      private var hscroll:Number = 0;
      
      public var textField:TextField;
      
      public function TextInput()
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
         super.tabEnabled = false;
         mouseEnabled = mouseChildren = this.enabled;
         super.initialize();
         _textFormat = this.textField.getTextFormat();
         defaultTextFormat = new TextFormat();
         this.defaultTextFormat.italic = true;
         this.defaultTextFormat.color = 11184810;
      }
      
      [Inspectable(defaultValue="true")]
      override public function get enabled() : Boolean
      {
         return super.enabled;
      }
      
      override public function set enabled(value:Boolean) : void
      {
         super.enabled = value;
         mouseChildren = value;
         super.tabEnabled = false;
         tabChildren = _focusable;
         this.setState(this.defaultState);
      }
      
      [Inspectable(defaultValue="true")]
      override public function get focusable() : Boolean
      {
         return _focusable;
      }
      
      override public function set focusable(value:Boolean) : void
      {
         _focusable = value;
         if(!_focusable && this.enabled)
         {
            tabChildren = false;
         }
         this.changeFocus();
         if(_focusable && this.editable)
         {
            addEventListener(MouseEvent.MOUSE_DOWN,this.handleMouseDown,false,0,true);
         }
         else
         {
            removeEventListener(MouseEvent.MOUSE_DOWN,this.handleMouseDown,false);
         }
      }
      
      public function set textSize(value:Number) : void
      {
         this._textFormat.size = value;
         this.defaultTextFormat.size = value;
         if(_focused || this._text != "" || this._defaultText == "")
         {
            this.textField.defaultTextFormat = this._textFormat;
            this.textField.setTextFormat(this._textFormat);
         }
         else
         {
            this.textField.defaultTextFormat = this.defaultTextFormat;
            this.textField.setTextFormat(this.defaultTextFormat);
         }
      }
      
      [Inspectable(type="String",defaultValue="")]
      public function get text() : String
      {
         return this._text;
      }
      
      public function set text(value:String) : void
      {
         _isHtml = false;
         _text = value;
         invalidateData();
      }
      
      public function get htmlText() : String
      {
         return this._text;
      }
      
      public function set htmlText(value:String) : void
      {
         _isHtml = true;
         _text = value;
         invalidateData();
      }
      
      [Inspectable(verbose="1",type="String",defaultValue="")]
      public function get defaultText() : String
      {
         return this._defaultText;
      }
      
      public function set defaultText(value:String) : void
      {
         _defaultText = value;
         invalidateData();
      }
      
      [Inspectable(defaultValue="false")]
      public function get displayAsPassword() : Boolean
      {
         return this._displayAsPassword;
      }
      
      public function set displayAsPassword(value:Boolean) : void
      {
         _displayAsPassword = value;
         if(this.textField != null)
         {
            this.textField.displayAsPassword = value;
         }
      }
      
      [Inspectable(defaultValue="0")]
      public function get maxChars() : uint
      {
         return this._maxChars;
      }
      
      public function set maxChars(value:uint) : void
      {
         _maxChars = value;
         if(this.textField != null)
         {
            this.textField.maxChars = value;
         }
      }
      
      [Inspectable(defaultValue="false")]
      public function get selectable() : Boolean
      {
         return this._editable;
      }
      
      public function set selectable(value:Boolean) : void
      {
         _selectable = value;
         if(this.textField != null)
         {
            this.textField.selectable = this._selectable;
         }
         focusable = value;
      }
      
      [Inspectable(defaultValue="true")]
      public function get editable() : Boolean
      {
         return this._editable;
      }
      
      public function set editable(value:Boolean) : void
      {
         _editable = value;
         if(this.textField != null)
         {
            this.textField.type = this._editable && this.enabled ? TextFieldType.INPUT : TextFieldType.DYNAMIC;
         }
         focusable = value;
      }
      
      override public function get tabEnabled() : Boolean
      {
         return this.textField.tabEnabled;
      }
      
      override public function set tabEnabled(value:Boolean) : void
      {
         this.textField.tabEnabled = value;
      }
      
      override public function get tabIndex() : int
      {
         return this.textField.tabIndex;
      }
      
      override public function set tabIndex(value:int) : void
      {
         this.textField.tabIndex = value;
      }
      
      [Inspectable(defaultValue="false")]
      public function get actAsButton() : Boolean
      {
         return this._actAsButton;
      }
      
      public function set actAsButton(value:Boolean) : void
      {
         if(this._actAsButton == value)
         {
            return;
         }
         _actAsButton = value;
         if(value)
         {
            addEventListener(MouseEvent.ROLL_OVER,this.handleRollOver,false,0,true);
            addEventListener(MouseEvent.ROLL_OUT,this.handleRollOut,false,0,true);
         }
         else
         {
            removeEventListener(MouseEvent.ROLL_OVER,this.handleRollOver,false);
            removeEventListener(MouseEvent.ROLL_OUT,this.handleRollOut,false);
         }
      }
      
      public function get alwaysShowSelection() : Boolean
      {
         return this._alwaysShowSelection;
      }
      
      public function set alwaysShowSelection(value:Boolean) : void
      {
         _alwaysShowSelection = value;
         if(this.textField != null)
         {
            this.textField.alwaysShowSelection = value;
         }
      }
      
      public function get length() : uint
      {
         return this.textField.length;
      }
      
      public function get defaultState() : String
      {
         return !this.enabled ? "disabled" : (focused ? "focused" : "default");
      }
      
      public function getTextFormat() : TextFormat
      {
         return this._textFormat;
      }
      
      public function appendText(value:String) : void
      {
         _text += value;
         _isHtml = false;
         invalidateData();
      }
      
      public function appendHtml(value:String) : void
      {
         _text += value;
         _isHtml = true;
         invalidateData();
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         var details:InputDetails;
         var navEquivalent:String;
         var nexSpacePos:int;
         var nextSpaceLine:int;
         var caretLine:int;
         var nexCaretPos:int;
         if(event.handled)
         {
            return;
         }
         details = event.details;
         navEquivalent = event.details.navEquivalent;
         if((navEquivalent == NavigationCode.RIGHT || navEquivalent == NavigationCode.LEFT) && details.ctrlKey && details.value == InputValue.KEY_DOWN)
         {
            if(navEquivalent == NavigationCode.RIGHT)
            {
               nexSpacePos = this.textField.text.indexOf(" ",this.textField.caretIndex);
               if(nexSpacePos == -1)
               {
                  nexSpacePos = this.textField.length;
               }
               caretLine = this.textField.getLineIndexOfChar(this.textField.caretIndex);
               nextSpaceLine = this.textField.getLineIndexOfChar(nexSpacePos);
               if(caretLine < nextSpaceLine)
               {
                  nexCaretPos = this.textField.getLineOffset(caretLine) + this.textField.getLineLength(caretLine) - 2;
                  if(this.textField.caretIndex != nexCaretPos + 1)
                  {
                     nexSpacePos = nexCaretPos;
                  }
                  else
                  {
                     nexSpacePos = this.textField.getLineOffset(caretLine + 1) - 1;
                  }
               }
            }
            else
            {
               nexSpacePos = this.textField.caretIndex;
               while(nexSpacePos > 0)
               {
                  if(this.textField.text.charAt(nexSpacePos - 2) == " ")
                  {
                     break;
                  }
                  nexSpacePos--;
               }
               caretLine = this.textField.getLineIndexOfChar(this.textField.caretIndex);
               nextSpaceLine = this.textField.getLineIndexOfChar(nexSpacePos);
               if(caretLine > nextSpaceLine)
               {
                  nexCaretPos = this.textField.getLineOffset(caretLine) + 1;
                  if(this.textField.caretIndex != nexCaretPos - 1)
                  {
                     nexSpacePos = nexCaretPos;
                  }
                  else
                  {
                     nexSpacePos = this.textField.getLineOffset(caretLine);
                  }
               }
            }
            if(nexSpacePos != -1)
            {
               this.textField.setSelection(nexSpacePos,nexSpacePos);
               event.handled = true;
            }
         }
         if(navEquivalent == NavigationCode.RIGHT && this.textField.caretIndex == this.textField.selectionEndIndex && this.textField.selectionEndIndex != this.textField.selectionBeginIndex)
         {
            event.handled = true;
         }
         trace("INPUTT:hold",details.value,details.value == InputValue.KEY_HOLD);
         if(details.value == InputValue.KEY_DOWN || details.value == InputValue.KEY_HOLD)
         {
            return;
         }
      }
      
      override public function toString() : String
      {
         return "[CLIK TextInput " + name + "]";
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(!this.constraintsDisabled)
         {
            constraints.addElement("textField",this.textField,Constraints.ALL);
         }
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         this.textField.addEventListener(FocusEvent.FOCUS_IN,this.handleTextFieldFocusIn,false,0,true);
         if(this.focusable && this.editable)
         {
            addEventListener(MouseEvent.MOUSE_DOWN,this.handleMouseDown,false,0,true);
         }
         this.setState(this.defaultState,"default");
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
            this.updateTextField();
            dispatchEvent(new ComponentEvent(ComponentEvent.STATE_CHANGE));
            invalidate(InvalidationType.SIZE);
         }
         else if(isInvalid(InvalidationType.DATA))
         {
            this.updateText();
         }
         if(isInvalid(InvalidationType.SIZE))
         {
            setActualSize(_width,_height);
            if(!this.constraintsDisabled)
            {
               constraints.update(_width,_height);
            }
         }
      }
      
      override protected function changeFocus() : void
      {
         this.setState(this.defaultState);
      }
      
      protected function updateTextField() : void
      {
         if(this.textField == null)
         {
            trace(">>> Error :: " + this + ", textField is NULL.");
            return;
         }
         this.updateText();
         this.textField.maxChars = this._maxChars;
         this.textField.alwaysShowSelection = this._alwaysShowSelection;
         this.textField.selectable = this.enabled ? this._editable || this._selectable : this.enabled;
         this.textField.type = this._editable && this.enabled ? TextFieldType.INPUT : TextFieldType.DYNAMIC;
         this.textField.tabEnabled = this._editable && this.enabled && _focusable;
         this.textField.addEventListener(Event.CHANGE,this.handleTextChange,false,0,true);
         if(this.textField.hasEventListener(FocusEvent.FOCUS_IN))
         {
            this.textField.removeEventListener(FocusEvent.FOCUS_IN,this.handleTextFieldFocusIn,false);
         }
         this.textField.addEventListener(FocusEvent.FOCUS_IN,this.handleTextFieldFocusIn,false,0,true);
      }
      
      protected function handleTextFieldFocusIn(e:FocusEvent) : void
      {
         FocusHandler.getInstance().setFocus(this);
      }
      
      protected function updateText() : void
      {
         if(_focused && this._usingDefaultTextFormat)
         {
            this.textField.defaultTextFormat = this._textFormat;
            _usingDefaultTextFormat = false;
            if(this._displayAsPassword && !this.textField.displayAsPassword)
            {
               this.textField.displayAsPassword = true;
            }
         }
         if(this._text != "")
         {
            if(this._isHtml)
            {
               this.textField.htmlText = this._text;
            }
            else
            {
               this.textField.text = this._text;
            }
         }
         else
         {
            this.textField.text = "";
            if(!_focused && this._defaultText != "")
            {
               if(this._displayAsPassword)
               {
                  this.textField.displayAsPassword = false;
               }
               this.textField.text = this._defaultText;
               _usingDefaultTextFormat = true;
               if(this.defaultTextFormat != null)
               {
                  this.textField.setTextFormat(this.defaultTextFormat);
               }
            }
         }
      }
      
      protected function setState(... rest) : void
      {
         if(rest.length == 1)
         {
            newState = rest[0].toString();
            if(this._state != newState && _labelHash[newState])
            {
               _state = _newFrame = newState;
               invalidateState();
            }
            return;
         }
         var count:uint = (rest.length);
         var i:uint = 0;
         var newState:String;
         var frameLabel:String;
         while(i < count)
         {
            frameLabel = rest[i].toString();
            if(_labelHash[frameLabel])
            {
               _state = _newFrame = frameLabel;
               invalidateState();
               break;
            }
            i++;
         }
      }
      
      protected function updateAfterStateChange() : void
      {
         var numControllers:uint;
         var i:uint;
         if(!initialized)
         {
            return;
         }
         constraints.updateElement("textField",this.textField);
         if(_focused)
         {
            if(Extensions.isScaleform)
            {
               numControllers = Extensions.numControllers;
               i = 0;
               while(i < numControllers)
               {
                  if(FocusManager.getFocus(i) == this)
                  {
                     FocusManager.setFocus(this.textField,i);
                  }
                  i++;
               }
            }
            else
            {
               stage.focus = this.textField;
            }
         }
      }
      
      protected function handleRollOver(event:MouseEvent) : void
      {
         if(focused || !this.enabled)
         {
            return;
         }
         this.setState("over");
      }
      
      protected function handleRollOut(event:MouseEvent) : void
      {
         if(focused || !this.enabled)
         {
            return;
         }
         this.setState("out","default");
      }
      
      protected function handleMouseDown(event:MouseEvent) : void
      {
         if(focused || !this.enabled)
         {
            return;
         }
         if(event is MouseEventEx)
         {
            FocusManager.setFocus(this.textField,(event as MouseEventEx).mouseIdx);
         }
         else
         {
            stage.focus = this.textField;
         }
      }
      
      protected function handleTextChange(event:Event) : void
      {
         _text = this._isHtml ? this.textField.htmlText : this.textField.text;
         dispatchEvent(new Event(Event.CHANGE));
      }
   }
}

