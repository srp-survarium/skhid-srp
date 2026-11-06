package scaleform.clik.controls
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.interfaces.IDataProvider;
   import scaleform.clik.managers.PopUpManager;
   import scaleform.clik.ui.InputDetails;
   import scaleform.clik.utils.Padding;
   
   [Event(name="change",type="flash.events.Event")]
   public class DropdownMenu extends Button
   {
      
      [Inspectable(type="String",defaultValue="CLIKScrollingList")]
      public var dropdown:Object = "CLIKScrollingList";
      
      [Inspectable(type="String",defaultValue="CLIKListItemRenderer")]
      public var itemRenderer:Object = "CLIKListItemRenderer";
      
      [Inspectable(type="String")]
      public var scrollBar:Object;
      
      [Inspectable(enumeration="normal,stick,wrap",defaultValue="normal")]
      public var menuWrapping:String = "normal";
      
      [Inspectable(enumeration="up,down",defaultValue="down")]
      public var menuDirection:String = "down";
      
      [Inspectable(defaultValue="-1")]
      public var menuWidth:Number = -1;
      
      [Inspectable(defaultValue="1")]
      public var menuMargin:Number = 1;
      
      [Inspectable(defaultValue="5")]
      public var menuRowCount:Number = 5;
      
      [Inspectable(defaultFixedLength="true")]
      public var menuRowsFixed:Boolean = true;
      
      public var menuPadding:Padding;
      
      public var menuOffset:Padding;
      
      public var thumbOffsetTop:Number;
      
      public var thumbOffsetBottom:Number;
      
      protected var _selectedIndex:int = -1;
      
      protected var _dataProvider:IDataProvider;
      
      protected var _labelField:String = "label";
      
      protected var _labelFunction:Function;
      
      protected var _popup:MovieClip;
      
      private var _dropdownRef:MovieClip = null;
      
      public function DropdownMenu()
      {
         super();
      }
      
      override protected function initialize() : void
      {
         dataProvider = new DataProvider();
         menuOffset = new Padding(0,0,0,0);
         menuPadding = new Padding(0,0,0,0);
         super.initialize();
      }
      
      override public function get autoRepeat() : Boolean
      {
         return false;
      }
      
      override public function set autoRepeat(value:Boolean) : void
      {
      }
      
      override public function get data() : Object
      {
         return null;
      }
      
      override public function set data(value:Object) : void
      {
      }
      
      override public function get label() : String
      {
         return "";
      }
      
      override public function set label(value:String) : void
      {
      }
      
      override public function get selected() : Boolean
      {
         return super.selected;
      }
      
      override public function set selected(value:Boolean) : void
      {
         super.selected = value;
      }
      
      override public function get toggle() : Boolean
      {
         return super.toggle;
      }
      
      override public function set toggle(value:Boolean) : void
      {
         super.toggle = value;
      }
      
      [Inspectable(name="menuPadding",defaultValue="top:0,right:0,bottom:0,left:0")]
      public function set inspectableMenuPadding(value:Object) : void
      {
         if(!componentInspectorSetting)
         {
            return;
         }
         menuPadding = new Padding(value.top,value.right,value.bottom,value.left);
      }
      
      [Inspectable(name="menuOffset",defaultValue="top:0,right:0,bottom:0,left:0")]
      public function set inspectableMenuOffset(value:Object) : void
      {
         if(!componentInspectorSetting)
         {
            return;
         }
         menuOffset = new Padding(value.top,value.right,value.bottom,value.left);
      }
      
      [Inspectable(name="thumbOffset",defaultValue="top:0,bottom:0")]
      public function set inspectableThumbOffset(value:Object) : void
      {
         if(!componentInspectorSetting)
         {
            return;
         }
         thumbOffsetTop = Number(value.top);
         thumbOffsetBottom = Number(value.bottom);
      }
      
      [Inspectable(defaultValue="true")]
      override public function get focusable() : Boolean
      {
         return _focusable;
      }
      
      override public function set focusable(value:Boolean) : void
      {
         super.focusable = value;
      }
      
      public function get selectedIndex() : int
      {
         return this._selectedIndex;
      }
      
      public function set selectedIndex(value:int) : void
      {
         var list:CoreList;
         var scrollOffset:uint;
         if(this._selectedIndex == value)
         {
            return;
         }
         _selectedIndex = value;
         this.invalidateSelectedIndex();
         if(this._dropdownRef != null)
         {
            list = this._dropdownRef as CoreList;
            scrollOffset = list is ScrollingList ? ((list as ScrollingList).scrollPosition) : 0;
            dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE,true,false,this._selectedIndex,-1,-1,list.getRendererAt(this._selectedIndex,scrollOffset),this._dataProvider[this._selectedIndex]));
         }
      }
      
      public function get dataProvider() : IDataProvider
      {
         return this._dataProvider;
      }
      
      public function set dataProvider(value:IDataProvider) : void
      {
         if(this._dataProvider == value)
         {
            return;
         }
         if(this._dataProvider != null)
         {
            this._dataProvider.removeEventListener(Event.CHANGE,this.handleDataChange,false);
         }
         _dataProvider = value;
         var itemCount:int = (this._dataProvider.length);
         if(!this.menuRowsFixed && itemCount > 0 && itemCount < this.menuRowCount)
         {
            menuRowCount = itemCount;
         }
         if(this._dataProvider == null)
         {
            return;
         }
         this._dataProvider.addEventListener(Event.CHANGE,this.handleDataChange,false,0,true);
         invalidateData();
      }
      
      public function get labelField() : String
      {
         return this._labelField;
      }
      
      public function set labelField(value:String) : void
      {
         _labelField = value;
         invalidateData();
      }
      
      public function get labelFunction() : Function
      {
         return this._labelFunction;
      }
      
      public function set labelFunction(value:Function) : void
      {
         _labelFunction = value;
         invalidateData();
      }
      
      public function itemToLabel(item:Object) : String
      {
         if(item == null)
         {
            return "";
         }
         if(this._labelFunction != null)
         {
            return this._labelFunction(item);
         }
         if(item is String)
         {
            return item.toString();
         }
         if(this._labelField != null && item[this._labelField] != null)
         {
            return item[this._labelField];
         }
         return item.toString();
      }
      
      public function open() : void
      {
         selected = true;
         stage.addEventListener(MouseEvent.MOUSE_DOWN,this.handleStageClick,false,0,true);
         this.showDropdown();
      }
      
      public function close() : void
      {
         selected = false;
         stage.removeEventListener(MouseEvent.MOUSE_DOWN,this.handleStageClick,false);
         this.hideDropdown();
      }
      
      public function invalidateSelectedIndex() : void
      {
         invalidate(InvalidationType.SELECTED_INDEX);
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         trace("DM::handleInput",event.handled);
         if(event.handled)
         {
            return;
         }
         if(this._dropdownRef != null && this.selected)
         {
            this._dropdownRef.handleInput(event);
            if(event.handled)
            {
               return;
            }
         }
         super.handleInput(event);
         var details:InputDetails = event.details;
         var keyPress:Boolean = details.value == InputValue.KEY_DOWN;
         switch(details.navEquivalent)
         {
            case NavigationCode.ESCAPE:
               if(this.selected)
               {
                  if(keyPress)
                  {
                     this.close();
                  }
                  event.handled = true;
               }
            default:
               break;
         }
      }
      
      override public function toString() : String
      {
         return "[CLIK DropdownMenu " + name + "]";
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.SELECTED_INDEX) || isInvalid(InvalidationType.DATA))
         {
            this._dataProvider.requestItemAt(this._selectedIndex,this.populateText);
            invalidateData();
         }
         super.draw();
      }
      
      override protected function changeFocus() : void
      {
         super.changeFocus();
         if(_selected && this._dropdownRef)
         {
            this.close();
         }
      }
      
      override protected function handleClick(controllerIndex:uint = 0) : void
      {
         if(!_selected)
         {
            this.open();
         }
         else
         {
            this.close();
         }
         super.handleClick();
      }
      
      protected function handleDataChange(event:Event) : void
      {
         invalidate(InvalidationType.DATA);
      }
      
      protected function populateText(item:Object) : void
      {
         this.updateLabel(item);
         dispatchEvent(new Event(Event.CHANGE));
      }
      
      protected function updateLabel(item:Object) : void
      {
         _label = this.itemToLabel(item);
      }
      
      protected function handleStageClick(event:MouseEvent) : void
      {
         if(this.contains(event.target as DisplayObject))
         {
            return;
         }
         if(this._dropdownRef.contains(event.target as DisplayObject))
         {
            return;
         }
         this.close();
      }
      
      protected function showDropdown() : void
      {
         var menu:MovieClip;
         var classRef:Class;
         if(this.dropdown == null)
         {
            return;
         }
         if(this.dropdown is String && this.dropdown != "")
         {
            classRef = getDefinitionByName(this.dropdown.toString()) as Class;
            if(classRef != null)
            {
               menu = new classRef() as CoreList;
            }
         }
         if(menu)
         {
            if(this.itemRenderer is String && this.itemRenderer != "")
            {
               menu.itemRenderer = getDefinitionByName(this.itemRenderer.toString()) as Class;
            }
            else if(this.itemRenderer is Class)
            {
               menu.itemRenderer = this.itemRenderer as Class;
            }
            if(this.scrollBar is String && this.scrollBar != "")
            {
               menu.scrollBar = getDefinitionByName(this.scrollBar.toString()) as Class;
            }
            else if(this.scrollBar is Class)
            {
               menu.scrollBar = this.scrollBar as Class;
            }
            menu.selectedIndex = this._selectedIndex;
            menu.width = this.menuWidth == -1 ? width + this.menuOffset.left + this.menuOffset.right : this.menuWidth;
            menu.dataProvider = this._dataProvider;
            menu.padding = this.menuPadding;
            menu.wrapping = this.menuWrapping;
            menu.margin = this.menuMargin;
            menu.thumbOffset = {
               top:this.thumbOffsetTop,
               bottom:this.thumbOffsetBottom
            };
            menu.focusTarget = this;
            menu.rowCount = this.menuRowCount < 1 ? 5 : this.menuRowCount;
            menu.labelField = this._labelField;
            menu.labelFunction = this._labelFunction;
            menu.addEventListener(ListEvent.ITEM_CLICK,this.handleMenuItemClick,false,0,true);
            _dropdownRef = menu;
            PopUpManager.show(menu,x + this.menuOffset.left,this.menuDirection == "down" ? y + height + this.menuOffset.top : y - this._dropdownRef.height + this.menuOffset.bottom,parent);
         }
      }
      
      protected function hideDropdown() : void
      {
         if(this._dropdownRef)
         {
            this._dropdownRef.parent.removeChild(this._dropdownRef);
            _dropdownRef = undefined;
         }
      }
      
      protected function handleMenuItemClick(event:ListEvent) : void
      {
         selectedIndex = event.index;
         this.close();
      }
   }
}

