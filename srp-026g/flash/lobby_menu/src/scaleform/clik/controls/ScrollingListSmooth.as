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
      
      public function set rowScroll(param1:uint) : void
      {
         _rowScroll = param1;
         this.updateRenderers();
      }
      
      public function get rowScroll() : uint
      {
         return this._rowScroll;
      }
      
      public function set maskAutoResize(param1:Boolean) : void
      {
         _maskAutoResize = param1;
      }
      
      public function get maskAutoResize() : Boolean
      {
         return this._maskAutoResize;
      }
      
      override public function get scrollPosition() : Number
      {
         return _scrollPosition;
      }
      
      override public function set scrollPosition(param1:Number) : void
      {
         var _loc2_:Boolean = false;
         _delta = param1 % this._pow;
         var _loc3_:Number = this.getMexScrollPosition();
         if(param1 == _loc3_ * this._pow && param1 != 0)
         {
            _delta = this._pow;
            _lastIdex = true;
         }
         else
         {
            _lastIdex = false;
         }
         var _loc4_:int = int(param1 / this._pow);
         var _loc5_:int;
         var _loc6_:IListItemRenderer;
         if(param1 != _loc3_ * this._pow && this._lastDelta != _loc3_)
         {
            if(_loc4_ - this._lastDelta >= 1)
            {
               _loc5_ = 0;
               while(_loc5_ < this._toMove)
               {
                  _loc6_ = this.getRendererAt(0);
                  _renderers.shift();
                  _renderers.push(_loc6_);
                  _loc5_++;
               }
            }
            else if(_loc4_ - this._lastDelta <= -1)
            {
               _loc5_ = 0;
               while(_loc5_ < this._toMove)
               {
                  _loc6_ = this.getRendererAt(_renderers.length - 1);
                  _renderers.unshift(_loc6_);
                  _renderers.pop();
                  _loc5_++;
               }
            }
         }
         _lastDelta = _loc4_;
         this.updateRenderers();
         param1 = Math.max(0,Math.min(_loc3_,int(param1 / this._pow)));
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
      
      public function playScrollTween() : void
      {
         _rowScroll = _rowHeight;
         TweenLite.to(this,0.2,{rowScroll:0});
      }
      
      public function getScrollable() : Boolean
      {
         return _dataProvider.length - _totalRenderers > -1;
      }
      
      override public function getRendererAt(param1:uint, param2:int = 0) : IListItemRenderer
      {
         if(_renderers == null)
         {
            return null;
         }
         var _loc3_:Number = Math.ceil(_dataProvider.length - _rowCount) + 1;
         var _loc4_:Number = param2 == _loc3_ && _loc3_ ? param2 - 1 : param2;
         var _loc5_:uint = param1 - _loc4_;
         if(_loc5_ >= _renderers.length)
         {
            return null;
         }
         return _renderers[_loc5_] as IListItemRenderer;
      }
      
      override public function scrollToIndex(param1:uint) : void
      {
         if(_totalRenderers == 0)
         {
            return;
         }
         var _loc2_:Number = 1;
         var _loc3_:Number = _scrollPosition * _loc2_;
         if(param1 >= _loc3_ && param1 < _loc3_ + _rowCount)
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
            _scrollTarget = this._pow * (Math.floor(param1 / _loc2_) - _rowCount + 1);
            this.scrollList(0);
         }
         else if(_scrollBar)
         {
            _scrollBar.position = this._pow * (Math.floor(param1 / _loc2_) - _rowCount + 1);
         }
         else
         {
            scrollPosition = this._pow * (Math.floor(param1 / _loc2_) - _rowCount + 1);
         }
      }
      
      override protected function draw() : void
      {
         var _loc1_:Number;
         var _loc2_:Number;
         var _loc3_:DisplayObject;
         if(isInvalid(InvalidationType.DATA))
         {
            _loc1_ = Math.ceil(_dataProvider.length - _rowCount) + 1;
            if(_scrollPosition != _loc1_ && this._delta == this._pow)
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
               _loc2_ = 0;
            }
            else
            {
               _loc2_ = height / scaleY;
            }
            this._mask.graphics.drawRect(0,0,width / scaleX,_loc2_);
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
            _loc3_ = _scrollBar as DisplayObject;
            if(_loc3_ && container.contains(_loc3_))
            {
               container.addChild(_loc3_);
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
         var _loc1_:int = _selectedIndex;
         var _loc2_:IListItemRenderer = this.getRendererAt(_selectedIndex,_scrollPosition);
         var _loc3_:Boolean;
         if(_loc2_ != null)
         {
            _loc2_.selected = false;
            _loc2_.validateNow();
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
         _loc2_ = this.getRendererAt(_selectedIndex,_scrollPosition);
         if(_loc2_ != null)
         {
            _loc3_ = _loc2_.y + rowHeight > (_rowCount - 1) * rowHeight;
            if(_loc3_)
            {
               if(this._tween)
               {
                  this._tween.paused = true;
               }
               _scrollTarget = _selectedIndex - (_renderers.length - this._toMove * 2) * this._pow;
               this.scrollToIndex(_selectedIndex - (_renderers.length - this._toMove * 2));
            }
            else if(_loc2_.y < 0 || _loc2_.x < 0)
            {
               this.scrollToIndex(_selectedIndex);
            }
            _loc2_.selected = true;
            _loc2_.validateNow();
         }
         else
         {
            if(_loc1_ < _selectedIndex)
            {
               this.scrollToIndex(_selectedIndex + 1);
            }
            else
            {
               this.scrollToIndex(_selectedIndex);
            }
            if(_loc2_)
            {
               _loc2_ = this.getRendererAt(_selectedIndex,_scrollPosition);
               _loc2_.selected = true;
               _loc2_.validateNow();
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
         _loc1_ = Math.ceil(_dataProvider.length) - _rowCount;
         if(_scrollBar is ScrollIndicator)
         {
            _loc2_ = _scrollBar as ScrollIndicator;
            (_scrollBar as ScrollBar).trackScrollPageSize = this._pow;
            _loc2_.setScrollProperties(_rowCount * this._pow,0,(_loc1_ + 1) * this._pow,this._pow);
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
         else if(this._lastIdex)
         {
            scrollPosition = _scrollBar.position;
         }
         _scrollBar.validateNow();
      }
      
      override protected function scrollList(param1:int) : void
      {
         trace("scrollList::",param1,this._scrollTarget);
         var _loc2_:Number = this._pow * this.getMexScrollPosition();
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
         var _loc1_:ScrollIndicator = _scrollBar as ScrollIndicator;
         _loc1_.rotation = 0;
         _loc1_.x = _width - _loc1_.width + margin;
         _loc1_.y = margin;
         _loc1_.height = int(_rowCount - 1) * rowHeight;
         _scrollBar.validateNow();
      }
      
      override protected function createScrollBar() : void
      {
         var _loc1_:ScrollBar = _scrollBar as ScrollBar;
         if(_scrollBar)
         {
            _scrollBar.removeEventListener(Event.COMPLETE,this.onSBarDragEnd);
            _scrollBar.removeEventListener(Event.SELECT,this.onSBarDragStart);
            if(_loc1_.track)
            {
               _loc1_.track.removeEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
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
         _loc1_ = _scrollBar as ScrollBar;
         if(_scrollBar)
         {
            _scrollBar.addEventListener(Event.COMPLETE,this.onSBarDragEnd);
            _scrollBar.addEventListener(Event.SELECT,this.onSBarDragStart);
            if(_loc1_.track)
            {
               _loc1_.track.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress);
            }
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
         var _loc4_:IListItemRenderer;
         if(_loc3_)
         {
            _loc4_ = createRenderer(0);
            if(_loc3_)
            {
               _autoRowHeight = _loc4_.height;
            }
            cleanUpRenderer(_loc4_);
         }
         _rowCount = 1 + Math.ceil(availableHeight / rowHeight);
         _totalRenderers = _rowCount;
         _toMove = 1;
         return _totalRenderers;
      }
      
      override protected function refreshData() : void
      {
         var _loc1_:uint = Math.min(Math.max(0,_dataProvider.length - _totalRenderers),_scrollPosition);
         _scrollPosition = _loc1_;
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
         this.updateSelectedIndex();
         _dataProvider.requestItemRange(_loc1_,Math.min(_dataProvider.length - 1,_loc1_ + _totalRenderers - 1),this.populateData);
         this.updateRenderers();
      }
      
      override protected function populateData(param1:Array) : void
      {
         var _loc2_:uint = param1.length;
         var _loc3_:uint = _renderers.length;
         var _loc4_:uint = 0;
         var _loc5_:IListItemRenderer;
         var _loc6_:uint;
         var _loc7_:ListData;
         while(_loc4_ < _loc3_)
         {
            _loc5_ = this.getRendererAt(_loc4_);
            _loc6_ = _scrollPosition + _loc4_;
            _loc7_ = new ListData(_loc6_,itemToLabel(param1[_loc4_]),_selectedIndex == _loc6_);
            _loc5_.enabled = _loc4_ >= _loc2_ ? false : true;
            _loc5_.setListData(_loc7_);
            _loc5_.setData(param1[_loc4_]);
            _loc5_.validateNow();
            _loc4_++;
         }
      }
      
      protected function getMexScrollPosition() : int
      {
         return Math.ceil(_dataProvider.length - _rowCount) + 1;
      }
      
      protected function updateRenderers() : void
      {
         var _loc1_:uint = _renderers.length;
         var _loc2_:int = 0;
         var _loc3_:IListItemRenderer;
         while(_loc2_ < _loc1_)
         {
            _loc3_ = this.getRendererAt(_loc2_);
            _loc3_.y = _loc2_ * _rowHeight + margin - _rowHeight / this._pow * this._delta - this._rowScroll;
            _loc2_++;
         }
      }
      
      protected function goDrag(param1:MouseEvent) : void
      {
         var _loc2_:int;
         var _loc3_:int;
         if(this._dragY == mouseY)
         {
            return;
         }
         _loc3_ = _rowHeight / this._pow;
         var _loc4_:int = int(mouseY * scaleY / _loc3_) * _loc3_;
         var _loc5_:Number;
         _loc2_ = int((this._dragY - _loc4_) / _loc3_) * _loc3_;
         if(_loc2_ == 0)
         {
            return;
         }
         _dragY = _loc4_;
         if(_scrollBar)
         {
            _scrollBar.position += _loc2_ / _loc3_;
         }
         else
         {
            _loc5_ = this._pow * Math.ceil(1 + (_dataProvider.length - _rowCount));
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

