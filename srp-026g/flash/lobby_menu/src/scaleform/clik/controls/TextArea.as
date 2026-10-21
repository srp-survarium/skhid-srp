package scaleform.clik.controls
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.events.ComponentEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.interfaces.IScrollBar;
   import scaleform.clik.utils.ConstrainedElement;
   import scaleform.clik.utils.Constraints;
   import scaleform.gfx.Extensions;
   
   public class TextArea extends TextInput
   {
      
      protected var _scrollPolicy:String = "auto";
      
      protected var _position:int = 1;
      
      protected var _maxScroll:Number = 1;
      
      protected var _resetScrollPosition:Boolean = false;
      
      protected var _scrollBarValue:Object;
      
      protected var _autoScrollBar:Boolean = false;
      
      protected var _thumbOffset:Object = {
         top:0,
         bottom:0
      };
      
      protected var _minThumbSize:uint = 1;
      
      protected var _scrollBar:IScrollBar;
      
      public var container:Sprite;
      
      public function TextArea()
      {
         super();
      }
      
      override protected function preInitialize() : void
      {
         if(!constraintsDisabled)
         {
            constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         }
      }
      
      override protected function initialize() : void
      {
         super.initialize();
         if(this.container == null)
         {
            container = new Sprite();
            addChild(this.container);
         }
      }
      
      [Inspectable(defaultValue="true")]
      override public function get enabled() : Boolean
      {
         return super.enabled;
      }
      
      override public function set enabled(value:Boolean) : void
      {
         super.enabled = value;
         this.updateScrollBar();
      }
      
      public function get position() : int
      {
         return this._position;
      }
      
      public function set position(value:int) : void
      {
         _position = value;
         textField.scrollV = this._position;
      }
      
      [Inspectable(type="String")]
      public function get scrollBar() : Object
      {
         return this._scrollBar;
      }
      
      public function set scrollBar(value:Object) : void
      {
         _scrollBarValue = value;
         invalidate(InvalidationType.SCROLL_BAR);
      }
      
      [Inspectable(defaultValue="1")]
      public function get minThumbSize() : uint
      {
         return this._minThumbSize;
      }
      
      public function set minThumbSize(value:uint) : void
      {
         _minThumbSize = value;
         if(!this._autoScrollBar)
         {
            return;
         }
         var indicator:ScrollIndicator = this._scrollBar as ScrollIndicator;
         indicator.minThumbSize = value;
      }
      
      [Inspectable(name="thumbOffset",defaultValue="top:0,bottom:0")]
      public function get thumbOffset() : Object
      {
         return this._thumbOffset;
      }
      
      public function set thumbOffset(value:Object) : void
      {
         _thumbOffset = value;
         if(!this._autoScrollBar)
         {
            return;
         }
         var indicator:ScrollIndicator = this._scrollBar as ScrollIndicator;
         indicator.offsetTop = this._thumbOffset.top;
         indicator.offsetBottom = this._thumbOffset.bottom;
      }
      
      public function get availableWidth() : Number
      {
         return Math.round(_width) - (this._autoScrollBar && (this._scrollBar as MovieClip).visible ? Math.round(this._scrollBar.width) : 0);
      }
      
      public function get availableHeight() : Number
      {
         return Math.round(_height);
      }
      
      override public function toString() : String
      {
         return "[CLIK TextArea " + name + "]";
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         var navCode:String;
         var pageLines:Number;
         var pageLinesDown:Number;
         super.handleInput(event);
         if(event.handled)
         {
            return;
         }
         if(_editable)
         {
            return;
         }
         navCode = event.details.navEquivalent;
         if(event.details.value == InputValue.KEY_DOWN)
         {
            return;
         }
         switch(navCode)
         {
            case NavigationCode.UP:
               if(this.position == 1)
               {
                  return;
               }
               position = Math.max(1,this.position - 1);
               event.handled = true;
               break;
            case NavigationCode.DOWN:
               if(this.position == this._maxScroll)
               {
                  return;
               }
               position = Math.min(this._maxScroll,this.position + 1);
               event.handled = true;
               break;
            case NavigationCode.END:
               position = this._maxScroll;
               event.handled = true;
               break;
            case NavigationCode.HOME:
               position = 1;
               event.handled = true;
               break;
            case NavigationCode.PAGE_UP:
               pageLines = textField.bottomScrollV - textField.scrollV;
               position = Math.max(1,this.position - pageLines);
               event.handled = true;
               break;
            case NavigationCode.PAGE_DOWN:
               pageLinesDown = textField.bottomScrollV - textField.scrollV;
               position = Math.min(this._maxScroll,this.position + pageLinesDown);
               event.handled = true;
            break;
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(textField != null)
         {
            textField.addEventListener(Event.SCROLL,this.onScroller,false,0,true);
         }
      }
      
      override protected function draw() : void
      {
         var textWidth:uint;
         if(isInvalid(InvalidationType.SCROLL_BAR))
         {
            this.createScrollBar();
         }
         if(isInvalid(InvalidationType.STATE))
         {
            if(_newFrame)
            {
               gotoAndPlay(_newFrame);
               _newFrame = null;
            }
            updateAfterStateChange();
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
            removeChild(this.container);
            setActualSize(_width,_height);
            this.container.scaleX = 1 / scaleX;
            this.container.scaleY = 1 / scaleY;
            if(!constraintsDisabled)
            {
               constraints.update(this.availableWidth,_height);
               if(!Extensions.enabled)
               {
                  textWidth = textField.textWidth;
               }
            }
            addChild(this.container);
            if(this._autoScrollBar)
            {
               this.drawScrollBar();
            }
         }
      }
      
      protected function createScrollBar() : void
      {
         var sb:IScrollBar;
         var classRef:Class;
         var sbObject:Object;
         if(this._scrollBar != null)
         {
            this._scrollBar.removeEventListener(Event.SCROLL,this.handleScroll,false);
            this._scrollBar.removeEventListener(Event.CHANGE,this.handleScroll,false);
            this._scrollBar.focusTarget = null;
            if(this.container.contains(this._scrollBar as DisplayObject))
            {
               this.container.removeChild(this._scrollBar as DisplayObject);
            }
            _scrollBar = null;
         }
         if(!this._scrollBarValue || this._scrollBarValue == "")
         {
            return;
         }
         _autoScrollBar = false;
         if(this._scrollBarValue is String)
         {
            if(parent != null)
            {
               sb = parent.getChildByName(this._scrollBarValue.toString()) as IScrollBar;
            }
            if(sb == null)
            {
               classRef = getDefinitionByName(this._scrollBarValue.toString()) as Class;
               if(classRef)
               {
                  sb = new classRef() as IScrollBar;
               }
               if(sb)
               {
                  _autoScrollBar = true;
                  sbObject = sb as Object;
                  if(sbObject && this._thumbOffset)
                  {
                     sbObject.offsetTop = this._thumbOffset.top;
                     sbObject.offsetBottom = this._thumbOffset.bottom;
                  }
                  sb.addEventListener(MouseEvent.MOUSE_WHEEL,this.blockMouseWheel,false,0,true);
                  (sb as Object).minThumbSize = this._minThumbSize;
                  this.container.addChild(sb as DisplayObject);
               }
            }
         }
         else if(this._scrollBarValue is Class)
         {
            sb = new (this._scrollBarValue as Class)() as IScrollBar;
            sb.addEventListener(MouseEvent.MOUSE_WHEEL,this.blockMouseWheel,false,0,true);
            if(sb != null)
            {
               _autoScrollBar = true;
               (sb as Object).offsetTop = this._thumbOffset.top;
               (sb as Object).offsetBottom = this._thumbOffset.bottom;
               (sb as Object).minThumbSize = this._minThumbSize;
               this.container.addChild(sb as DisplayObject);
            }
         }
         else
         {
            sb = this._scrollBarValue as IScrollBar;
         }
         _scrollBar = sb;
         invalidateSize();
         if(this._scrollBar != null)
         {
            this._scrollBar.addEventListener(Event.SCROLL,this.handleScroll,false,0,true);
            this._scrollBar.addEventListener(Event.CHANGE,this.handleScroll,false,0,true);
            this._scrollBar.focusTarget = this;
            (this._scrollBar as Object).scrollTarget = textField;
            this._scrollBar.tabEnabled = false;
         }
      }
      
      protected function drawScrollBar() : void
      {
         if(!this._autoScrollBar)
         {
            return;
         }
         this._scrollBar.x = _width - this._scrollBar.width;
         this._scrollBar.height = this.availableHeight;
         this._scrollBar.validateNow();
      }
      
      protected function updateScrollBar() : void
      {
         _maxScroll = textField.maxScrollV;
         var indicator:ScrollIndicator = this._scrollBar as ScrollIndicator;
         if(indicator == null)
         {
            return;
         }
         var textElement:ConstrainedElement = constraints.getElement("textField");
         if(this._scrollPolicy == "on" || this._scrollPolicy == "auto" && textField.maxScrollV > 1)
         {
            if(this._autoScrollBar && !indicator.visible)
            {
               if(textElement != null)
               {
                  constraints.update(_width,_height);
                  invalidate();
               }
               _maxScroll = textField.maxScrollV;
            }
            indicator.visible = true;
         }
         if(this._scrollPolicy == "off" || this._scrollPolicy == "auto" && textField.maxScrollV == 1)
         {
            if(this._autoScrollBar && indicator.visible)
            {
               indicator.visible = false;
               if(textElement != null)
               {
                  constraints.update(this.availableWidth,_height);
                  invalidate();
               }
            }
         }
         textField.dispatchEvent(new Event(Event.SCROLL));
         if(indicator.enabled != this.enabled)
         {
            indicator.enabled = this.enabled;
         }
      }
      
      override protected function updateText() : void
      {
         super.updateText();
         this.updateScrollBar();
      }
      
      override protected function updateTextField() : void
      {
         _resetScrollPosition = true;
         super.updateTextField();
      }
      
      protected function handleScroll(event:Event) : void
      {
         position = this._scrollBar.position;
      }
      
      protected function blockMouseWheel(event:MouseEvent) : void
      {
         event.stopPropagation();
      }
      
      override protected function handleTextChange(event:Event) : void
      {
         if(this._maxScroll != textField.maxScrollV)
         {
            this.updateScrollBar();
         }
         super.handleTextChange(event);
      }
      
      protected function onScroller(event:Event) : void
      {
         if(this._resetScrollPosition)
         {
            textField.scrollV = this._position;
         }
         else
         {
            _position = textField.scrollV;
         }
         _resetScrollPosition = false;
      }
   }
}

