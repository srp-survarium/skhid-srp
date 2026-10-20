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
      
      public function set scrollBar(param1:Object) : void
      {
         _scrollBarValue = param1;
         invalidate(InvalidationType.SCROLL_BAR);
      }
      
      [Inspectable(defaultValue="0")]
      public function get rowHeight() : Number
      {
         return isNaN(this._autoRowHeight) ? this._rowHeight : this._autoRowHeight;
      }
      
      public function set rowHeight(param1:Number) : void
      {
         if(param1 == 0)
         {
            param1 = NaN;
            if(_inspector)
            {
               return;
            }
         }
         _rowHeight = param1;
         _autoRowHeight = NaN;
         invalidateSize();
      }
      
      [Inspectable(defaultValue="0")]
      public function get columnWidth() : Number
      {
         return isNaN(this._autoColumnWidth) ? this._columnWidth : this._autoColumnWidth;
      }
      
      public function set columnWidth(param1:Number) : void
      {
         if(param1 == 0)
         {
            param1 = NaN;
            if(_inspector)
            {
               return;
            }
         }
         _columnWidth = param1;
         _autoColumnWidth = NaN;
         invalidateSize();
      }
      
      public function get rowCount() : uint
      {
         return _totalRenderers;
      }
      
      public function set rowCount(param1:uint) : void
      {
         var _loc2_:Number = this.rowHeight;
         if(isNaN(_loc2_))
         {
            this.calculateRendererTotal(this.availableWidth,this.availableHeight);
            _loc2_ = this.rowHeight;
         }
         height = _loc2_ * param1 + this.margin * 2 + this.padding.vertical + (this._direction == DirectionMode.HORIZONTAL && this._autoScrollBar ? Math.round(this._siWidth) : 0);
         _totalRows = param1;
      }
      
      public function get columnCount() : uint
      {
         return this._totalColumns;
      }
      
      public function set columnCount(param1:uint) : void
      {
         var _loc2_:Number = this.columnWidth;
         if(isNaN(_loc2_))
         {
            this.calculateRendererTotal(this.availableWidth,this.availableHeight);
            _loc2_ = this.columnWidth;
         }
         width = _loc2_ * param1 + this.margin * 2 + this.padding.horizontal + (this._direction == DirectionMode.VERTICAL && this._autoScrollBar ? Math.round(this._siWidth) : 0);
      }
      
      [Inspectable(type="List",enumeration="horizontal,vertical",defaultValue="horizontal")]
      public function get direction() : String
      {
         return this._direction;
      }
      
      public function set direction(param1:String) : void
      {
         if(param1 == this._direction)
         {
            return;
         }
         _direction = param1;
         invalidate();
      }
      
      override public function set selectedIndex(param1:int) : void
      {
         if(param1 == _selectedIndex || param1 == _newSelectedIndex)
         {
            return;
         }
         _newSelectedIndex = param1;
         invalidateSelectedIndex();
      }
      
      [Inspectable(defaultValue="0")]
      public function get margin() : Number
      {
         return this._margin;
      }
      
      public function set margin(param1:Number) : void
      {
         _margin = param1;
         invalidateSize();
      }
      
      public function get padding() : Padding
      {
         return this._padding;
      }
      
      public function set padding(param1:Padding) : void
      {
         _padding = param1;
         invalidateSize();
      }
      
      [Inspectable(name="padding",defaultValue="top:0,right:0,bottom:0,left:0")]
      public function set inspectablePadding(param1:Object) : void
      {
         if(!componentInspectorSetting)
         {
            return;
         }
         padding = new Padding(param1.top,param1.right,param1.bottom,param1.left);
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
      
      public function set scrollPosition(param1:Number) : void
      {
         var _loc2_:Number = Math.ceil((_dataProvider.length - this._totalRows * this._totalColumns) / (this._direction == DirectionMode.HORIZONTAL ? this._totalRows : this._totalColumns));
         param1 = Math.max(0,Math.min(_loc2_,Math.round(param1)));
         if(this._scrollPosition == param1)
         {
            return;
         }
         _scrollPosition = param1;
         invalidateData();
      }
      
      [Inspectable(defaultValue="false")]
      public function get noIndexOnLostFocus() : Boolean
      {
         return this._noIndex;
      }
      
      public function set noIndexOnLostFocus(param1:Boolean) : void
      {
      }
      
      override public function getRendererAt(param1:uint, param2:int = 0) : IListItemRenderer
      {
         if(_renderers == null)
         {
            return null;
         }
         var _loc3_:uint = param1 - param2 * (this._direction == DirectionMode.HORIZONTAL ? this._totalRows : this._totalColumns);
         if(_loc3_ >= _renderers.length)
         {
            return null;
         }
         return _renderers[_loc3_] as IListItemRenderer;
      }
      
      override public function scrollToIndex(param1:uint) : void
      {
         if(_totalRenderers == 0)
         {
            return;
         }
         var _loc2_:Number = this._direction == DirectionMode.HORIZONTAL ? this._totalRows : this._totalColumns;
         var _loc3_:Number = this._scrollPosition * _loc2_;
         if(_loc2_ == 0)
         {
            return;
         }
         if(param1 >= _loc3_ && param1 < _loc3_ + this._totalRows * this._totalColumns)
         {
            return;
         }
         if(param1 < _loc3_)
         {
            scrollPosition = param1 / _loc2_ >> 0;
         }
         else
         {
            scrollPosition = Math.floor(param1 / _loc2_) - (this._direction == DirectionMode.HORIZONTAL ? this._totalColumns : this._totalRows) + 1;
         }
      }
      
      override public function handleInput(param1:InputEvent) : void
      {
         if(param1.handled)
         {
            return;
         }
         var _loc2_:IListItemRenderer = this.getRendererAt(_selectedIndex,this._scrollPosition);
         if(_loc2_ != null)
         {
            _loc2_.handleInput(param1);
            if(param1.handled)
            {
               return;
            }
         }
         var _loc3_:InputDetails = param1.details;
         var _loc4_:Boolean = _loc3_.value == InputValue.KEY_DOWN || _loc3_.value == InputValue.KEY_HOLD;
         var _loc5_:int = -1;
         var _loc6_:String = _loc3_.navEquivalent;
         if(this._direction == DirectionMode.HORIZONTAL)
         {
            switch(_loc6_)
            {
               case NavigationCode.RIGHT:
                  _loc5_ = _selectedIndex + this._totalRows;
                  break;
               case NavigationCode.LEFT:
                  _loc5_ = _selectedIndex - this._totalRows;
                  break;
               case NavigationCode.UP:
                  _loc5_ = _selectedIndex - 1;
                  break;
               case NavigationCode.DOWN:
                  _loc5_ = _selectedIndex + 1;
                  break;
            }
         }
         else
         {
            switch(_loc6_)
            {
               case NavigationCode.DOWN:
                  _loc5_ = _selectedIndex + this._totalColumns;
                  break;
               case NavigationCode.UP:
                  _loc5_ = _selectedIndex - this._totalColumns;
                  break;
               case NavigationCode.LEFT:
                  _loc5_ = _selectedIndex - 1;
                  break;
               case NavigationCode.RIGHT:
                  _loc5_ = _selectedIndex + 1;
                  break;
            }
         }
         if(_loc5_ == -1)
         {
            switch(_loc6_)
            {
               case NavigationCode.HOME:
                  _loc5_ = 0;
                  break;
               case NavigationCode.END:
                  _loc5_ = _dataProvider.length - 1;
                  break;
               case NavigationCode.PAGE_DOWN:
                  _loc5_ = Math.min(_dataProvider.length - 1,_selectedIndex + this._totalColumns * this._totalRows);
                  break;
               case NavigationCode.PAGE_UP:
                  _loc5_ = Math.max(0,_selectedIndex - this._totalColumns * this._totalRows);
                  break;
            }
         }
         if(_loc5_ != -1)
         {
            if(!_loc4_)
            {
               param1.handled = true;
               return;
            }
            if(_loc5_ >= 0 && _loc5_ < dataProvider.length)
            {
               selectedIndex = Math.max(0,Math.min(_dataProvider.length - 1,_loc5_));
               param1.handled = true;
            }
            else if(this.wrapping == WrappingMode.STICK)
            {
               _loc5_ = Math.max(0,Math.min(_dataProvider.length - 1,_loc5_));
               if(selectedIndex != _loc5_)
               {
                  selectedIndex = _loc5_;
               }
               param1.handled = true;
            }
            else if(this.wrapping == WrappingMode.WRAP)
            {
               selectedIndex = _loc5_ < 0 ? (_dataProvider.length - 1) : (selectedIndex < _dataProvider.length - 1 ? (_dataProvider.length - 1) : 0);
               param1.handled = true;
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
         var _loc1_:IScrollBar;
         var _loc2_:Class;
         var _loc3_:Object;
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
               _loc1_ = parent.getChildByName(this._scrollBarValue.toString()) as IScrollBar;
            }
            if(_loc1_ == null)
            {
               _loc2_ = getDefinitionByName(this._scrollBarValue.toString()) as Class;
               if(_loc2_)
               {
                  _loc1_ = new _loc2_() as IScrollBar;
               }
               if(_loc1_)
               {
                  _autoScrollBar = true;
                  _loc3_ = _loc1_ as Object;
                  if(_loc3_ && this.thumbOffset)
                  {
                     _loc3_.offsetTop = this.thumbOffset.top;
                     _loc3_.offsetBottom = this.thumbOffset.bottom;
                  }
                  _loc1_.addEventListener(MouseEvent.MOUSE_WHEEL,this.blockMouseWheel,false,0,true);
                  container.addChild(_loc1_ as DisplayObject);
               }
            }
         }
         else if(this._scrollBarValue is Class)
         {
            _loc1_ = new (this._scrollBarValue as Class)() as IScrollBar;
            _loc1_.addEventListener(MouseEvent.MOUSE_WHEEL,this.blockMouseWheel,false,0,true);
            if(_loc1_ != null)
            {
               _autoScrollBar = true;
               (_loc1_ as Object).offsetTop = this.thumbOffset.top;
               (_loc1_ as Object).offsetBottom = this.thumbOffset.bottom;
               container.addChild(_loc1_ as DisplayObject);
            }
         }
         else
         {
            _loc1_ = this._scrollBarValue as IScrollBar;
         }
         _scrollBar = _loc1_;
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
      
      override protected function calculateRendererTotal(param1:Number, param2:Number) : uint
      {
         var _loc3_:Boolean = isNaN(this._rowHeight) && isNaN(this._autoRowHeight);
         var _loc4_:Boolean = isNaN(this._columnWidth) && isNaN(this._autoColumnWidth);
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
         _totalRows = this.availableHeight / this.rowHeight >> 0;
         _totalColumns = this.availableWidth / this.columnWidth >> 0;
         _totalRenderers = this._totalRows * this._totalColumns;
         return _totalRenderers;
      }
      
      override protected function updateSelectedIndex() : void
      {
         var _loc1_:IListItemRenderer;
         if(_selectedIndex == _newSelectedIndex)
         {
            if(!this._noIndex)
            {
               _loc1_ = this.getRendererAt(_selectedIndex,this.scrollPosition);
               if(_loc1_ != null)
               {
                  _loc1_.displayFocus = focused > 0;
                  _loc1_.validateNow();
               }
            }
            return;
         }
         if(_totalRenderers == 0)
         {
            return;
         }
         _loc1_ = this.getRendererAt(_selectedIndex,this.scrollPosition);
         if(_loc1_ != null)
         {
            if(_loc1_.selected == true)
            {
               _loc1_.selected = false;
               _loc1_.validateNow();
            }
         }
         super.selectedIndex = _newSelectedIndex;
         if(_selectedIndex < 0 || _selectedIndex >= _dataProvider.length)
         {
            return;
         }
         _loc1_ = this.getRendererAt(_selectedIndex,this._scrollPosition);
         if(_loc1_ != null)
         {
            _loc1_.selected = true;
            _loc1_.validateNow();
         }
         else
         {
            this.scrollToIndex(_selectedIndex);
            _loc1_ = this.getRendererAt(_selectedIndex,this.scrollPosition);
            _loc1_.selected = true;
            _loc1_.validateNow();
         }
      }
      
      override protected function refreshData() : void
      {
         var _loc1_:Number = this._direction == DirectionMode.HORIZONTAL ? this._totalRows : this._totalColumns;
         var _loc2_:Number = Math.ceil(_dataProvider.length / _loc1_);
         var _loc3_:Number = _loc2_ - (this._direction == DirectionMode.HORIZONTAL ? this._totalColumns : this._totalRows);
         _scrollPosition = Math.max(0,Math.min(_loc3_,this._scrollPosition));
         var _loc4_:Number = this._scrollPosition * _loc1_;
         var _loc5_:Number = _loc4_ + this._totalColumns * this._totalRows - 1;
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
         this.updateSelectedIndex();
         _dataProvider.requestItemRange(_loc4_,_loc5_,this.populateData);
      }
      
      override protected function drawLayout() : void
      {
         var _loc1_:uint = _renderers.length;
         var _loc2_:Number = this.rowHeight;
         var _loc3_:Number = this.columnWidth;
         var _loc4_:Number = this.margin + this.padding.left;
         var _loc5_:Number = this.margin + this.padding.top;
         var _loc6_:Boolean = isInvalid(InvalidationType.DATA);
         var _loc7_:uint = 0;
         var _loc8_:IListItemRenderer;
         while(_loc7_ < _loc1_)
         {
            _loc8_ = this.getRendererAt(_loc7_);
            if(this.direction == DirectionMode.HORIZONTAL)
            {
               _loc8_.y = _loc5_ + _loc7_ % this._totalRows * _loc2_ + this.margin;
               _loc8_.x = _loc4_ + (_loc7_ / this._totalRows >> 0) * _loc3_ + this.margin;
            }
            else
            {
               _loc8_.x = _loc4_ + _loc7_ % this._totalColumns * _loc3_ + this.margin;
               _loc8_.y = _loc5_ + (_loc7_ / this._totalColumns >> 0) * _loc2_ + this.margin;
            }
            _loc8_.width = _loc3_;
            _loc8_.height = _loc2_;
            if(!_loc6_)
            {
               _loc8_.validateNow();
            }
            _loc7_++;
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
         var _loc1_:IListItemRenderer = this.getRendererAt(_selectedIndex,this._scrollPosition);
         if(_loc1_ != null)
         {
            if(this._noIndex)
            {
               if(focused > 0)
               {
                  _loc1_.displayFocus = true;
                  _loc1_.validateNow();
               }
               else
               {
                  _loc1_.displayFocus = false;
                  _loc1_.validateNow();
               }
            }
            else
            {
               _loc1_.displayFocus = focused > 0;
               _loc1_.validateNow();
            }
         }
      }
      
      protected function populateData(param1:Array) : void
      {
         var _loc2_:uint = param1.length;
         var _loc3_:uint = _renderers.length;
         var _loc4_:uint = 0;
         var _loc5_:IListItemRenderer;
         var _loc6_:uint;
         var _loc7_:ListData;
         var _loc8_:Boolean;
         while(_loc4_ < _loc3_)
         {
            _loc5_ = this.getRendererAt(_loc4_);
            _loc6_ = this._scrollPosition * (this._direction == DirectionMode.HORIZONTAL ? this._totalRows : this._totalColumns) + _loc4_;
            _loc7_ = new ListData(_loc6_,itemToLabel(param1[_loc4_]),_selectedIndex == _loc6_);
            _loc8_ = _loc4_ >= _loc2_ ? false : true;
            if(_loc5_.enabled != _loc8_)
            {
               _loc5_.enabled = _loc8_;
            }
            _loc5_.setListData(_loc7_);
            _loc5_.setData(param1[_loc4_]);
            _loc5_.validateNow();
            _loc4_++;
         }
      }
      
      protected function drawScrollBar() : void
      {
         if(!this._autoScrollBar)
         {
            return;
         }
         var _loc1_:ScrollIndicator = this._scrollBar as ScrollIndicator;
         _loc1_.direction = this._direction;
         if(this._direction == DirectionMode.VERTICAL)
         {
            _loc1_.rotation = 0;
            _loc1_.x = _width - _loc1_.width + this.margin;
            _loc1_.y = this.margin;
            _loc1_.height = this.availableHeight;
         }
         else
         {
            _loc1_.rotation = -90;
            _loc1_.x = this.margin;
            _loc1_.y = _height - this.margin;
            _loc1_.width = this.availableWidth;
         }
         this._scrollBar.validateNow();
      }
      
      protected function updateScrollBar() : void
      {
         var _loc1_:Number;
         var _loc2_:ScrollIndicator;
         if(this._scrollBar == null)
         {
            return;
         }
         if(this.direction == DirectionMode.HORIZONTAL)
         {
            _loc1_ = Math.ceil(_dataProvider.length / this._totalRows) - this._totalColumns;
         }
         else
         {
            _loc1_ = Math.ceil(_dataProvider.length / this._totalColumns) - this._totalRows;
         }
         if(this._scrollBar is ScrollIndicator)
         {
            _loc2_ = this._scrollBar as ScrollIndicator;
            _loc2_.setScrollProperties(this._direction == DirectionMode.HORIZONTAL ? this._totalColumns : this._totalRows,0,_loc1_);
         }
         this._scrollBar.position = this._scrollPosition;
         this._scrollBar.validateNow();
      }
      
      protected function handleScroll(param1:Event) : void
      {
         scrollPosition = this._scrollBar.position;
      }
      
      override protected function scrollList(param1:int) : void
      {
         scrollPosition -= param1;
      }
      
      protected function blockMouseWheel(param1:MouseEvent) : void
      {
         param1.stopPropagation();
      }
   }
}

