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
      
      public function set rowStep(value:Number) : void
      {
         _rowStep = value;
         invalidateSize();
      }
      
      override protected function draw() : void
      {
         var maskHeight:Number;
         if(isInvalid(InvalidationType.SCROLL_BAR) || isInvalid(InvalidationType.SIZE))
         {
            _mask.graphics.clear();
            _mask.graphics.beginFill(0,0.2);
            if(height == 0 || scaleY == 0)
            {
               maskHeight = 0;
            }
            else
            {
               maskHeight = height / scaleY;
            }
            _mask.graphics.drawRect(0,0,width / scaleX,maskHeight);
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
         var indicator:ScrollIndicator;
         var maxScroll:Number;
         if(_scrollBar == null)
         {
            return;
         }
         if(_scrollBar is ScrollIndicator)
         {
            indicator = _scrollBar as ScrollIndicator;
            (_scrollBar as ScrollBar).trackScrollPageSize = _pow;
            maxScroll = Math.max(0,Math.ceil(_dataProvider.length) - _rowCount);
            _maxScrollPosition = (maxScroll + this._extraRows) * _pow;
            indicator.setScrollProperties(_rowCount * _pow,0,this._maxScrollPosition,_pow);
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
         var indicator:ScrollIndicator = _scrollBar as ScrollIndicator;
         indicator.rotation = 0;
         indicator.x = _width - indicator.width + margin;
         indicator.y = margin;
         indicator.height = _mask ? height - margin * 2 : int(_rowCount - 1) * rowHeight;
         _scrollBar.validateNow();
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
         _rowCount = 1 + Math.ceil(availableHeight / (rowHeight + this._rowStep));
         _totalRenderers = _rowCount;
         _toMove = 1;
         return _totalRenderers;
      }
      
      override protected function getMexScrollPosition() : int
      {
         var maxScroll:int = (Math.max(0,_dataProvider.length - _rowCount) + this._extraRows + 1);
         return maxScroll;
      }
      
      override protected function refreshData() : void
      {
         var startIndex:uint;
         var endIndex:int;
         startIndex = Math.min(Math.max(0,_dataProvider.length - _totalRenderers) + this._extraRows + 1,_scrollPosition);
         endIndex = Math.min(_dataProvider.length - 1,startIndex + _totalRenderers + this._extraRows + 1);
         _scrollPosition = startIndex;
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
         updateSelectedIndex();
         _dataProvider.requestItemRange(startIndex,endIndex,populateData);
         this.calculateScrollStep();
         this.updateRenderers();
      }
      
      override protected function updateRenderers() : void
      {
         var nextY:int = 0;
         var i:int;
         var renderer:IListItemRenderer;
         var rendererClip:DisplayObject;
         var rowSpan:Number;
         if(_renderers)
         {
            i = 0;
            while(i < _renderers.length)
            {
               renderer = getRendererAt(i);
               rendererClip = renderer as DisplayObject;
               renderer.y = nextY - this._elementStep * _delta;
               rowSpan = (renderer as DisplayObject).height + this._rowStep;
               nextY += rowSpan;
               i++;
            }
         }
      }
      
      override protected function drawLayout() : void
      {
         var rendererCount:uint = _renderers.length;
         var startX:Number = margin + padding.left;
         var dataInvalid:Boolean = isInvalid(InvalidationType.DATA);
         var i:uint = 0;
         var renderer:IListItemRenderer;
         while(i < rendererCount)
         {
            renderer = getRendererAt(i);
            renderer.x = startX;
            if(!dataInvalid)
            {
               renderer.validateNow();
            }
            i++;
         }
         this.drawScrollBar();
      }
      
      protected function calculateAdditionalRows() : void
      {
         _extraRows = 0;
         var visibleRows:int = Math.ceil(availableHeight / (_rowHeight + this._rowStep));
         var firstIndex:int = _dataProvider.length - visibleRows - 1;
         if(firstIndex < 0)
         {
            firstIndex = 0;
         }
         var renderer:IListItemRenderer = createRenderer(0);
         var totalHeight:uint = 0;
         var i:int = _dataProvider.length - 1;
         var rowSpan:Number;
         while(i >= firstIndex)
         {
            renderer.setData(_dataProvider[i]);
            renderer.validateNow();
            rowSpan = (renderer as DisplayObject).height + this._rowStep;
            totalHeight += rowSpan;
            if(totalHeight > availableHeight)
            {
               _extraRows += totalHeight - rowSpan > availableHeight ? 1 : (totalHeight - availableHeight) / rowSpan;
            }
            i--;
         }
         cleanUpRenderer(renderer);
      }
      
      protected function calculateScrollStep() : void
      {
         var firstHeight:Number = (_renderers[0] as DisplayObject).height;
         var rowSpan:int = firstHeight + this._rowStep;
         _elementStep = rowSpan / _pow;
      }
   }
}

