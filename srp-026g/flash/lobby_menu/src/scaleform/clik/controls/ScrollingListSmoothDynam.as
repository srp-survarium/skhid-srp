package scaleform.clik.controls
{
   import flash.display.DisplayObject;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.interfaces.IListItemRenderer;
   
   public class ScrollingListSmoothDynam extends ScrollingListSmooth
   {
      
      protected var _maxScrollPosition:uint = 0;
      
      protected var _elementStep:Number = 0;
      
      protected var _extraRows:Number = 0;
      
      protected var _rowStep:int = 0;
      
      public function ScrollingListSmoothDynam()
      {
         super();
      }
      
      [Inspectable(defaultValue="0")]
      public function get rowStep() : Number
      {
         return this._rowStep;
      }
      
      public function set rowStep(param1:Number) : void
      {
         _rowStep = param1;
         invalidateSize();
      }
      
      override protected function draw() : void
      {
         var _loc1_:Number;
         if(isInvalid(InvalidationType.SCROLL_BAR) || isInvalid(InvalidationType.SIZE))
         {
            _mask.graphics.clear();
            _mask.graphics.beginFill(0,0.2);
            if(height == 0 || scaleY == 0)
            {
               _loc1_ = 0;
            }
            else
            {
               _loc1_ = height / scaleY;
            }
            _mask.graphics.drawRect(0,0,width / scaleX,_loc1_);
            _mask.graphics.endFill();
            _mask.mouseChildren = false;
            _mask.mouseEnabled = false;
            if(this.mask != _mask)
            {
               this.mask = _mask;
               addChild(_mask);
            }
         }
         if(isInvalid(InvalidationType.SCROLL_BAR) || isInvalid(InvalidationType.DATA))
         {
            this.calculateAdditionalRows();
         }
         super.draw();
      }
      
      override protected function updateScrollBar() : void
      {
         var _loc1_:ScrollIndicator;
         var _loc2_:Number;
         if(_scrollBar == null)
         {
            return;
         }
         if(_scrollBar is ScrollIndicator)
         {
            _loc1_ = _scrollBar as ScrollIndicator;
            (_scrollBar as ScrollBar).trackScrollPageSize = _pow;
            _loc2_ = Math.max(0,Math.ceil(_dataProvider.length) - _rowCount);
            _maxScrollPosition = (_loc2_ + this._extraRows) * _pow;
            _loc1_.setScrollProperties(_rowCount * _pow,0,this._maxScrollPosition,_pow);
         }
         if(this._maxScrollPosition < 0)
         {
            _lastDelta = 0;
            _scrollBar.position = 0;
         }
         else if(this._maxScrollPosition < _scrollBar.position)
         {
            scrollPosition = this._maxScrollPosition;
            _scrollBar.position = this._maxScrollPosition;
         }
         _scrollBar.validateNow();
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
         _loc1_.height = _mask ? height - margin * 2 : int(_rowCount - 1) * rowHeight;
         _scrollBar.validateNow();
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
         _rowCount = 1 + Math.ceil(availableHeight / (rowHeight + this._rowStep));
         _totalRenderers = _rowCount;
         _toMove = 1;
         return _totalRenderers;
      }
      
      override protected function getMexScrollPosition() : int
      {
         var _loc1_:int = (Math.max(0,_dataProvider.length - _rowCount) + this._extraRows + 1);
         return _loc1_;
      }
      
      override protected function refreshData() : void
      {
         var _loc1_:uint;
         var _loc2_:int;
         _loc1_ = Math.min(Math.max(0,_dataProvider.length - _totalRenderers) + this._extraRows + 1,_scrollPosition);
         _loc2_ = Math.min(_dataProvider.length - 1,_loc1_ + _totalRenderers + this._extraRows + 1);
         _scrollPosition = _loc1_;
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
         updateSelectedIndex();
         _dataProvider.requestItemRange(_loc1_,_loc2_,populateData);
         this.calculateScrollStep();
         this.updateRenderers();
      }
      
      override protected function updateRenderers() : void
      {
         var _loc1_:int = 0;
         var _loc2_:int;
         var _loc3_:IListItemRenderer;
         var _loc4_:DisplayObject;
         var _loc5_:Number;
         if(_renderers)
         {
            _loc2_ = 0;
            while(_loc2_ < _renderers.length)
            {
               _loc3_ = getRendererAt(_loc2_);
               _loc4_ = _loc3_ as DisplayObject;
               _loc3_.y = _loc1_ - this._elementStep * _delta;
               _loc5_ = (_loc3_ as DisplayObject).height + this._rowStep;
               _loc1_ += _loc5_;
               _loc2_++;
            }
         }
      }
      
      override protected function drawLayout() : void
      {
         var _loc1_:uint = _renderers.length;
         var _loc2_:Number = margin + padding.left;
         var _loc3_:Boolean = isInvalid(InvalidationType.DATA);
         var _loc4_:uint = 0;
         var _loc5_:IListItemRenderer;
         while(_loc4_ < _loc1_)
         {
            _loc5_ = getRendererAt(_loc4_);
            _loc5_.x = _loc2_;
            if(!_loc3_)
            {
               _loc5_.validateNow();
            }
            _loc4_++;
         }
         this.drawScrollBar();
      }
      
      protected function calculateAdditionalRows() : void
      {
         _extraRows = 0;
         var _loc1_:int = Math.ceil(availableHeight / (_rowHeight + this._rowStep));
         var _loc2_:int = _dataProvider.length - _loc1_ - 1;
         if(_loc2_ < 0)
         {
            _loc2_ = 0;
         }
         var _loc3_:IListItemRenderer = createRenderer(0);
         var _loc4_:uint = 0;
         var _loc5_:int = _dataProvider.length - 1;
         var _loc6_:Number;
         while(_loc5_ >= _loc2_)
         {
            _loc3_.setData(_dataProvider[_loc5_]);
            _loc3_.validateNow();
            _loc6_ = (_loc3_ as DisplayObject).height + this._rowStep;
            _loc4_ += _loc6_;
            if(_loc4_ > availableHeight)
            {
               _extraRows += _loc4_ - _loc6_ > availableHeight ? 1 : (_loc4_ - availableHeight) / _loc6_;
            }
            _loc5_--;
         }
         cleanUpRenderer(_loc3_);
      }
      
      protected function calculateScrollStep() : void
      {
         var _loc1_:Number = (_renderers[0] as DisplayObject).height;
         var _loc2_:int = _loc1_ + this._rowStep;
         _elementStep = _loc2_ / _pow;
      }
   }
}

