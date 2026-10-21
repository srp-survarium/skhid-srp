package scaleform.clik.controls
{
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ComponentEvent;
   import scaleform.clik.utils.ConstrainedElement;
   import scaleform.clik.utils.Constraints;
   
   [Event(name="change",type="flash.events.Event")]
   public class Label extends UIComponent
   {
      
      public var constraintsDisabled:Boolean = false;
      
      protected var _text:String;
      
      protected var _autoSize:String = "none";
      
      protected var isHtml:Boolean;
      
      protected var state:String = "default";
      
      protected var _newFrame:String;
      
      public var textField:TextField;
      
      public function Label()
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
      
      [Inspectable(defaultValue="true")]
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
         mouseEnabled = mouseChildren = value;
         this.setState(this.defaultState);
      }
      
      [Inspectable(name="text")]
      public function get text() : String
      {
         return this._text;
      }
      
      public function set text(value:String) : void
      {
         if(value == null)
         {
            value == "";
         }
         isHtml = false;
         _text = value;
         invalidateData();
      }
      
      public function get htmlText() : String
      {
         return this._text;
      }
      
      public function set htmlText(value:String) : void
      {
         if(value == null)
         {
            value == "";
         }
         isHtml = true;
         _text = value;
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
      
      public function get length() : uint
      {
         return this.textField.length;
      }
      
      public function get defaultState() : String
      {
         return !this.enabled ? "disabled" : (focused ? "focused" : "default");
      }
      
      public function appendText(value:String) : void
      {
         _text += value;
         isHtml = false;
         invalidateData();
      }
      
      public function appendHtml(value:String) : void
      {
         _text += value;
         isHtml = true;
         invalidateData();
      }
      
      override public function toString() : String
      {
         return "[CLIK Label " + name + "]";
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(!this.constraintsDisabled)
         {
            constraints.addElement("textField",this.textField,Constraints.ALL);
         }
         focusable = false;
      }
      
      protected function calculateWidth() : Number
      {
         if(constraints == null || this.textField == null)
         {
            return actualWidth;
         }
         if(!this.constraintsDisabled)
         {
            element = constraints.getElement("textField");
         }
         var w:Number = Math.ceil(this.textField.textWidth + element.left + element.right + 5);
         return w;
         var element:ConstrainedElement;
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
            invalidate(InvalidationType.SIZE);
         }
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateText();
            if(this.autoSize != TextFieldAutoSize.NONE)
            {
               this.alignForAutoSize();
               invalidateSize();
            }
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
      
      protected function updateText() : void
      {
         if(this._text != null && this.textField != null)
         {
            if(this.isHtml)
            {
               this.textField.htmlText = this._text;
            }
            else
            {
               this.textField.text = this._text;
            }
         }
      }
      
      protected function updateAfterStateChange() : void
      {
         if(!initialized)
         {
            return;
         }
         if(constraints != null && !this.constraintsDisabled)
         {
            constraints.updateElement("textField",this.textField);
         }
         this.updateText();
         dispatchEvent(new ComponentEvent(ComponentEvent.STATE_CHANGE));
      }
      
      protected function setState(... rest) : void
      {
         if(rest.length == 1)
         {
            frameLabel = rest[0].toString();
            if(this.state != frameLabel && _labelHash[frameLabel])
            {
               state = _newFrame = frameLabel;
               invalidateState();
            }
            return;
         }
         var count:uint = (rest.length);
         var i:uint = 0;
         var frameLabel:String;
         var candidate:String;
         while(i < count)
         {
            candidate = rest[i].toString();
            if(_labelHash[candidate])
            {
               state = _newFrame = candidate;
               invalidateState();
               break;
            }
            i++;
         }
      }
   }
}

