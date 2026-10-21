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
      
      override public function set dataProvider(value:IDataProvider) : void
      {
         super.dataProvider = value;
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
      
      override public function set scrollPosition(value:Number) : void
      {
         var moved:Boolean = false;
         _delta = value % this._pow;
         var perLine:Number = _direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns;
         var lineCount:Number = Math.ceil(_dataProvider.length / perLine);
         var maxScroll:Number = lineCount - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows - 1 - this.extraRow);
         if(value == maxScroll * this._pow && value != 0)
         {
            _delta = this._pow;
         }
         var lineIndex:int = int(value / this._pow);
         if(value != maxScroll * this._pow && this._lastDelta != maxScroll)
         {
            if(lineIndex - this._lastDelta >= 1)
            {
               i = 0;
               while(i < this._toMove)
               {
                  renderer = this.getRendererAt(0);
                  _renderers.shift();
                  _renderers.push(renderer);
                  i++;
               }
            }
            else if(lineIndex - this._lastDelta <= -1)
            {
               i = 0;
               while(i < this._toMove)
               {
                  renderer = this.getRendererAt(_renderers.length - 1);
                  _renderers.unshift(renderer);
                  _renderers.pop();
                  i++;
               }
            }
         }
         _lastDelta = lineIndex;
         var oldPosition:Number = _scrollPosition;
         var rendererCount:int = (_renderers.length);
         var i:int;
         var renderer:IListItemRenderer;
         i = 0;
         while(i < _renderers.length)
         {
            renderer = this.getRendererAt(i);
            if(_direction == DirectionMode.VERTICAL)
            {
               renderer.y = (i / _totalColumns >> 0) * rowHeight + margin - _rowHeight / this._pow * this._delta;
            }
            else
            {
               renderer.x = (i / _totalRows >> 0) * columnWidth + margin - _columnWidth / this._pow * this._delta;
            }
            i++;
         }
         value = Math.max(0,Math.min(maxScroll,int(value / this._pow)));
         if(_scrollPosition == value)
         {
            return;
         }
         _scrollPosition = value;
         if(!this._scrollTween)
         {
            _scrollTarget = _scrollPosition * this._pow;
         }
         invalidateData();
      }
      
      override protected function refreshData() : void
      {
         var perLine:Number = _direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns;
         var lineCount:Number = Math.ceil(_dataProvider.length / perLine);
         var maxScroll:Number = lineCount - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows - this.extraRow);
         _scrollPosition = Math.max(0,Math.min(maxScroll,_scrollPosition));
         var startIndex:Number = _scrollPosition * perLine;
         var endIndex:Number = startIndex + _totalColumns * _totalRows - 1;
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
         this.updateSelectedIndex();
         _dataProvider.requestItemRange(startIndex,endIndex,populateData);
         if(_scrollPosition == maxScroll && scrollBar)
         {
            scrollPosition = (scrollBar as ScrollBar).position;
         }
      }
      
      [Inspectable(defaultValue="false")]
      public function get draggable() : Boolean
      {
         return this._draggable;
      }
      
      public function set draggable(value:Boolean) : void
      {
         if(value)
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
         _draggable = value;
      }
      
      [Inspectable(defaultValue="0")]
      public function get scrollYPadding() : int
      {
         return this._scrollYPadding;
      }
      
      public function set scrollYPadding(value:int) : void
      {
         _scrollYPadding = value;
      }
      
      protected function get pos() : Number
      {
         return _scrollPosition * this._pow + this._delta;
      }
      
      protected function set pos(value:Number) : void
      {
         scrollPosition = value;
      }
      
      override public function getRendererAt(itemIndex:uint, offset:int = 0) : IListItemRenderer
      {
         if(_renderers == null)
         {
            return null;
         }
         var rendererIndex:uint = itemIndex - offset * (_direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns);
         if(rendererIndex >= _renderers.length)
         {
            return null;
         }
         return _renderers[rendererIndex] as IListItemRenderer;
      }
      
      public function set softScroll(value:Boolean) : void
      {
         _softScroll = value;
      }
      
      public function get softScroll() : Boolean
      {
         return this._softScroll;
      }
      
      override public function set selectedIndex(value:int) : void
      {
         stackTrace("setSelectedIndex::" + String(name) + String(value));
         if(value == _selectedIndex || value == _newSelectedIndex)
         {
            return;
         }
         _newSelectedIndex = value;
         invalidateSelectedIndex();
      }
      
      public function forceScrollToSelected(itemIndex:uint) : void
      {
         if(_totalRenderers == 0)
         {
            return;
         }
         _newSelectedIndex = itemIndex;
         var perLine:Number = _direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns;
         var firstVisible:Number = _scrollPosition * perLine;
         if(itemIndex >= firstVisible && itemIndex < firstVisible + _totalRows * _totalColumns)
         {
            scrollPosition = (itemIndex / perLine >> 0) * this._pow;
         }
         else if(itemIndex < firstVisible)
         {
            scrollPosition = (itemIndex / perLine >> 0) * this._pow;
         }
         else
         {
            scrollPosition = this._pow * (Math.floor(itemIndex / perLine) - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows) + 1);
         }
         validateNow();
         this.updateSelectedIndex();
      }
      
      public function getActualScrollPosition() : Number
      {
         var perLine:Number = _direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns;
         var lineCount:Number = Math.ceil(_dataProvider.length / perLine);
         var maxScroll:Number = lineCount - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows - this.extraRow);
         var position:Number = Math.max(0,Math.min(maxScroll,_scrollPosition));
         return position;
      }
      
      override public function scrollToIndex(itemIndex:uint) : void
      {
         if(_totalRenderers == 0)
         {
            return;
         }
         var perLine:Number = _direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns;
         var firstVisible:Number = _scrollPosition * perLine;
         if(itemIndex >= firstVisible && itemIndex < firstVisible + _totalRows * _totalColumns)
         {
            if(this._softScroll)
            {
               _scrollTarget = (itemIndex / perLine >> 0) * this._pow;
               this.scrollList(0);
            }
            else if(_scrollBar)
            {
               scrollBar.position = (itemIndex / perLine >> 0) * this._pow;
            }
            else
            {
               scrollPosition = (itemIndex / perLine >> 0) * this._pow;
            }
         }
         else if(itemIndex < firstVisible)
         {
            if(this._softScroll)
            {
               _scrollTarget = (itemIndex / perLine >> 0) * this._pow;
               this.scrollList(0);
            }
            else if(_scrollBar)
            {
               _scrollBar.position = (itemIndex / perLine >> 0) * this._pow;
            }
            else
            {
               scrollPosition = (itemIndex / perLine >> 0) * this._pow;
            }
         }
         else if(this._softScroll)
         {
            _scrollTarget = this._pow * (Math.floor(itemIndex / perLine) - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows) + 1);
            this.scrollList(0);
         }
         else if(_scrollBar)
         {
            _scrollBar.position = this._pow * (Math.floor(itemIndex / perLine) - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows) + 1);
         }
         else
         {
            scrollPosition = this._pow * (Math.floor(itemIndex / perLine) - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows) + 1);
         }
      }
      
      override protected function draw() : void
      {
         var maskHeight:Number;
         if(isInvalid(InvalidationType.SCROLL_BAR) || isInvalid(InvalidationType.SIZE))
         {
            this._mask.graphics.clear();
            this._mask.graphics.beginFill(0,0.2);
            if(height == 0 || scaleY == 0)
            {
               maskHeight = 0;
            }
            else
            {
               maskHeight = height / scaleY;
            }
            this._mask.graphics.drawRect(0,0,width / scaleX,maskHeight);
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
         var maxScroll:Number = 1 + Math.ceil((_dataProvider.length - (_totalRows - this.extraRow) * _totalColumns) / (_direction == DirectionMode.HORIZONTAL ? _totalRows - this.extraRow : _totalColumns));
         var offset:Number = this._scrollPosition == maxScroll && maxScroll ? this._scrollPosition - 1 : this._scrollPosition;
         var renderer:IListItemRenderer = this.getRendererAt(_selectedIndex,offset);
         trace("DISPLAY_FOCUS::",renderer,"this scroll::",this._scrollPosition,maxScroll,"getScroll::",offset);
         if(renderer != null)
         {
            if(_noIndex)
            {
               if(focused > 0)
               {
                  renderer.displayFocus = true;
                  renderer.validateNow();
               }
               else
               {
                  renderer.displayFocus = false;
                  renderer.validateNow();
               }
            }
            else
            {
               renderer.displayFocus = focused > 0;
               renderer.validateNow();
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
         var oldIndex:int = _selectedIndex;
         var maxScroll:Number = 1 + Math.ceil((_dataProvider.length - (_totalRows - this.extraRow) * _totalColumns) / (_direction == DirectionMode.HORIZONTAL ? _totalRows - this.extraRow : _totalColumns));
         var offset:Number = this._scrollPosition == maxScroll && maxScroll ? this._scrollPosition - 1 : this._scrollPosition;
         var renderer:IListItemRenderer = this.getRendererAt(_selectedIndex,offset);
         var pastEnd:Boolean;
         var targetIndex:int;
         var itemPos:int;
         var visibleRows:int;
         var visibleExtent:int;
         var targetRow:int;
         if(renderer != null)
         {
            renderer.selected = false;
            renderer.validateNow();
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
         renderer = this.getRendererAt(_selectedIndex,offset);
         if(renderer != null)
         {
            if(direction == DirectionMode.VERTICAL)
            {
               pastEnd = renderer.y + rowHeight > (rowCount / columnCount - 1 - this.extraRow) * rowHeight;
            }
            else
            {
               pastEnd = renderer.x + columnWidth > (columnCount - 1) * columnWidth;
            }
            if(pastEnd)
            {
               if(this._tween)
               {
                  this._tween.paused = true;
               }
               targetIndex = _selectedIndex - (_renderers.length - this._toMove * (2 + this.extraRow));
               _scrollTarget = targetIndex * this._pow;
               this.scrollToIndex(targetIndex);
            }
            else if(renderer.y < 0 || renderer.x < 0)
            {
               this.scrollToIndex(_selectedIndex);
            }
            renderer.selected = true;
            renderer.validateNow();
         }
         else
         {
            if(direction == DirectionMode.VERTICAL)
            {
               visibleRows = rowCount / columnCount;
               visibleExtent = (visibleRows - 1 - this.extraRow) * rowHeight;
               targetRow = Math.ceil(_selectedIndex / (visibleRows - 1 - this.extraRow));
               itemPos = (targetRow - offset) * rowHeight;
               pastEnd = itemPos > visibleExtent;
            }
            else
            {
               itemPos = (_selectedIndex / columnCount + offset - columnCount) * rowHeight;
               pastEnd = itemPos > (columnCount - 1) * columnWidth;
            }
            if(pastEnd)
            {
               if(this._tween)
               {
                  this._tween.paused = true;
               }
               targetIndex = _selectedIndex + this._toMove * (1 + this.extraRow);
               _scrollTarget = targetIndex * this._pow;
               this.scrollToIndex(targetIndex);
            }
            else if(oldIndex < _selectedIndex)
            {
               this.scrollToIndex(_selectedIndex + 1);
            }
            else
            {
               this.scrollToIndex(_selectedIndex);
            }
            if(renderer)
            {
               renderer = this.getRendererAt(_selectedIndex,offset);
               renderer.selected = true;
               renderer.validateNow();
            }
         }
         if(_scrollBar && _scrollBar.enabled == false)
         {
            this.scrollToIndex(0);
         }
      }
      
      override protected function updateScrollBar() : void
      {
         var maxScroll:Number;
         var indicator:ScrollIndicator;
         if(_scrollBar == null)
         {
            return;
         }
         if(direction == DirectionMode.HORIZONTAL)
         {
            maxScroll = Math.ceil(_dataProvider.length / _totalRows) - _totalColumns;
         }
         else
         {
            maxScroll = Math.ceil(_dataProvider.length / _totalColumns) - (_totalRows - this.extraRow);
         }
         if(_scrollBar is ScrollIndicator)
         {
            indicator = _scrollBar as ScrollIndicator;
            (_scrollBar as ScrollBar).trackScrollPageSize = this._pow;
            indicator.setScrollProperties(_direction == DirectionMode.HORIZONTAL ? _totalColumns * this._pow : _totalRows * this._pow,0,(maxScroll + 1) * this._pow,this._pow);
         }
         if(maxScroll < 0)
         {
            _lastDelta = 0;
            _scrollBar.position = 0;
         }
         else if((maxScroll + 1) * this._pow < _scrollBar.position)
         {
            scrollPosition = (maxScroll + 1) * this._pow;
            _scrollBar.position = (maxScroll + 1) * this._pow;
         }
         _scrollBar.validateNow();
      }
      
      override protected function scrollList(delta:int) : void
      {
         var maxTarget:Number = this._pow * (1 + Math.ceil((_dataProvider.length - (_totalRows - this.extraRow) * _totalColumns) / (_direction == DirectionMode.HORIZONTAL ? _totalRows - this.extraRow : _totalColumns)));
         if(_scrollBar && _scrollBar.enabled || !_scrollBarValue)
         {
            _scrollTarget -= delta * this._pow;
         }
         else if(_scrollBarValue)
         {
            _scrollTarget = 0;
         }
         if(this._scrollTarget > maxTarget)
         {
            _scrollTarget = maxTarget;
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
            if(delta == 0)
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
         var indicator:ScrollIndicator = _scrollBar as ScrollIndicator;
         var barHeight:Number;
         indicator.direction = _direction;
         if(_direction == DirectionMode.VERTICAL)
         {
            indicator.rotation = 0;
            indicator.x = _width - indicator.width + margin;
            indicator.y = margin;
            barHeight = availableHeight;
            indicator.height = barHeight;
         }
         else
         {
            indicator.rotation = -90;
            indicator.x = margin;
            indicator.y = _height - margin + this._scrollYPadding;
            indicator.width = availableWidth;
         }
         _scrollBar.validateNow();
      }
      
      override protected function createScrollBar() : void
      {
         var bar:ScrollBar = _scrollBar as ScrollBar;
         if(bar)
         {
            _scrollBar.removeEventListener(Event.COMPLETE,this.onSBarDragEnd);
            _scrollBar.removeEventListener(Event.SELECT,this.onSBarDragStart);
            bar.track.removeEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            if(bar.upArrow)
            {
               bar.upArrow.removeEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
            if(bar.downArrow)
            {
               bar.downArrow.removeEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
         }
         super.createScrollBar();
         if(bar)
         {
            _scrollBar.addEventListener(Event.COMPLETE,this.onSBarDragEnd);
            _scrollBar.addEventListener(Event.SELECT,this.onSBarDragStart);
            bar.track.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            if(bar.upArrow)
            {
               bar.upArrow.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
            if(bar.downArrow)
            {
               bar.downArrow.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
         }
      }
      
      override protected function calculateRendererTotal(totalWidth:Number, totalHeight:Number) : uint
      {
         var measureRow:Boolean = isNaN(_rowHeight) && isNaN(_autoRowHeight);
         var measureColumn:Boolean = isNaN(_columnWidth) && isNaN(_autoColumnWidth);
         var renderer:IListItemRenderer;
         if(measureRow || measureColumn)
         {
            renderer = createRenderer(0);
            if(measureRow)
            {
               _autoRowHeight = renderer.height;
            }
            if(measureColumn)
            {
               _autoColumnWidth = renderer.width;
            }
            cleanUpRenderer(renderer);
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
      
      protected function goDrag(event:MouseEvent) : void
      {
         var dragDelta:int;
         var step:int;
         var snapped:int;
         var maxScroll:Number;
         if(_direction == DirectionMode.VERTICAL)
         {
            if(this._dragY == mouseY)
            {
               return;
            }
            step = _rowHeight / this._pow;
            snapped = int(mouseY * scaleY / step) * step;
            dragDelta = int((this._dragY - snapped) / step) * step;
            if(dragDelta == 0)
            {
               return;
            }
            _dragY = snapped;
         }
         else
         {
            if(this._dragX == mouseX)
            {
               return;
            }
            step = _columnWidth / this._pow;
            snapped = int(mouseX * scaleX / step) * step;
            dragDelta = int((this._dragX - snapped) / step) * step;
            if(dragDelta == 0)
            {
               return;
            }
            _dragX = snapped;
         }
         if(_scrollBar)
         {
            _scrollBar.position += dragDelta / step;
         }
         else
         {
            maxScroll = this._pow * Math.ceil(1 + (_dataProvider.length - _totalRows * _totalColumns) / (_direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns));
            if(_scrollPosition * this._pow + this._delta + dragDelta / step > maxScroll)
            {
               scrollPosition = Math.max(maxScroll,0);
            }
            else if(_scrollPosition * this._pow + this._delta + dragDelta / step < 0)
            {
               scrollPosition = 0;
            }
            else
            {
               scrollPosition = _scrollPosition * this._pow + this._delta + dragDelta / step;
            }
         }
      }
      
      protected function stDrag(event:MouseEvent) : void
      {
         var step:int = _rowHeight / this._pow;
         _dragX = mouseX * scaleX;
         _dragY = mouseY * scaleY;
         if(event.type == MouseEvent.MOUSE_DOWN)
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
      
      protected function handleTrackPress(event:MouseEvent) : void
      {
         if((_scrollBar as ScrollBar).thumb.y > (_scrollBar as ScrollBar).mouseY)
         {
            this.scrollList(1);
         }
         else
         {
            this.scrollList(-1);
         }
         event.stopImmediatePropagation();
      }
      
      protected function onSBarDragStart(event:Event = null) : void
      {
         if(this._tween)
         {
            this._tween.reset();
            this._tween.paused = true;
            _scrollTween = false;
         }
      }
      
      protected function onSBarDragEnd(event:Event = null) : void
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
      
      protected function endTween(tween:* = null) : void
      {
         _scrollTween = false;
      }
   }
}

