package scaleform.clik.controls
{
   import flash.display.DisplayObject;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.utils.getDefinitionByName;
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
   public class ScrollingList extends CoreList
   {
      
      [Inspectable(enumeration="normal,stick,wrap",defaultValue="normal")]
      public var wrapping:String = "normal";
      
      public var thumbOffset:Object;
      
      public var thumbSizeFactor:Number = 1;
      
      protected var _rowHeight:Number = NaN;
      
      protected var _autoRowHeight:Number = NaN;
      
      protected var _rowCount:Number = NaN;
      
      protected var _scrollPosition:uint = 0;
      
      protected var _autoScrollBar:Boolean = false;
      
      protected var _scrollBarValue:Object;
      
      protected var _margin:Number = 0;
      
      protected var _padding:Padding;
      
      protected var _scrollBar:IScrollBar;
      
      public function ScrollingList()
      {
         super();
      }
      
      override protected function initialize() : void
      {
         super.initialize();
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
      
      public function get scrollPosition() : Number
      {
         return this._scrollPosition;
      }
      
      public function set scrollPosition(value:Number) : void
      {
         value = Math.max(0,Math.min(_dataProvider.length - _totalRenderers,Math.round(value)));
         if(this._scrollPosition == value)
         {
            return;
         }
         _scrollPosition = value;
         invalidateData();
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
      
      public function get rowCount() : uint
      {
         return _totalRenderers;
      }
      
      public function set rowCount(value:uint) : void
      {
         var h:Number = this.rowHeight;
         if(isNaN(this.rowHeight))
         {
            this.calculateRendererTotal(this.availableWidth,this.availableHeight);
         }
         h = this.rowHeight;
         height = h * value + this.margin * 2 + this.padding.top + this.padding.bottom;
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
      
      override public function get availableWidth() : Number
      {
         return Math.round(_width) - this.margin * 2 - (this._autoScrollBar ? Math.round(this._scrollBar.width) : 0);
      }
      
      override public function get availableHeight() : Number
      {
         return Math.round(_height) - this.margin * 2;
      }
      
      override public function scrollToIndex(index:uint) : void
      {
         if(_totalRenderers == 0)
         {
            return;
         }
         if(index >= this._scrollPosition && index < this._scrollPosition + _totalRenderers)
         {
            return;
         }
         if(index < this._scrollPosition)
         {
            scrollPosition = index;
         }
         else
         {
            scrollPosition = index - (_totalRenderers - 1);
         }
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.handled)
         {
            return;
         }
         var renderer:IListItemRenderer = getRendererAt(_selectedIndex,this._scrollPosition);
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
         switch(details.navEquivalent)
         {
            case NavigationCode.UP:
               if(selectedIndex == -1)
               {
                  if(keyPress)
                  {
                     selectedIndex = this.scrollPosition + _totalRenderers - 1;
                  }
               }
               else if(_selectedIndex > 0)
               {
                  if(keyPress)
                  {
                     --selectedIndex;
                  }
               }
               else
               {
                  if(this.wrapping == WrappingMode.STICK)
                  {
                  }
                  else if(this.wrapping == WrappingMode.WRAP)
                  {
                     if(keyPress)
                     {
                        selectedIndex = _dataProvider.length - 1;
                     }
                  }
                  else
                  {
                     return;
                  }
               }
               break;
            case NavigationCode.DOWN:
               if(_selectedIndex == -1)
               {
                  if(keyPress)
                  {
                     selectedIndex = this._scrollPosition;
                  }
               }
               else if(_selectedIndex < _dataProvider.length - 1)
               {
                  if(keyPress)
                  {
                     ++selectedIndex;
                  }
               }
               else
               {
                  if(this.wrapping == WrappingMode.STICK)
                  {
                  }
                  else if(this.wrapping == WrappingMode.WRAP)
                  {
                     if(keyPress)
                     {
                        selectedIndex = 0;
                     }
                  }
                  else
                  {
                     return;
                  }
               }
               break;
            case NavigationCode.END:
               if(!keyPress)
               {
                  selectedIndex = _dataProvider.length - 1;
               }
               break;
            case NavigationCode.HOME:
               if(!keyPress)
               {
                  selectedIndex = 0;
               }
               break;
            case NavigationCode.PAGE_UP:
               if(keyPress)
               {
                  selectedIndex = Math.max(0,_selectedIndex - _totalRenderers);
               }
               break;
            case NavigationCode.PAGE_DOWN:
               if(keyPress)
               {
                  selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex + _totalRenderers);
               }
               break;
            default:
               return;
         }
         event.handled = true;
      }
      
      override public function toString() : String
      {
         return "[CLIK ScrollingList " + name + "]";
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
         }
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateScrollBar();
         }
      }
      
      override protected function drawLayout() : void
      {
         var l:uint = _renderers.length;
         var h:Number = this.rowHeight;
         var w:Number = this.availableWidth - this.padding.horizontal;
         var rx:Number = this.margin + this.padding.left;
         var ry:Number = this.margin + this.padding.top;
         var dataWillChange:Boolean = isInvalid(InvalidationType.DATA);
         var i:uint = 0;
         var renderer:IListItemRenderer;
         while(i < l)
         {
            renderer = getRendererAt(i);
            renderer.x = rx;
            renderer.y = ry + i * h;
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
      
      protected function createScrollBar() : void
      {
         var sb:IScrollBar;
         var classRef:Class;
         var sbInst:Object;
         if(this._scrollBar)
         {
            this._scrollBar.removeEventListener(Event.SCROLL,this.handleScroll,false);
            this._scrollBar.removeEventListener(Event.CHANGE,this.handleScroll,false);
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
      
      protected function drawScrollBar() : void
      {
         if(!this._autoScrollBar)
         {
            return;
         }
         this._scrollBar.x = _width - this._scrollBar.width - this.margin;
         this._scrollBar.y = this.margin;
         this._scrollBar.height = this.availableHeight;
         this._scrollBar.validateNow();
      }
      
      protected function updateScrollBar() : void
      {
         if(this._scrollBar == null)
         {
            return;
         }
         var max:Number = Math.max(0,_dataProvider.length - _totalRenderers);
         var scrollIndicator:ScrollIndicator;
         if(this._scrollBar is ScrollIndicator)
         {
            scrollIndicator = this._scrollBar as ScrollIndicator;
            scrollIndicator.setScrollProperties(_dataProvider.length - _totalRenderers,0,_dataProvider.length - _totalRenderers);
         }
         this._scrollBar.position = this._scrollPosition;
         this._scrollBar.validateNow();
      }
      
      override protected function changeFocus() : void
      {
         super.changeFocus();
         var renderer:IListItemRenderer = getRendererAt(_selectedIndex,this._scrollPosition);
         if(renderer != null)
         {
            renderer.displayFocus = focused > 0;
            renderer.validateNow();
         }
      }
      
      override protected function refreshData() : void
      {
         _scrollPosition = Math.min(Math.max(0,_dataProvider.length - _totalRenderers),this._scrollPosition);
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
         this.updateSelectedIndex();
         _dataProvider.requestItemRange(this._scrollPosition,Math.min(_dataProvider.length - 1,this._scrollPosition + _totalRenderers - 1),this.populateData);
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
         var renderer:IListItemRenderer = getRendererAt(_selectedIndex,this.scrollPosition);
         if(renderer != null)
         {
            renderer.selected = false;
            renderer.validateNow();
         }
         super.selectedIndex = _newSelectedIndex;
         if(_selectedIndex < 0 || _selectedIndex >= _dataProvider.length)
         {
            return;
         }
         renderer = getRendererAt(_selectedIndex,this._scrollPosition);
         if(renderer != null)
         {
            renderer.selected = true;
            renderer.validateNow();
         }
         else
         {
            this.scrollToIndex(_selectedIndex);
            renderer = getRendererAt(_selectedIndex,this.scrollPosition);
            renderer.selected = true;
            renderer.validateNow();
         }
      }
      
      override protected function calculateRendererTotal(width:Number, height:Number) : uint
      {
         var renderer:IListItemRenderer;
         if(isNaN(this._rowHeight) && isNaN(this._autoRowHeight))
         {
            renderer = createRenderer(0);
            _autoRowHeight = renderer.height;
            cleanUpRenderer(renderer);
         }
         return (this.availableHeight - this.padding.vertical) / this.rowHeight >> 0;
      }
      
      protected function handleScroll(event:Event) : void
      {
         scrollPosition = this._scrollBar.position;
      }
      
      protected function populateData(data:Array) : void
      {
         var dl:uint = data.length;
         var l:uint = _renderers.length;
         var i:uint = 0;
         var renderer:IListItemRenderer;
         var index:uint;
         var listData:ListData;
         while(i < l)
         {
            renderer = getRendererAt(i);
            index = this._scrollPosition + i;
            listData = new ListData(index,itemToLabel(data[i]),_selectedIndex == index);
            renderer.enabled = i >= dl ? false : true;
            renderer.setListData(listData);
            renderer.setData(data[i]);
            renderer.validateNow();
            i++;
         }
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

