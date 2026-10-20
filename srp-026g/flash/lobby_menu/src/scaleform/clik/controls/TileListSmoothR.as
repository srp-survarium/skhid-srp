package scaleform.clik.controls
{
   import fl.transitions.easing.Strong;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import scaleform.clik.constants.DirectionMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.interfaces.IDataProvider;
   import scaleform.clik.interfaces.IListItemRenderer;
   import scaleform.clik.motion.Tween;
   
   public class TileListSmoothR extends TileList
   {
      
      protected var _pow:int = 20;
      
      protected var _delta:int = 0;
      
      protected var _inDrag:Boolean = false;
      
      protected var _softScroll:Boolean = true;
      
      protected var _scrollTarget:int = 0;
      
      protected var _scrollTween:Boolean = false;
      
      protected var _scrollYPadding:int = 0;
      
      protected var _lastDelta:int = 0;
      
      protected var _toMove:int = 0;
      
      protected var _tween:Tween;
      
      protected var _draggable:Boolean = false;
      
      protected var _dragX:Number = 0;
      
      protected var _dragY:Number = 0;
      
      protected var _mask:Sprite = new Sprite();
      
      protected var extraRow:int = 0;
      
      public function TileListSmoothR()
      {
         super();
      }
      
      override public function set dataProvider(param1:IDataProvider) : void
      {
         super.dataProvider = param1;
         if(this._scrollTween && this._tween)
         {
            this._tween.reset();
            this._tween.paused = true;
            _scrollTween = false;
            scrollPosition = this._scrollTarget;
         }
      }
      
      override public function get scrollPosition() : Number
      {
         return _scrollPosition;
      }
      
      override public function set scrollPosition(param1:Number) : void
      {
         var _loc2_:Boolean = false;
         _delta = param1 % this._pow;
         var _loc3_:Number = _direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns;
         var _loc4_:Number = Math.ceil(_dataProvider.length / _loc3_);
         var _loc5_:Number = _loc4_ - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows - 1 - this.extraRow);
         if(param1 == _loc5_ * this._pow && param1 != 0)
         {
            _delta = this._pow;
         }
         var _loc6_:int = int(param1 / this._pow);
         if(param1 != _loc5_ * this._pow && this._lastDelta != _loc5_)
         {
            if(_loc6_ - this._lastDelta >= 1)
            {
               _loc9_ = 0;
               while(_loc9_ < this._toMove)
               {
                  _loc10_ = this.getRendererAt(0);
                  _renderers.shift();
                  _renderers.push(_loc10_);
                  _loc9_++;
               }
            }
            else if(_loc6_ - this._lastDelta <= -1)
            {
               _loc9_ = 0;
               while(_loc9_ < this._toMove)
               {
                  _loc10_ = this.getRendererAt(_renderers.length - 1);
                  _renderers.unshift(_loc10_);
                  _renderers.pop();
                  _loc9_++;
               }
            }
         }
         _lastDelta = _loc6_;
         var _loc7_:Number = _scrollPosition;
         var _loc8_:int = (_renderers.length);
         var _loc9_:int;
         var _loc10_:IListItemRenderer;
         _loc9_ = 0;
         while(_loc9_ < _renderers.length)
         {
            _loc10_ = this.getRendererAt(_loc9_);
            if(_direction == DirectionMode.VERTICAL)
            {
               _loc10_.y = (_loc9_ / _totalColumns >> 0) * rowHeight + margin - _rowHeight / this._pow * this._delta;
            }
            else
            {
               _loc10_.x = (_loc9_ / _totalRows >> 0) * columnWidth + margin - _columnWidth / this._pow * this._delta;
            }
            _loc9_++;
         }
         param1 = Math.max(0,Math.min(_loc5_,int(param1 / this._pow)));
         if(_scrollPosition == param1)
         {
            return;
         }
         _scrollPosition = param1;
         if(!this._scrollTween)
         {
            _scrollTarget = _scrollPosition * this._pow;
         }
         invalidateData();
      }
      
      override protected function refreshData() : void
      {
         var _loc1_:Number = _direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns;
         var _loc2_:Number = Math.ceil(_dataProvider.length / _loc1_);
         var _loc3_:Number = _loc2_ - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows - this.extraRow);
         _scrollPosition = Math.max(0,Math.min(_loc3_,_scrollPosition));
         var _loc4_:Number = _scrollPosition * _loc1_;
         var _loc5_:Number = _loc4_ + _totalColumns * _totalRows - 1;
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
         this.updateSelectedIndex();
         _dataProvider.requestItemRange(_loc4_,_loc5_,populateData);
         if(_scrollPosition == _loc3_ && scrollBar)
         {
            scrollPosition = (scrollBar as ScrollBar).position;
         }
      }
      
      [Inspectable(defaultValue="false")]
      public function get draggable() : Boolean
      {
         return this._draggable;
      }
      
      public function set draggable(param1:Boolean) : void
      {
         if(param1)
         {
            if(!this._draggable)
            {
               addEventListener(MouseEvent.MOUSE_DOWN,this.stDrag);
            }
         }
         else if(this._draggable)
         {
            removeEventListener(MouseEvent.MOUSE_DOWN,this.stDrag);
         }
         _draggable = param1;
      }
      
      [Inspectable(defaultValue="0")]
      public function get scrollYPadding() : int
      {
         return this._scrollYPadding;
      }
      
      public function set scrollYPadding(param1:int) : void
      {
         _scrollYPadding = param1;
      }
      
      protected function get pos() : Number
      {
         return _scrollPosition * this._pow + this._delta;
      }
      
      protected function set pos(param1:Number) : void
      {
         scrollPosition = param1;
      }
      
      override public function getRendererAt(param1:uint, param2:int = 0) : IListItemRenderer
      {
         if(_renderers == null)
         {
            return null;
         }
         var _loc3_:uint = param1 - param2 * (_direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns);
         if(_loc3_ >= _renderers.length)
         {
            return null;
         }
         return _renderers[_loc3_] as IListItemRenderer;
      }
      
      public function set softScroll(param1:Boolean) : void
      {
         _softScroll = param1;
      }
      
      public function get softScroll() : Boolean
      {
         return this._softScroll;
      }
      
      override public function set selectedIndex(param1:int) : void
      {
         stackTrace("setSelectedIndex::" + String(name) + String(param1));
         if(param1 == _selectedIndex || param1 == _newSelectedIndex)
         {
            return;
         }
         _newSelectedIndex = param1;
         invalidateSelectedIndex();
      }
      
      public function forceScrollToSelected(param1:uint) : void
      {
         if(_totalRenderers == 0)
         {
            return;
         }
         _newSelectedIndex = param1;
         var _loc2_:Number = _direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns;
         var _loc3_:Number = _scrollPosition * _loc2_;
         if(param1 >= _loc3_ && param1 < _loc3_ + _totalRows * _totalColumns)
         {
            scrollPosition = (param1 / _loc2_ >> 0) * this._pow;
         }
         else if(param1 < _loc3_)
         {
            scrollPosition = (param1 / _loc2_ >> 0) * this._pow;
         }
         else
         {
            scrollPosition = this._pow * (Math.floor(param1 / _loc2_) - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows) + 1);
         }
         validateNow();
         this.updateSelectedIndex();
      }
      
      public function getActualScrollPosition() : Number
      {
         var _loc1_:Number = _direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns;
         var _loc2_:Number = Math.ceil(_dataProvider.length / _loc1_);
         var _loc3_:Number = _loc2_ - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows - this.extraRow);
         var _loc4_:Number = Math.max(0,Math.min(_loc3_,_scrollPosition));
         return _loc4_;
      }
      
      override public function scrollToIndex(param1:uint) : void
      {
         if(_totalRenderers == 0)
         {
            return;
         }
         var _loc2_:Number = _direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns;
         var _loc3_:Number = _scrollPosition * _loc2_;
         if(param1 >= _loc3_ && param1 < _loc3_ + _totalRows * _totalColumns)
         {
            if(this._softScroll)
            {
               _scrollTarget = (param1 / _loc2_ >> 0) * this._pow;
               this.scrollList(0);
            }
            else if(_scrollBar)
            {
               scrollBar.position = (param1 / _loc2_ >> 0) * this._pow;
            }
            else
            {
               scrollPosition = (param1 / _loc2_ >> 0) * this._pow;
            }
         }
         else if(param1 < _loc3_)
         {
            if(this._softScroll)
            {
               _scrollTarget = (param1 / _loc2_ >> 0) * this._pow;
               this.scrollList(0);
            }
            else if(_scrollBar)
            {
               _scrollBar.position = (param1 / _loc2_ >> 0) * this._pow;
            }
            else
            {
               scrollPosition = (param1 / _loc2_ >> 0) * this._pow;
            }
         }
         else if(this._softScroll)
         {
            _scrollTarget = this._pow * (Math.floor(param1 / _loc2_) - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows) + 1);
            this.scrollList(0);
         }
         else if(_scrollBar)
         {
            _scrollBar.position = this._pow * (Math.floor(param1 / _loc2_) - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows) + 1);
         }
         else
         {
            scrollPosition = this._pow * (Math.floor(param1 / _loc2_) - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows) + 1);
         }
      }
      
      override protected function draw() : void
      {
         var _loc1_:Number;
         if(isInvalid(InvalidationType.SCROLL_BAR) || isInvalid(InvalidationType.SIZE))
         {
            this._mask.graphics.clear();
            this._mask.graphics.beginFill(0,0.2);
            if(height == 0 || scaleY == 0)
            {
               _loc1_ = 0;
            }
            else
            {
               _loc1_ = height / scaleY;
            }
            this._mask.graphics.drawRect(0,0,width / scaleX,_loc1_);
            this._mask.graphics.endFill();
            this._mask.mouseChildren = false;
            this._mask.mouseEnabled = false;
            if(this.mask != this._mask)
            {
               this.mask = this._mask;
               addChild(this._mask);
            }
         }
         super.draw();
         if(isInvalid(InvalidationType.DATA) && _selectedIndex != -1)
         {
            dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE,true,false,_selectedIndex,-1,-1,this.getRendererAt(_selectedIndex),dataProvider[_selectedIndex]));
         }
      }
      
      override protected function changeFocus() : void
      {
         trace("DISPLAY_FOCUS_st::");
         if(_focused || _displayFocus)
         {
            setState("focused","default");
         }
         else
         {
            setState("default");
         }
         var _loc1_:Number = 1 + Math.ceil((_dataProvider.length - (_totalRows - this.extraRow) * _totalColumns) / (_direction == DirectionMode.HORIZONTAL ? _totalRows - this.extraRow : _totalColumns));
         var _loc2_:Number = this._scrollPosition == _loc1_ && _loc1_ ? this._scrollPosition - 1 : this._scrollPosition;
         var _loc3_:IListItemRenderer = this.getRendererAt(_selectedIndex,_loc2_);
         trace("DISPLAY_FOCUS::",_loc3_,"this scroll::",this._scrollPosition,_loc1_,"getScroll::",_loc2_);
         if(_loc3_ != null)
         {
            if(_noIndex)
            {
               if(focused > 0)
               {
                  _loc3_.displayFocus = true;
                  _loc3_.validateNow();
               }
               else
               {
                  _loc3_.displayFocus = false;
                  _loc3_.validateNow();
               }
            }
            else
            {
               _loc3_.displayFocus = focused > 0;
               _loc3_.validateNow();
            }
         }
      }
      
      override protected function updateSelectedIndex() : void
      {
         if(_selectedIndex == _newSelectedIndex)
         {
            return;
         }
         if(_totalRenderers == 0)
         {
            return;
         }
         var _loc1_:int = _selectedIndex;
         var _loc2_:Number = 1 + Math.ceil((_dataProvider.length - (_totalRows - this.extraRow) * _totalColumns) / (_direction == DirectionMode.HORIZONTAL ? _totalRows - this.extraRow : _totalColumns));
         var _loc3_:Number = this._scrollPosition == _loc2_ && _loc2_ ? this._scrollPosition - 1 : this._scrollPosition;
         var _loc4_:IListItemRenderer = this.getRendererAt(_selectedIndex,_loc3_);
         var _loc5_:Boolean;
         var _loc6_:int;
         var _loc7_:int;
         var _loc8_:int;
         var _loc9_:int;
         var _loc10_:int;
         if(_loc4_ != null)
         {
            _loc4_.selected = false;
            _loc4_.validateNow();
         }
         if(_selectedIndex != _newSelectedIndex)
         {
            _selectedIndex = _newSelectedIndex;
            invalidateSelectedIndex();
            dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE,true,false,_selectedIndex,-1,-1,this.getRendererAt(_selectedIndex),dataProvider[_selectedIndex]));
         }
         if(_selectedIndex < 0 || _selectedIndex >= _dataProvider.length)
         {
            return;
         }
         _loc4_ = this.getRendererAt(_selectedIndex,_loc3_);
         if(_loc4_ != null)
         {
            if(direction == DirectionMode.VERTICAL)
            {
               _loc5_ = _loc4_.y + rowHeight > (rowCount / columnCount - 1 - this.extraRow) * rowHeight;
            }
            else
            {
               _loc5_ = _loc4_.x + columnWidth > (columnCount - 1) * columnWidth;
            }
            if(_loc5_)
            {
               if(this._tween)
               {
                  this._tween.paused = true;
               }
               _loc6_ = _selectedIndex - (_renderers.length - this._toMove * (2 + this.extraRow));
               _scrollTarget = _loc6_ * this._pow;
               this.scrollToIndex(_loc6_);
            }
            else if(_loc4_.y < 0 || _loc4_.x < 0)
            {
               this.scrollToIndex(_selectedIndex);
            }
            _loc4_.selected = true;
            _loc4_.validateNow();
         }
         else
         {
            if(direction == DirectionMode.VERTICAL)
            {
               _loc8_ = rowCount / columnCount;
               _loc9_ = (_loc8_ - 1 - this.extraRow) * rowHeight;
               _loc10_ = Math.ceil(_selectedIndex / (_loc8_ - 1 - this.extraRow));
               _loc7_ = (_loc10_ - _loc3_) * rowHeight;
               _loc5_ = _loc7_ > _loc9_;
            }
            else
            {
               _loc7_ = (_selectedIndex / columnCount + _loc3_ - columnCount) * rowHeight;
               _loc5_ = _loc7_ > (columnCount - 1) * columnWidth;
            }
            if(_loc5_)
            {
               if(this._tween)
               {
                  this._tween.paused = true;
               }
               _loc6_ = _selectedIndex + this._toMove * (1 + this.extraRow);
               _scrollTarget = _loc6_ * this._pow;
               this.scrollToIndex(_loc6_);
            }
            else if(_loc1_ < _selectedIndex)
            {
               this.scrollToIndex(_selectedIndex + 1);
            }
            else
            {
               this.scrollToIndex(_selectedIndex);
            }
            if(_loc4_)
            {
               _loc4_ = this.getRendererAt(_selectedIndex,_loc3_);
               _loc4_.selected = true;
               _loc4_.validateNow();
            }
         }
         if(_scrollBar && _scrollBar.enabled == false)
         {
            this.scrollToIndex(0);
         }
      }
      
      override protected function updateScrollBar() : void
      {
         var _loc1_:Number;
         var _loc2_:ScrollIndicator;
         if(_scrollBar == null)
         {
            return;
         }
         if(direction == DirectionMode.HORIZONTAL)
         {
            _loc1_ = Math.ceil(_dataProvider.length / _totalRows) - _totalColumns;
         }
         else
         {
            _loc1_ = Math.ceil(_dataProvider.length / _totalColumns) - (_totalRows - this.extraRow);
         }
         if(_scrollBar is ScrollIndicator)
         {
            _loc2_ = _scrollBar as ScrollIndicator;
            (_scrollBar as ScrollBar).trackScrollPageSize = this._pow;
            _loc2_.setScrollProperties(_direction == DirectionMode.HORIZONTAL ? _totalColumns * this._pow : _totalRows * this._pow,0,(_loc1_ + 1) * this._pow,this._pow);
         }
         if(_loc1_ < 0)
         {
            _lastDelta = 0;
            _scrollBar.position = 0;
         }
         else if((_loc1_ + 1) * this._pow < _scrollBar.position)
         {
            scrollPosition = (_loc1_ + 1) * this._pow;
            _scrollBar.position = (_loc1_ + 1) * this._pow;
         }
         _scrollBar.validateNow();
      }
      
      override protected function scrollList(param1:int) : void
      {
         var _loc2_:Number = this._pow * (1 + Math.ceil((_dataProvider.length - (_totalRows - this.extraRow) * _totalColumns) / (_direction == DirectionMode.HORIZONTAL ? _totalRows - this.extraRow : _totalColumns)));
         if(_scrollBar && _scrollBar.enabled || !_scrollBarValue)
         {
            _scrollTarget -= param1 * this._pow;
         }
         else if(_scrollBarValue)
         {
            _scrollTarget = 0;
         }
         if(this._scrollTarget > _loc2_)
         {
            _scrollTarget = _loc2_;
         }
         if(this._scrollTarget < 0)
         {
            _scrollTarget = 0;
         }
         if(_scrollBar)
         {
            if(!this._tween)
            {
               _scrollTween = true;
               _tween = new Tween(500,_scrollBar,{position:this._scrollTarget},{
                  ease:Strong.easeOut,
                  onComplete:this.endTween
               });
            }
            else
            {
               this._tween.reset();
               this._tween.paused = false;
               _scrollTween = true;
               this._tween.props = {position:this._scrollTarget};
            }
         }
         else if(!this._tween)
         {
            _scrollTween = true;
            _tween = new Tween(250,this,{pos:this._scrollTarget},{onComplete:this.endTween});
         }
         else
         {
            if(param1 == 0)
            {
               if(_direction == DirectionMode.VERTICAL)
               {
                  this._tween.duration = Math.min(250,250 * (this._pow * Math.abs(this.pos - this._scrollTarget) / _rowHeight));
               }
               else
               {
                  this._tween.duration = Math.min(250,250 * (this._pow * Math.abs(this.pos - this._scrollTarget) / _columnWidth));
               }
            }
            else
            {
               this._tween.duration = 250;
            }
            this._tween.reset();
            this._tween.paused = false;
            _scrollTween = true;
            this._tween.props = {pos:this._scrollTarget};
         }
      }
      
      override protected function drawScrollBar() : void
      {
         if(!_autoScrollBar)
         {
            return;
         }
         var _loc1_:ScrollIndicator = _scrollBar as ScrollIndicator;
         var _loc2_:Number;
         _loc1_.direction = _direction;
         if(_direction == DirectionMode.VERTICAL)
         {
            _loc1_.rotation = 0;
            _loc1_.x = _width - _loc1_.width + margin;
            _loc1_.y = margin;
            _loc2_ = availableHeight;
            _loc1_.height = _loc2_;
         }
         else
         {
            _loc1_.rotation = -90;
            _loc1_.x = margin;
            _loc1_.y = _height - margin + this._scrollYPadding;
            _loc1_.width = availableWidth;
         }
         _scrollBar.validateNow();
      }
      
      override protected function createScrollBar() : void
      {
         var _loc1_:ScrollBar = _scrollBar as ScrollBar;
         if(_loc1_)
         {
            _scrollBar.removeEventListener(Event.COMPLETE,this.onSBarDragEnd);
            _scrollBar.removeEventListener(Event.SELECT,this.onSBarDragStart);
            _loc1_.track.removeEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            if(_loc1_.upArrow)
            {
               _loc1_.upArrow.removeEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
            if(_loc1_.downArrow)
            {
               _loc1_.downArrow.removeEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
         }
         super.createScrollBar();
         if(_loc1_)
         {
            _scrollBar.addEventListener(Event.COMPLETE,this.onSBarDragEnd);
            _scrollBar.addEventListener(Event.SELECT,this.onSBarDragStart);
            _loc1_.track.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            if(_loc1_.upArrow)
            {
               _loc1_.upArrow.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
            if(_loc1_.downArrow)
            {
               _loc1_.downArrow.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
         }
      }
      
      override protected function calculateRendererTotal(param1:Number, param2:Number) : uint
      {
         var _loc3_:Boolean = isNaN(_rowHeight) && isNaN(_autoRowHeight);
         var _loc4_:Boolean = isNaN(_columnWidth) && isNaN(_autoColumnWidth);
         var _loc5_:IListItemRenderer;
         if(_loc3_ || _loc4_)
         {
            _loc5_ = createRenderer(0);
            if(_loc3_)
            {
               _autoRowHeight = _loc5_.height;
            }
            if(_loc4_)
            {
               _autoColumnWidth = _loc5_.width;
            }
            cleanUpRenderer(_loc5_);
         }
         if(Math.floor(availableHeight / rowHeight) < Math.ceil(availableHeight / rowHeight))
         {
            extraRow = 1;
         }
         else
         {
            extraRow = 0;
         }
         _totalRows = (_direction == DirectionMode.VERTICAL ? 1 : 0) + Math.ceil(availableHeight / rowHeight);
         _totalColumns = (_direction == DirectionMode.VERTICAL ? 0 : 1) + availableWidth / columnWidth >> 0;
         _totalRenderers = _totalRows * _totalColumns;
         _toMove = _direction == DirectionMode.VERTICAL ? (_totalColumns) : (_totalRows);
         return _totalRenderers;
      }
      
      protected function goDrag(param1:MouseEvent) : void
      {
         var _loc2_:int;
         var _loc3_:int;
         var _loc4_:int;
         var _loc5_:Number;
         if(_direction == DirectionMode.VERTICAL)
         {
            if(this._dragY == mouseY)
            {
               return;
            }
            _loc3_ = _rowHeight / this._pow;
            _loc4_ = int(mouseY * scaleY / _loc3_) * _loc3_;
            _loc2_ = int((this._dragY - _loc4_) / _loc3_) * _loc3_;
            if(_loc2_ == 0)
            {
               return;
            }
            _dragY = _loc4_;
         }
         else
         {
            if(this._dragX == mouseX)
            {
               return;
            }
            _loc3_ = _columnWidth / this._pow;
            _loc4_ = int(mouseX * scaleX / _loc3_) * _loc3_;
            _loc2_ = int((this._dragX - _loc4_) / _loc3_) * _loc3_;
            if(_loc2_ == 0)
            {
               return;
            }
            _dragX = _loc4_;
         }
         if(_scrollBar)
         {
            _scrollBar.position += _loc2_ / _loc3_;
         }
         else
         {
            _loc5_ = this._pow * Math.ceil(1 + (_dataProvider.length - _totalRows * _totalColumns) / (_direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns));
            if(_scrollPosition * this._pow + this._delta + _loc2_ / _loc3_ > _loc5_)
            {
               scrollPosition = Math.max(_loc5_,0);
            }
            else if(_scrollPosition * this._pow + this._delta + _loc2_ / _loc3_ < 0)
            {
               scrollPosition = 0;
            }
            else
            {
               scrollPosition = _scrollPosition * this._pow + this._delta + _loc2_ / _loc3_;
            }
         }
      }
      
      protected function stDrag(param1:MouseEvent) : void
      {
         var _loc2_:int = _rowHeight / this._pow;
         _dragX = mouseX * scaleX;
         _dragY = mouseY * scaleY;
         if(param1.type == MouseEvent.MOUSE_DOWN)
         {
            if(stage)
            {
               stage.addEventListener(MouseEvent.MOUSE_UP,this.stDrag);
            }
            _inDrag = true;
            if(this._tween)
            {
               this._tween.reset();
               this._tween.paused = true;
               _scrollTween = false;
            }
            addEventListener(MouseEvent.MOUSE_MOVE,this.goDrag);
         }
         else if(this._inDrag != false)
         {
            _inDrag = false;
            removeEventListener(MouseEvent.MOUSE_MOVE,this.goDrag);
            if(stage)
            {
               stage.removeEventListener(MouseEvent.MOUSE_UP,this.stDrag);
            }
            this.onSBarDragEnd();
         }
      }
      
      protected function handleTrackPress(param1:MouseEvent) : void
      {
         if((_scrollBar as ScrollBar).thumb.y > (_scrollBar as ScrollBar).mouseY)
         {
            this.scrollList(1);
         }
         else
         {
            this.scrollList(-1);
         }
         param1.stopImmediatePropagation();
      }
      
      protected function onSBarDragStart(param1:Event = null) : void
      {
         if(this._tween)
         {
            this._tween.reset();
            this._tween.paused = true;
            _scrollTween = false;
         }
      }
      
      protected function onSBarDragEnd(param1:Event = null) : void
      {
         if(_scrollBar)
         {
            _scrollTarget = Math.round(_scrollBar.position / this._pow) * this._pow;
         }
         else
         {
            _scrollTarget = Math.round(this.pos / this._pow) * this._pow;
         }
         this.scrollList(0);
      }
      
      protected function endTween(param1:* = null) : void
      {
         _scrollTween = false;
      }
   }
}

