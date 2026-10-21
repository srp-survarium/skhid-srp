package scaleform.clik.controls
{
   import flash.display.DisplayObject;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.constants.DirectionMode;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.constants.WrappingMode;
   import scaleform.clik.data.ListData;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.interfaces.IListItemRenderer;
   import scaleform.clik.interfaces.IScrollBar;
   import scaleform.clik.ui.InputDetails;
   import scaleform.clik.utils.Padding;
   
   [Event(name="change",type="flash.events.Event")]
   [Event(name="itemClick",type="scaleform.clik.events.ListEvent")]
   [Event(name="itemPress",type="scaleform.clik.events.ListEvent")]
   [Event(name="itemRollOver",type="scaleform.clik.events.ListEvent")]
   [Event(name="itemRollOut",type="scaleform.clik.events.ListEvent")]
   [Event(name="itemDoubleClick",type="scaleform.clik.events.ListEvent")]
   public class TileList extends CoreList
   {
      
      [Inspectable(defaultValue="normal",enumeration="normal,wrap,stick")]
      public var wrapping:String = "normal";
      
      public var thumbOffset:Object;
      
      public var thumbSizeFactor:Number = 1;
      
      [Inspectable(defaultValue="0")]
      public var externalColumnCount:Number = 0;
      
      protected var _rowHeight:Number = NaN;
      
      protected var _autoRowHeight:Number = NaN;
      
      protected var _totalRows:Number = 0;
      
      protected var _columnWidth:Number = NaN;
      
      protected var _autoColumnWidth:Number = NaN;
      
      protected var _totalColumns:uint = 0;
      
      protected var _scrollPosition:uint = 0;
      
      protected var _autoScrollBar:Boolean = false;
      
      protected var _scrollBarValue:Object;
      
      protected var _margin:Number = 0;
      
      protected var _padding:Padding;
      
      protected var _direction:String = "horizontal";
      
      protected var _siWidth:Number = 0;
      
      protected var _noIndex:Boolean = false;
      
      protected var _scrollBar:IScrollBar;
      
      public function TileList()
      {
         super();
      }
      
      override protected function initialize() : void
      {
         super.initialize();
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
      
      [Inspectable(defaultValue="0")]
      public function get rowHeight() : Number
      {
         return isNaN(this._autoRowHeight) ? this._rowHeight : this._autoRowHeight;
      }
      
      public function set rowHeight(value:Number) : void
      {
         if(value == 0)
         {
            value = NaN;
            if(_inspector)
            {
               return;
            }
         }
         _rowHeight = value;
         _autoRowHeight = NaN;
         invalidateSize();
      }
      
      [Inspectable(defaultValue="0")]
      public function get columnWidth() : Number
      {
         return isNaN(this._autoColumnWidth) ? this._columnWidth : this._autoColumnWidth;
      }
      
      public function set columnWidth(value:Number) : void
      {
         if(value == 0)
         {
            value = NaN;
            if(_inspector)
            {
               return;
            }
         }
         _columnWidth = value;
         _autoColumnWidth = NaN;
         invalidateSize();
      }
      
      public function get rowCount() : uint
      {
         return _totalRenderers;
      }
      
      public function set rowCount(value:uint) : void
      {
         var h:Number = this.rowHeight;
         if(isNaN(h))
         {
            this.calculateRendererTotal(this.availableWidth,this.availableHeight);
            h = this.rowHeight;
         }
         height = h * value + this.margin * 2 + this.padding.vertical + (this._direction == DirectionMode.HORIZONTAL && this._autoScrollBar ? Math.round(this._siWidth) : 0);
         _totalRows = value;
      }
      
      public function get columnCount() : uint
      {
         return this._totalColumns;
      }
      
      public function set columnCount(value:uint) : void
      {
         var w:Number = this.columnWidth;
         if(isNaN(w))
         {
            this.calculateRendererTotal(this.availableWidth,this.availableHeight);
            w = this.columnWidth;
         }
         width = w * value + this.margin * 2 + this.padding.horizontal + (this._direction == DirectionMode.VERTICAL && this._autoScrollBar ? Math.round(this._siWidth) : 0);
      }
      
      [Inspectable(type="List",enumeration="horizontal,vertical",defaultValue="horizontal")]
      public function get direction() : String
      {
         return this._direction;
      }
      
      public function set direction(value:String) : void
      {
         if(value == this._direction)
         {
            return;
         }
         _direction = value;
         invalidate();
      }
      
      override public function set selectedIndex(value:int) : void
      {
         if(value == _selectedIndex || value == _newSelectedIndex)
         {
            return;
         }
         _newSelectedIndex = value;
         invalidateSelectedIndex();
      }
      
      [Inspectable(defaultValue="0")]
      public function get margin() : Number
      {
         return this._margin;
      }
      
      public function set margin(value:Number) : void
      {
         _margin = value;
         invalidateSize();
      }
      
      public function get padding() : Padding
      {
         return this._padding;
      }
      
      public function set padding(value:Padding) : void
      {
         _padding = value;
         invalidateSize();
      }
      
      [Inspectable(name="padding",defaultValue="top:0,right:0,bottom:0,left:0")]
      public function set inspectablePadding(value:Object) : void
      {
         if(!componentInspectorSetting)
         {
            return;
         }
         padding = new Padding(value.top,value.right,value.bottom,value.left);
      }
      
      override public function get availableWidth() : Number
      {
         return Math.round(_width) - this.margin * 2 - (this._direction == DirectionMode.VERTICAL && this._autoScrollBar ? Math.round(this._siWidth) : 0);
      }
      
      override public function get availableHeight() : Number
      {
         return Math.round(_height) - this.margin * 2 - (this._direction == DirectionMode.HORIZONTAL && this._autoScrollBar ? Math.round(this._siWidth) : 0);
      }
      
      public function get scrollPosition() : Number
      {
         return this._scrollPosition;
      }
      
      public function set scrollPosition(value:Number) : void
      {
         var maxScrollPosition:Number = Math.ceil((_dataProvider.length - this._totalRows * this._totalColumns) / (this._direction == DirectionMode.HORIZONTAL ? this._totalRows : this._totalColumns));
         value = Math.max(0,Math.min(maxScrollPosition,Math.round(value)));
         if(this._scrollPosition == value)
         {
            return;
         }
         _scrollPosition = value;
         invalidateData();
      }
      
      [Inspectable(defaultValue="false")]
      public function get noIndexOnLostFocus() : Boolean
      {
         return this._noIndex;
      }
      
      public function set noIndexOnLostFocus(value:Boolean) : void
      {
      }
      
      override public function getRendererAt(index:uint, offset:int = 0) : IListItemRenderer
      {
         if(_renderers == null)
         {
            return null;
         }
         var rendererIndex:uint = index - offset * (this._direction == DirectionMode.HORIZONTAL ? this._totalRows : this._totalColumns);
         if(rendererIndex >= _renderers.length)
         {
            return null;
         }
         return _renderers[rendererIndex] as IListItemRenderer;
      }
      
      override public function scrollToIndex(index:uint) : void
      {
         if(_totalRenderers == 0)
         {
            return;
         }
         var factor:Number = this._direction == DirectionMode.HORIZONTAL ? this._totalRows : this._totalColumns;
         var startIndex:Number = this._scrollPosition * factor;
         if(factor == 0)
         {
            return;
         }
         if(index >= startIndex && index < startIndex + this._totalRows * this._totalColumns)
         {
            return;
         }
         if(index < startIndex)
         {
            scrollPosition = index / factor >> 0;
         }
         else
         {
            scrollPosition = Math.floor(index / factor) - (this._direction == DirectionMode.HORIZONTAL ? this._totalColumns : this._totalRows) + 1;
         }
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.handled)
         {
            return;
         }
         var renderer:IListItemRenderer = this.getRendererAt(_selectedIndex,this._scrollPosition);
         if(renderer != null)
         {
            renderer.handleInput(event);
            if(event.handled)
            {
               return;
            }
         }
         var details:InputDetails = event.details;
         var keyPress:Boolean = details.value == InputValue.KEY_DOWN || details.value == InputValue.KEY_HOLD;
         var nextIndex:int = -1;
         var nav:String = details.navEquivalent;
         if(this._direction == DirectionMode.HORIZONTAL)
         {
            switch(nav)
            {
               case NavigationCode.RIGHT:
                  nextIndex = _selectedIndex + this._totalRows;
                  break;
               case NavigationCode.LEFT:
                  nextIndex = _selectedIndex - this._totalRows;
                  break;
               case NavigationCode.UP:
                  nextIndex = _selectedIndex - 1;
                  break;
               case NavigationCode.DOWN:
                  nextIndex = _selectedIndex + 1;
                  break;
            }
         }
         else
         {
            switch(nav)
            {
               case NavigationCode.DOWN:
                  nextIndex = _selectedIndex + this._totalColumns;
                  break;
               case NavigationCode.UP:
                  nextIndex = _selectedIndex - this._totalColumns;
                  break;
               case NavigationCode.LEFT:
                  nextIndex = _selectedIndex - 1;
                  break;
               case NavigationCode.RIGHT:
                  nextIndex = _selectedIndex + 1;
                  break;
            }
         }
         if(nextIndex == -1)
         {
            switch(nav)
            {
               case NavigationCode.HOME:
                  nextIndex = 0;
                  break;
               case NavigationCode.END:
                  nextIndex = _dataProvider.length - 1;
                  break;
               case NavigationCode.PAGE_DOWN:
                  nextIndex = Math.min(_dataProvider.length - 1,_selectedIndex + this._totalColumns * this._totalRows);
                  break;
               case NavigationCode.PAGE_UP:
                  nextIndex = Math.max(0,_selectedIndex - this._totalColumns * this._totalRows);
                  break;
            }
         }
         if(nextIndex != -1)
         {
            if(!keyPress)
            {
               event.handled = true;
               return;
            }
            if(nextIndex >= 0 && nextIndex < dataProvider.length)
            {
               selectedIndex = Math.max(0,Math.min(_dataProvider.length - 1,nextIndex));
               event.handled = true;
            }
            else if(this.wrapping == WrappingMode.STICK)
            {
               nextIndex = Math.max(0,Math.min(_dataProvider.length - 1,nextIndex));
               if(selectedIndex != nextIndex)
               {
                  selectedIndex = nextIndex;
               }
               event.handled = true;
            }
            else if(this.wrapping == WrappingMode.WRAP)
            {
               selectedIndex = nextIndex < 0 ? (_dataProvider.length - 1) : (selectedIndex < _dataProvider.length - 1 ? (_dataProvider.length - 1) : 0);
               event.handled = true;
            }
         }
      }
      
      override public function toString() : String
      {
         return "[CLIK TileList " + name + "]";
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(this.padding == null)
         {
            padding = new Padding();
         }
         if(_itemRenderer == null && !_usingExternalRenderers)
         {
            itemRendererName = _itemRendererName;
         }
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.SCROLL_BAR))
         {
            this.createScrollBar();
         }
         if(isInvalid(InvalidationType.RENDERERS))
         {
            _autoRowHeight = NaN;
            _autoColumnWidth = NaN;
            if(_usingExternalRenderers)
            {
               _totalColumns = this.externalColumnCount == 0 ? 1 : (this.externalColumnCount);
               _totalRows = Math.ceil(_renderers.length / this._totalColumns);
            }
         }
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateScrollBar();
         }
      }
      
      protected function createScrollBar() : void
      {
         var sb:IScrollBar;
         var classRef:Class;
         var sbInst:Object;
         if(this._scrollBar)
         {
            this._scrollBar.removeEventListener(Event.SCROLL,this.handleScroll);
            this._scrollBar.removeEventListener(Event.CHANGE,this.handleScroll);
            this._scrollBar.focusTarget = null;
            if(container.contains(this._scrollBar as DisplayObject))
            {
               container.removeChild(this._scrollBar as DisplayObject);
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
                  sbInst = sb as Object;
                  if(sbInst && this.thumbOffset)
                  {
                     sbInst.offsetTop = this.thumbOffset.top;
                     sbInst.offsetBottom = this.thumbOffset.bottom;
                  }
                  sb.addEventListener(MouseEvent.MOUSE_WHEEL,this.blockMouseWheel,false,0,true);
                  container.addChild(sb as DisplayObject);
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
               (sb as Object).offsetTop = this.thumbOffset.top;
               (sb as Object).offsetBottom = this.thumbOffset.bottom;
               container.addChild(sb as DisplayObject);
            }
         }
         else
         {
            sb = this._scrollBarValue as IScrollBar;
         }
         _scrollBar = sb;
         _siWidth = this._scrollBar.width;
         invalidateSize();
         if(this._scrollBar == null)
         {
            return;
         }
         this._scrollBar.addEventListener(Event.SCROLL,this.handleScroll,false,0,true);
         this._scrollBar.addEventListener(Event.CHANGE,this.handleScroll,false,0,true);
         this._scrollBar.focusTarget = this;
         this._scrollBar.tabEnabled = false;
      }
      
      override protected function calculateRendererTotal(width:Number, height:Number) : uint
      {
         var invalidRowHeight:Boolean = isNaN(this._rowHeight) && isNaN(this._autoRowHeight);
         var invalidColumnWidth:Boolean = isNaN(this._columnWidth) && isNaN(this._autoColumnWidth);
         var renderer:IListItemRenderer;
         if(invalidRowHeight || invalidColumnWidth)
         {
            renderer = createRenderer(0);
            if(invalidRowHeight)
            {
               _autoRowHeight = renderer.height;
            }
            if(invalidColumnWidth)
            {
               _autoColumnWidth = renderer.width;
            }
            cleanUpRenderer(renderer);
         }
         _totalRows = this.availableHeight / this.rowHeight >> 0;
         _totalColumns = this.availableWidth / this.columnWidth >> 0;
         _totalRenderers = this._totalRows * this._totalColumns;
         return _totalRenderers;
      }
      
      override protected function updateSelectedIndex() : void
      {
         var renderer:IListItemRenderer;
         if(_selectedIndex == _newSelectedIndex)
         {
            if(!this._noIndex)
            {
               renderer = this.getRendererAt(_selectedIndex,this.scrollPosition);
               if(renderer != null)
               {
                  renderer.displayFocus = focused > 0;
                  renderer.validateNow();
               }
            }
            return;
         }
         if(_totalRenderers == 0)
         {
            return;
         }
         renderer = this.getRendererAt(_selectedIndex,this.scrollPosition);
         if(renderer != null)
         {
            if(renderer.selected == true)
            {
               renderer.selected = false;
               renderer.validateNow();
            }
         }
         super.selectedIndex = _newSelectedIndex;
         if(_selectedIndex < 0 || _selectedIndex >= _dataProvider.length)
         {
            return;
         }
         renderer = this.getRendererAt(_selectedIndex,this._scrollPosition);
         if(renderer != null)
         {
            renderer.selected = true;
            renderer.validateNow();
         }
         else
         {
            this.scrollToIndex(_selectedIndex);
            renderer = this.getRendererAt(_selectedIndex,this.scrollPosition);
            renderer.selected = true;
            renderer.validateNow();
         }
      }
      
      override protected function refreshData() : void
      {
         var itemsPerSet:Number = this._direction == DirectionMode.HORIZONTAL ? this._totalRows : this._totalColumns;
         var numberOfSets:Number = Math.ceil(_dataProvider.length / itemsPerSet);
         var maxScrollPosition:Number = numberOfSets - (this._direction == DirectionMode.HORIZONTAL ? this._totalColumns : this._totalRows);
         _scrollPosition = Math.max(0,Math.min(maxScrollPosition,this._scrollPosition));
         var startIndex:Number = this._scrollPosition * itemsPerSet;
         var endIndex:Number = startIndex + this._totalColumns * this._totalRows - 1;
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
         this.updateSelectedIndex();
         _dataProvider.requestItemRange(startIndex,endIndex,this.populateData);
      }
      
      override protected function drawLayout() : void
      {
         var l:uint = _renderers.length;
         var h:Number = this.rowHeight;
         var w:Number = this.columnWidth;
         var rx:Number = this.margin + this.padding.left;
         var ry:Number = this.margin + this.padding.top;
         var dataWillChange:Boolean = isInvalid(InvalidationType.DATA);
         var i:uint = 0;
         var renderer:IListItemRenderer;
         while(i < l)
         {
            renderer = this.getRendererAt(i);
            if(this.direction == DirectionMode.HORIZONTAL)
            {
               renderer.y = ry + i % this._totalRows * h + this.margin;
               renderer.x = rx + (i / this._totalRows >> 0) * w + this.margin;
            }
            else
            {
               renderer.x = rx + i % this._totalColumns * w + this.margin;
               renderer.y = ry + (i / this._totalColumns >> 0) * h + this.margin;
            }
            renderer.width = w;
            renderer.height = h;
            if(!dataWillChange)
            {
               renderer.validateNow();
            }
            i++;
         }
         this.drawScrollBar();
      }
      
      override protected function changeFocus() : void
      {
         if(_focused || _displayFocus)
         {
            setState("focused","default");
         }
         else
         {
            setState("default");
         }
         var renderer:IListItemRenderer = this.getRendererAt(_selectedIndex,this._scrollPosition);
         if(renderer != null)
         {
            if(this._noIndex)
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
      
      protected function populateData(data:Array) : void
      {
         var dl:uint = data.length;
         var l:uint = _renderers.length;
         var i:uint = 0;
         var renderer:IListItemRenderer;
         var index:uint;
         var listData:ListData;
         var _enable:Boolean;
         while(i < l)
         {
            renderer = this.getRendererAt(i);
            index = this._scrollPosition * (this._direction == DirectionMode.HORIZONTAL ? this._totalRows : this._totalColumns) + i;
            listData = new ListData(index,itemToLabel(data[i]),_selectedIndex == index);
            _enable = i >= dl ? false : true;
            if(renderer.enabled != _enable)
            {
               renderer.enabled = _enable;
            }
            renderer.setListData(listData);
            renderer.setData(data[i]);
            renderer.validateNow();
            i++;
         }
      }
      
      protected function drawScrollBar() : void
      {
         if(!this._autoScrollBar)
         {
            return;
         }
         var sb:ScrollIndicator = this._scrollBar as ScrollIndicator;
         sb.direction = this._direction;
         if(this._direction == DirectionMode.VERTICAL)
         {
            sb.rotation = 0;
            sb.x = _width - sb.width + this.margin;
            sb.y = this.margin;
            sb.height = this.availableHeight;
         }
         else
         {
            sb.rotation = -90;
            sb.x = this.margin;
            sb.y = _height - this.margin;
            sb.width = this.availableWidth;
         }
         this._scrollBar.validateNow();
      }
      
      protected function updateScrollBar() : void
      {
         var max:Number;
         var scrollIndicator:ScrollIndicator;
         if(this._scrollBar == null)
         {
            return;
         }
         if(this.direction == DirectionMode.HORIZONTAL)
         {
            max = Math.ceil(_dataProvider.length / this._totalRows) - this._totalColumns;
         }
         else
         {
            max = Math.ceil(_dataProvider.length / this._totalColumns) - this._totalRows;
         }
         if(this._scrollBar is ScrollIndicator)
         {
            scrollIndicator = this._scrollBar as ScrollIndicator;
            scrollIndicator.setScrollProperties(this._direction == DirectionMode.HORIZONTAL ? this._totalColumns : this._totalRows,0,max);
         }
         this._scrollBar.position = this._scrollPosition;
         this._scrollBar.validateNow();
      }
      
      protected function handleScroll(event:Event) : void
      {
         scrollPosition = this._scrollBar.position;
      }
      
      override protected function scrollList(delta:int) : void
      {
         scrollPosition -= delta;
      }
      
      protected function blockMouseWheel(event:MouseEvent) : void
      {
         event.stopPropagation();
      }
   }
}

