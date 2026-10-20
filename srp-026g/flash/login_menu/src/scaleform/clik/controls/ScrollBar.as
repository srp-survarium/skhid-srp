package scaleform.clik.controls
{
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.geom.Point;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.constants.ScrollBarTrackMode;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.utils.Constraints;
   
   [Event(name="complete",type="flash.events.Event")]
   public class ScrollBar extends ScrollIndicator
   {
      
      public var trackScrollPageSize:Number = 1;
      
      protected var _dragOffset:Point;
      
      protected var _trackMode:String = "scrollPage";
      
      protected var _trackScrollPosition:Number = -1;
      
      protected var _trackDragMouseIndex:Number = -1;
      
      public var upArrow:Button;
      
      public var downArrow:Button;
      
      public function ScrollBar()
      {
         super();
      }
      
      override protected function initialize() : void
      {
         super.initialize();
         var _loc1_:Number = rotation;
         rotation = 0;
         if(this.downArrow)
         {
            constraints.addElement("downArrow",this.downArrow,Constraints.BOTTOM);
         }
         constraints.addElement("track",track,Constraints.TOP | Constraints.BOTTOM);
         rotation = _loc1_;
      }
      
      override protected function preInitialize() : void
      {
         constraints = new Constraints(this,ConstrainMode.REFLOW);
      }
      
      [Inspectable(defaultValue="true")]
      override public function get enabled() : Boolean
      {
         return super.enabled;
      }
      
      override public function set enabled(param1:Boolean) : void
      {
         if(this.enabled == param1)
         {
            return;
         }
         super.enabled = param1;
         gotoAndPlay(this.enabled ? "default" : "disabled");
         invalidate(InvalidationType.STATE);
      }
      
      override public function get position() : Number
      {
         return _position;
      }
      
      override public function set position(param1:Number) : void
      {
         param1 = Math.round(param1);
         if(param1 == this.position)
         {
            return;
         }
         super.position = param1;
         this.updateScrollTarget();
      }
      
      [Inspectable(type="String",enumeration="scrollPage,scrollToCursor",defaultValue="scrollPage")]
      public function get trackMode() : String
      {
         return this._trackMode;
      }
      
      public function set trackMode(param1:String) : void
      {
         if(param1 == this._trackMode)
         {
            return;
         }
         _trackMode = param1;
         if(initialized)
         {
            track.autoRepeat = this.trackMode == ScrollBarTrackMode.SCROLL_PAGE;
         }
      }
      
      override public function get availableHeight() : Number
      {
         return track.height - thumb.height + offsetBottom + offsetTop;
      }
      
      override public function toString() : String
      {
         return "[CLIK ScrollBar " + name + "]";
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         mouseEnabled = mouseChildren = this.enabled;
         tabEnabled = tabChildren = _focusable;
         addEventListener(MouseEvent.MOUSE_WHEEL,this.handleMouseWheel,false,0,true);
         addEventListener(InputEvent.INPUT,handleInput,false,0,true);
         if(this.upArrow)
         {
            this.upArrow.addEventListener(ButtonEvent.CLICK,this.handleUpArrowClick,false,0,true);
            this.upArrow.addEventListener(ButtonEvent.PRESS,this.handleUpArrowPress,false,0,true);
            this.upArrow.focusTarget = this;
            this.upArrow.autoRepeat = true;
         }
         if(this.downArrow)
         {
            this.downArrow.addEventListener(ButtonEvent.CLICK,this.handleDownArrowClick,false,0,true);
            this.downArrow.addEventListener(ButtonEvent.PRESS,this.handleDownArrowPress,false,0,true);
            this.downArrow.focusTarget = this;
            this.downArrow.autoRepeat = true;
         }
         thumb.addEventListener(MouseEvent.MOUSE_DOWN,this.handleThumbPress,false,0,true);
         thumb.focusTarget = this;
         thumb.lockDragStateChange = true;
         track.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress,false,0,true);
         track.addEventListener(ButtonEvent.CLICK,this.handleTrackClick,false,0,true);
         if(track is UIComponent)
         {
            (track as UIComponent).focusTarget = this;
         }
         track.autoRepeat = this.trackMode == ScrollBarTrackMode.SCROLL_PAGE;
      }
      
      protected function scrollUp() : void
      {
         position -= _pageScrollSize;
      }
      
      protected function scrollDown() : void
      {
         position += _pageScrollSize;
      }
      
      override protected function drawLayout() : void
      {
         var _loc1_:Number;
         thumb.y = track.y - offsetTop;
         if(isHorizontal)
         {
            constraints.update(_height,_width);
         }
         else
         {
            constraints.update(_width,_height);
         }
         if(isHorizontal && actualWidth != width)
         {
            _loc1_ = width / actualWidth;
            scaleY = _loc1_;
         }
      }
      
      override protected function updateThumb() : void
      {
         var _loc1_:Number = Math.max(1,_maxPosition - _minPosition + _pageSize);
         var _loc2_:Number = track.height + offsetTop + offsetBottom;
         if(_pageSize >= _loc1_)
         {
            thumb.height = _minThumbSize;
         }
         else
         {
            thumb.height = Math.max(_minThumbSize,Math.min(_loc2_,_pageSize / _loc1_ * _loc2_));
         }
         if(thumb is UIComponent)
         {
            (thumb as UIComponent).validateNow();
         }
         this.updateThumbPosition();
      }
      
      override protected function updateThumbPosition() : void
      {
         var _loc1_:Number = (_position - _minPosition) / (_maxPosition - _minPosition);
         var _loc2_:Number = track.y - offsetTop;
         var _loc3_:Number = Math.round(_loc1_ * this.availableHeight + _loc2_);
         thumb.y = Math.max(_loc2_,Math.min(track.y + track.height - thumb.height + offsetBottom,_loc3_));
         thumb.visible = !(isNaN(_loc1_) || isNaN(_pageSize) || _maxPosition <= 0 || _maxPosition == Infinity);
         var _loc4_:Boolean = thumb.visible && this.enabled;
         if(this.upArrow)
         {
            this.upArrow.enabled = _loc4_ && _position > _minPosition;
            this.upArrow.validateNow();
         }
         if(this.downArrow)
         {
            this.downArrow.enabled = _loc4_ && _position < _maxPosition;
            this.downArrow.validateNow();
         }
         track.enabled = track.mouseEnabled = _loc4_;
      }
      
      protected function handleUpArrowClick(param1:ButtonEvent) : void
      {
         if(param1.isRepeat)
         {
            this.scrollUp();
         }
      }
      
      protected function handleUpArrowPress(param1:ButtonEvent) : void
      {
         this.scrollUp();
      }
      
      protected function handleDownArrowClick(param1:ButtonEvent) : void
      {
         if(param1.isRepeat)
         {
            this.scrollDown();
         }
      }
      
      protected function handleDownArrowPress(param1:ButtonEvent) : void
      {
         this.scrollDown();
      }
      
      public function get isDragging() : Boolean
      {
         return _isDragging;
      }
      
      public function resetDragOffset() : void
      {
         _dragOffset = new Point(0,mouseY - thumb.y);
      }
      
      protected function handleThumbPress(param1:Event) : void
      {
         if(_isDragging)
         {
            return;
         }
         _isDragging = true;
         stage.addEventListener(MouseEvent.MOUSE_MOVE,this.doDrag,false,0,true);
         stage.addEventListener(MouseEvent.MOUSE_UP,this.endDrag,false,0,true);
         _dragOffset = new Point(0,mouseY - thumb.y);
         dispatchEvent(new Event(Event.SELECT));
      }
      
      protected function doDrag(param1:MouseEvent) : void
      {
         var _loc2_:Number = (mouseY - this._dragOffset.y - track.y) / this.availableHeight;
         position = _minPosition + _loc2_ * (_maxPosition - _minPosition);
      }
      
      protected function endDrag(param1:MouseEvent) : void
      {
         stage.removeEventListener(MouseEvent.MOUSE_MOVE,this.doDrag);
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.endDrag);
         dispatchEvent(new Event(Event.COMPLETE));
         _isDragging = false;
      }
      
      protected function handleTrackPress(param1:MouseEvent) : void
      {
         var _loc2_:Number;
         if(param1.shiftKey || this.trackMode == ScrollBarTrackMode.SCROLL_TO_CURSOR)
         {
            _loc2_ = (mouseY - thumb.height / 2 - track.y) / this.availableHeight;
            position = Math.round(_loc2_ * (_maxPosition - _minPosition) + _minPosition);
            thumb.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_OVER));
            thumb.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN));
            this.handleThumbPress(param1);
            _dragOffset = new Point(0,thumb.height / 2);
         }
         if(_isDragging || this.position == this._trackScrollPosition)
         {
            return;
         }
         if(mouseY > thumb.y && mouseY < thumb.y + thumb.height)
         {
            return;
         }
         position += thumb.y < mouseY ? this.trackScrollPageSize : -this.trackScrollPageSize;
      }
      
      protected function handleTrackClick(param1:ButtonEvent) : void
      {
         if(param1.isRepeat)
         {
            if(_isDragging || this.position == this._trackScrollPosition)
            {
               return;
            }
            if(mouseY > thumb.y && mouseY < thumb.y + thumb.height)
            {
               return;
            }
            position += thumb.y < mouseY ? this.trackScrollPageSize : -this.trackScrollPageSize;
         }
      }
      
      protected function updateScrollTarget() : void
      {
         if(_scrollTarget == null || !this.enabled)
         {
            return;
         }
         _scrollTarget.scrollV = _position;
      }
      
      protected function handleMouseWheel(param1:MouseEvent) : void
      {
         position -= (param1.delta > 0 ? 1 : -1) * _pageScrollSize;
      }
      
      override protected function changeFocus() : void
      {
         thumb.displayFocus = _focused || _displayFocus;
      }
   }
}

