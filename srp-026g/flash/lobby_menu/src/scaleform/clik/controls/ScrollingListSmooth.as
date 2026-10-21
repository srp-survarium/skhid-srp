package scaleform.clik.controls
{
   import com.greensock.TweenLite;
   import fl.transitions.easing.Strong;
   import flash.display.DisplayObject;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.data.ListData;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.interfaces.IListItemRenderer;
   import scaleform.clik.motion.Tween;
   
   public class ScrollingListSmooth extends ScrollingList
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
      
      protected var _lastIdex:Boolean = false;
      
      protected var _draggable:Boolean = false;
      
      protected var _dragX:Number = 0;
      
      protected var _dragY:Number = 0;
      
      protected var _maskAutoResize:Boolean = false;
      
      protected var _rowScroll:uint = 0;
      
      protected var _mask:Sprite = new Sprite();
      
      public function ScrollingListSmooth()
      {
         super();
      }
      
      public function set rowScroll(value:uint) : void
      {
         _rowScroll = value;
         this.updateRenderers();
      }
      
      public function get rowScroll() : uint
      {
         return this._rowScroll;
      }
      
      public function set maskAutoResize(value:Boolean) : void
      {
         _maskAutoResize = value;
      }
      
      public function get maskAutoResize() : Boolean
      {
         return this._maskAutoResize;
      }
      
      override public function get scrollPosition() : Number
      {
         return _scrollPosition;
      }
      
      override public function set scrollPosition(value:Number) : void
      {
         var unused:Boolean = false;
         _delta = value % this._pow;
         var maxScroll:Number = this.getMexScrollPosition();
         if(value == maxScroll * this._pow && value != 0)
         {
            _delta = this._pow;
            _lastIdex = true;
         }
         else
         {
            _lastIdex = false;
         }
         var rowIndex:int = int(value / this._pow);
         var i:int;
         var renderer:IListItemRenderer;
         if(value != maxScroll * this._pow && this._lastDelta != maxScroll)
         {
            if(rowIndex - this._lastDelta >= 1)
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
            else if(rowIndex - this._lastDelta <= -1)
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
         _lastDelta = rowIndex;
         this.updateRenderers();
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
      
      public function playScrollTween() : void
      {
         _rowScroll = _rowHeight;
         TweenLite.to(this,0.2,{rowScroll:0});
      }
      
      public function getScrollable() : Boolean
      {
         return _dataProvider.length - _totalRenderers > -1;
      }
      
      override public function getRendererAt(itemIndex:uint, offset:int = 0) : IListItemRenderer
      {
         if(_renderers == null)
         {
            return null;
         }
         var maxScroll:Number = Math.ceil(_dataProvider.length - _rowCount) + 1;
         var startIndex:Number = offset == maxScroll && maxScroll ? offset - 1 : offset;
         var rendererIndex:uint = itemIndex - startIndex;
         if(rendererIndex >= _renderers.length)
         {
            return null;
         }
         return _renderers[rendererIndex] as IListItemRenderer;
      }
      
      override public function scrollToIndex(itemIndex:uint) : void
      {
         if(_totalRenderers == 0)
         {
            return;
         }
         var perRow:Number = 1;
         var firstVisible:Number = _scrollPosition * perRow;
         if(itemIndex >= firstVisible && itemIndex < firstVisible + _rowCount)
         {
            if(this._softScroll)
            {
               _scrollTarget = (itemIndex / perRow >> 0) * this._pow;
               this.scrollList(0);
            }
            else if(_scrollBar)
            {
               scrollBar.position = (itemIndex / perRow >> 0) * this._pow;
            }
            else
            {
               scrollPosition = (itemIndex / perRow >> 0) * this._pow;
            }
         }
         else if(itemIndex < firstVisible)
         {
            if(this._softScroll)
            {
               _scrollTarget = (itemIndex / perRow >> 0) * this._pow;
               this.scrollList(0);
            }
            else if(_scrollBar)
            {
               _scrollBar.position = (itemIndex / perRow >> 0) * this._pow;
            }
            else
            {
               scrollPosition = (itemIndex / perRow >> 0) * this._pow;
            }
         }
         else if(this._softScroll)
         {
            _scrollTarget = this._pow * (Math.floor(itemIndex / perRow) - _rowCount + 1);
            this.scrollList(0);
         }
         else if(_scrollBar)
         {
            _scrollBar.position = this._pow * (Math.floor(itemIndex / perRow) - _rowCount + 1);
         }
         else
         {
            scrollPosition = this._pow * (Math.floor(itemIndex / perRow) - _rowCount + 1);
         }
      }
      
      override protected function draw() : void
      {
         var maxScroll:Number;
         var maskHeight:Number;
         var sbDisplay:DisplayObject;
         if(isInvalid(InvalidationType.DATA))
         {
            maxScroll = Math.ceil(_dataProvider.length - _rowCount) + 1;
            if(_scrollPosition != maxScroll && this._delta == this._pow)
            {
               _delta = 0;
            }
         }
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
         if(isInvalid(InvalidationType.SCROLL_BAR) || isInvalid(InvalidationType.SIZE))
         {
            sbDisplay = _scrollBar as DisplayObject;
            if(sbDisplay && container.contains(sbDisplay))
            {
               container.addChild(sbDisplay);
            }
         }
         if(this._maskAutoResize)
         {
            this._mask.width = container.width;
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
         var renderer:IListItemRenderer = this.getRendererAt(_selectedIndex,_scrollPosition);
         var pastBottom:Boolean;
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
         renderer = this.getRendererAt(_selectedIndex,_scrollPosition);
         if(renderer != null)
         {
            pastBottom = renderer.y + rowHeight > (_rowCount - 1) * rowHeight;
            if(pastBottom)
            {
               if(this._tween)
               {
                  this._tween.paused = true;
               }
               _scrollTarget = _selectedIndex - (_renderers.length - this._toMove * 2) * this._pow;
               this.scrollToIndex(_selectedIndex - (_renderers.length - this._toMove * 2));
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
            if(oldIndex < _selectedIndex)
            {
               this.scrollToIndex(_selectedIndex + 1);
            }
            else
            {
               this.scrollToIndex(_selectedIndex);
            }
            if(renderer)
            {
               renderer = this.getRendererAt(_selectedIndex,_scrollPosition);
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
         maxScroll = Math.ceil(_dataProvider.length) - _rowCount;
         if(_scrollBar is ScrollIndicator)
         {
            indicator = _scrollBar as ScrollIndicator;
            (_scrollBar as ScrollBar).trackScrollPageSize = this._pow;
            indicator.setScrollProperties(_rowCount * this._pow,0,(maxScroll + 1) * this._pow,this._pow);
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
         else if(this._lastIdex)
         {
            scrollPosition = _scrollBar.position;
         }
         _scrollBar.validateNow();
      }
      
      override protected function scrollList(delta:int) : void
      {
         trace("scrollList::",delta,this._scrollTarget);
         var maxTarget:Number = this._pow * this.getMexScrollPosition();
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
               this._tween.duration = Math.min(250,250 * (this._pow * Math.abs(this.pos - this._scrollTarget) / _rowHeight));
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
         indicator.rotation = 0;
         indicator.x = _width - indicator.width + margin;
         indicator.y = margin;
         indicator.height = int(_rowCount - 1) * rowHeight;
         _scrollBar.validateNow();
      }
      
      override protected function createScrollBar() : void
      {
         var sb:ScrollBar = _scrollBar as ScrollBar;
         if(_scrollBar)
         {
            _scrollBar.removeEventListener(Event.COMPLETE,this.onSBarDragEnd);
            _scrollBar.removeEventListener(Event.SELECT,this.onSBarDragStart);
            if(sb.track)
            {
               sb.track.removeEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
            if(sb.upArrow)
            {
               sb.upArrow.removeEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
            if(sb.downArrow)
            {
               sb.downArrow.removeEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
         }
         super.createScrollBar();
         sb = _scrollBar as ScrollBar;
         if(_scrollBar)
         {
            _scrollBar.addEventListener(Event.COMPLETE,this.onSBarDragEnd);
            _scrollBar.addEventListener(Event.SELECT,this.onSBarDragStart);
            if(sb.track)
            {
               sb.track.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
            if(sb.upArrow)
            {
               sb.upArrow.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
            if(sb.downArrow)
            {
               sb.downArrow.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
         }
      }
      
      override protected function calculateRendererTotal(totalWidth:Number, totalHeight:Number) : uint
      {
         var needsRowHeight:Boolean = isNaN(_rowHeight) && isNaN(_autoRowHeight);
         var renderer:IListItemRenderer;
         if(needsRowHeight)
         {
            renderer = createRenderer(0);
            if(needsRowHeight)
            {
               _autoRowHeight = renderer.height;
            }
            cleanUpRenderer(renderer);
         }
         _rowCount = 1 + Math.ceil(availableHeight / rowHeight);
         _totalRenderers = _rowCount;
         _toMove = 1;
         return _totalRenderers;
      }
      
      override protected function refreshData() : void
      {
         var startIndex:uint = Math.min(Math.max(0,_dataProvider.length - _totalRenderers),_scrollPosition);
         _scrollPosition = startIndex;
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
         this.updateSelectedIndex();
         _dataProvider.requestItemRange(startIndex,Math.min(_dataProvider.length - 1,startIndex + _totalRenderers - 1),this.populateData);
         this.updateRenderers();
      }
      
      override protected function populateData(items:Array) : void
      {
         var itemCount:uint = items.length;
         var rendererCount:uint = _renderers.length;
         var i:uint = 0;
         var renderer:IListItemRenderer;
         var itemIndex:uint;
         var listData:ListData;
         while(i < rendererCount)
         {
            renderer = this.getRendererAt(i);
            itemIndex = _scrollPosition + i;
            listData = new ListData(itemIndex,itemToLabel(items[i]),_selectedIndex == itemIndex);
            renderer.enabled = i >= itemCount ? false : true;
            renderer.setListData(listData);
            renderer.setData(items[i]);
            renderer.validateNow();
            i++;
         }
      }
      
      protected function getMexScrollPosition() : int
      {
         return Math.ceil(_dataProvider.length - _rowCount) + 1;
      }
      
      protected function updateRenderers() : void
      {
         var rendererCount:uint = _renderers.length;
         var i:int = 0;
         var renderer:IListItemRenderer;
         while(i < rendererCount)
         {
            renderer = this.getRendererAt(i);
            renderer.y = i * _rowHeight + margin - _rowHeight / this._pow * this._delta - this._rowScroll;
            i++;
         }
      }
      
      protected function goDrag(event:MouseEvent) : void
      {
         var dragDelta:int;
         var step:int;
         if(this._dragY == mouseY)
         {
            return;
         }
         step = _rowHeight / this._pow;
         var snappedY:int = int(mouseY * scaleY / step) * step;
         var maxPos:Number;
         dragDelta = int((this._dragY - snappedY) / step) * step;
         if(dragDelta == 0)
         {
            return;
         }
         _dragY = snappedY;
         if(_scrollBar)
         {
            _scrollBar.position += dragDelta / step;
         }
         else
         {
            maxPos = this._pow * Math.ceil(1 + (_dataProvider.length - _rowCount));
            if(_scrollPosition * this._pow + this._delta + dragDelta / step > maxPos)
            {
               scrollPosition = Math.max(maxPos,0);
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
      
      protected function endTween(target:* = null) : void
      {
         _scrollTween = false;
      }
   }
}

