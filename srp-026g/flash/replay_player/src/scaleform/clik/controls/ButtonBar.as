package scaleform.clik.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.TextFieldAutoSize;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonBarEvent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.interfaces.IDataProvider;
   import scaleform.clik.ui.InputDetails;
   
   public class ButtonBar extends UIComponent
   {
      
      public static const DIRECTION_HORIZONTAL:String = "horizontal";
      
      public static const DIRECTION_VERTICAL:String = "vertical";
      
      protected var _autoSize:String = "none";
      
      protected var _buttonWidth:Number = 0;
      
      protected var _dataProvider:IDataProvider;
      
      protected var _direction:String = "horizontal";
      
      protected var _group:ButtonGroup;
      
      protected var _itemRenderer:String = "Button";
      
      protected var _itemRendererClass:Class;
      
      protected var _labelField:String = "label";
      
      protected var _labelFunction:Function;
      
      protected var _renderers:Array;
      
      protected var _spacing:Number = 0;
      
      protected var _selectedIndex:Number = -1;
      
      public var container:MovieClip;
      
      protected var _maxFit:Boolean = false;
      
      protected var _maxSize:int = 0;
      
      protected var _autoResize:Boolean = false;
      
      public function ButtonBar()
      {
         super();
      }
      
      public function getMaxWidth() : int
      {
         var _max:int;
         var i:Number = 0;
         while(i < this._renderers.length)
         {
            trace("ButtonBat::getMax::",this._renderers.length,_max,this._renderers[i].width,_width,actualWidth);
            if(this._renderers[i].width > _max)
            {
               _max = (this._renderers[i].width);
            }
            i++;
         }
         return _max;
      }
      
      override protected function initialize() : void
      {
         super.initialize();
         dataProvider = new DataProvider();
         _renderers = [];
      }
      
      override public function get enabled() : Boolean
      {
         return super.enabled;
      }
      
      override public function set enabled(value:Boolean) : void
      {
         if(this.enabled == value)
         {
            return;
         }
         super.enabled = value;
         for(var i:Number = 0; i < this._renderers.length; i++)
         {
            if(this._itemRendererClass)
            {
               (this._renderers[i] as this._itemRendererClass).enabled = value;
            }
            else
            {
               (this._renderers[i] as UIComponent).enabled = value;
            }
         }
      }
      
      override public function get focusable() : Boolean
      {
         return _focusable;
      }
      
      override public function set focusable(value:Boolean) : void
      {
         super.focusable = value;
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
         if(this._dataProvider == null)
         {
            return;
         }
         this._dataProvider.addEventListener(Event.CHANGE,this.handleDataChange,false,0,true);
         invalidateData();
      }
      
      public function set itemRendererName(value:String) : void
      {
         var classRef:Class;
         if(_inspector && value == "Button" || value == "")
         {
            return;
         }
         try
         {
            classRef = getDefinitionByName(value) as Class;
         }
         catch(error:*)
         {
            throw new Error("The class " + value + " cannot be found in your library. Please ensure it exists.");
         }
         if(classRef != null)
         {
            _itemRendererClass = classRef;
            invalidate();
         }
      }
      
      public function get spacing() : Number
      {
         return this._spacing;
      }
      
      public function set spacing(value:Number) : void
      {
         _spacing = value;
         this.invalidateSettings();
      }
      
      public function get direction() : String
      {
         return this._direction;
      }
      
      public function set direction(value:String) : void
      {
         _direction = value;
         this.invalidateSettings();
      }
      
      public function get autoSize() : String
      {
         return this._autoSize;
      }
      
      public function set autoSize(value:String) : void
      {
         if(value == this._autoSize)
         {
            return;
         }
         _autoSize = value;
         var i:Number = 0;
         while(i < this._renderers.length)
         {
            (this._renderers[i] as this._itemRendererClass).autoSize = this._autoSize;
            i++;
         }
         this.invalidateSettings();
      }
      
      public function get buttonWidth() : Number
      {
         return this._buttonWidth;
      }
      
      public function set buttonWidth(value:Number) : void
      {
         _buttonWidth = value;
         invalidate();
      }
      
      public function get selectedIndex() : int
      {
         return this._selectedIndex;
      }
      
      public function set selectedIndex(value:int) : void
      {
         if(value == this._selectedIndex)
         {
            return;
         }
         var oldSelectedIndex:int = this._selectedIndex;
         var renderer:Button = this._renderers[oldSelectedIndex] as Button;
         if(renderer)
         {
            renderer.selected = false;
         }
         _selectedIndex = value;
         renderer = this._renderers[this._selectedIndex] as Button;
         if(renderer)
         {
            renderer.selected = true;
         }
         dispatchEvent(new IndexEvent(IndexEvent.INDEX_CHANGE,true,true,this._selectedIndex,oldSelectedIndex,this._dataProvider[this._selectedIndex]));
      }
      
      public function get selectedItem() : Object
      {
         return this._dataProvider.requestItemAt(this._selectedIndex);
      }
      
      public function get data() : Object
      {
         return this.selectedItem.data;
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
      
      public function invalidateSettings() : void
      {
         invalidate(InvalidationType.SETTINGS);
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
            return item as String;
         }
         if(this._labelField != null && item[this._labelField] != null)
         {
            return item[this._labelField];
         }
         return item.toString();
      }
      
      public function getButtonAt(index:int) : Button
      {
         if(index >= 0 && index < this._renderers.length)
         {
            return this._renderers[index];
         }
         return null;
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.handled)
         {
            return;
         }
         var renderer:Button = this._renderers[this._selectedIndex] as Button;
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
         if(!keyPress)
         {
            return;
         }
         var indexChanged:Boolean = false;
         var newIndex:Number;
         switch(details.navEquivalent)
         {
            case NavigationCode.LEFT:
               if(this._direction == DIRECTION_HORIZONTAL)
               {
                  newIndex = this._selectedIndex - 1;
                  indexChanged = true;
               }
               break;
            case NavigationCode.RIGHT:
               if(this._direction == DIRECTION_HORIZONTAL)
               {
                  newIndex = this._selectedIndex + 1;
                  indexChanged = true;
               }
               break;
            case NavigationCode.UP:
               if(this._direction == DIRECTION_VERTICAL)
               {
                  newIndex = this._selectedIndex - 1;
                  indexChanged = true;
               }
               break;
            case NavigationCode.DOWN:
               if(this._direction == DIRECTION_VERTICAL)
               {
                  newIndex = this._selectedIndex + 1;
                  indexChanged = true;
               }
               break;
            default:
               break;
         }
         if(indexChanged)
         {
            newIndex = Math.max(0,Math.min(this._dataProvider.length - 1,newIndex));
            if(newIndex != this._selectedIndex)
            {
               selectedIndex = newIndex;
               event.handled = true;
            }
         }
      }
      
      override public function toString() : String
      {
         return "[CLIK ButtonBar " + name + "]";
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         tabEnabled = _focusable && this.enabled;
         if(this._group == null)
         {
            _group = new ButtonGroup(name + "Group",this);
         }
         this._group.addEventListener(ButtonEvent.CLICK,this.handleButtonGroupChange,false,0,true);
         if(this.container == null)
         {
            container = new MovieClip();
            addChild(this.container);
         }
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
      }
      
      override protected function draw() : void
      {
         var i:Number;
         var renderer:Button;
         if(isInvalid(InvalidationType.RENDERERS) || isInvalid(InvalidationType.DATA) || isInvalid(InvalidationType.SETTINGS) || isInvalid(InvalidationType.SIZE))
         {
            removeChild(this.container);
            setActualSize(_width,_height);
            this.container.scaleX = 1 / scaleX;
            this.container.scaleY = 1 / scaleY;
            addChild(this.container);
            this.updateRenderers();
            if(this._autoResize)
            {
               _buttonWidth = this.getMaxWidth();
               i = 0;
               while(i < this._renderers.length)
               {
                  renderer = this._renderers[i];
                  trace("setWidth::",this._buttonWidth,width);
                  renderer.autoSize = TextFieldAutoSize.NONE;
                  renderer.width = this._buttonWidth;
                  renderer.validateNow();
                  i++;
               }
            }
         }
      }
      
      protected function refreshData() : void
      {
         selectedIndex = Math.min(this._dataProvider.length - 1,this._selectedIndex);
         if(this._dataProvider)
         {
            this._dataProvider.requestItemRange(0,this._dataProvider.length - 1,this.populateData);
         }
      }
      
      public function setMaxFit(value:Boolean) : void
      {
         _maxFit = value;
      }
      
      public function setMaxSize(value:int) : void
      {
         _maxSize = value;
      }
      
      protected function updateRenderers() : void
      {
         var w:Number = 0;
         var h:Number = 0;
         var overflowIndex:int = -1;
         if(this._renderers[0] is this._itemRendererClass)
         {
            while(this._renderers.length > this._dataProvider.length)
            {
               c = (this._renderers.length - 1);
               if(this.container.contains(this._renderers[c]))
               {
                  this.container.removeChild(this._renderers[c]);
               }
               this._renderers.splice(c--,1);
            }
         }
         else
         {
            while(this.container.numChildren > 0)
            {
               this.container.removeChildAt(0);
            }
            this._renderers.length = 0;
         }
         var i:uint = 0;
         var c:int;
         var renderer:Button;
         var isNew:Boolean;
         var j:int;
         var rend:Button;
         while(i < this._dataProvider.length && overflowIndex == -1)
         {
            isNew = false;
            if(i < this._renderers.length)
            {
               renderer = this._renderers[i];
            }
            else
            {
               renderer = new this._itemRendererClass();
               this.setupRenderer(renderer,i);
               isNew = true;
            }
            this.populateRendererData(renderer,i);
            if(this._autoSize == TextFieldAutoSize.NONE && this._buttonWidth > 0)
            {
               renderer.width = this._buttonWidth;
            }
            else if(this._autoSize != TextFieldAutoSize.NONE)
            {
               renderer.autoSize = this._autoSize;
            }
            renderer.validateNow();
            if(this._direction == DIRECTION_HORIZONTAL)
            {
               if(_width > renderer.width + this._spacing + w || this._maxFit)
               {
                  renderer.y = 0;
                  renderer.x = w;
                  w += renderer.width + this._spacing;
               }
               else
               {
                  overflowIndex = (i);
                  renderer = null;
               }
            }
            else if(_height > renderer.height + this._spacing + h)
            {
               renderer.x = 0;
               renderer.y = h;
               h += renderer.height + this._spacing;
            }
            else
            {
               overflowIndex = (i);
               renderer = null;
            }
            if(isNew && renderer != null)
            {
               renderer.group = this._group;
               this.container.addChild(renderer);
               this._renderers.push(renderer);
            }
            i++;
         }
         if(overflowIndex > -1)
         {
            for(j = (this._renderers.length - 1); j >= overflowIndex; j--)
            {
               rend = this._renderers[j];
               if(rend)
               {
                  if(this.container.contains(rend))
                  {
                     this.container.removeChild(rend);
                  }
                  this._renderers.splice(j,1);
               }
            }
         }
         selectedIndex = Math.min(this._dataProvider.length - 1,this._selectedIndex);
      }
      
      public function set autoResize(value:Boolean) : void
      {
         _autoResize = value;
      }
      
      public function get autoResize() : Boolean
      {
         return this._autoResize;
      }
      
      protected function populateData(data:Array) : void
      {
         var i:uint = 0;
         var renderer:Button;
         while(i < this._renderers.length)
         {
            renderer = this._renderers[i] as Button;
            this.populateRendererData(renderer,i);
            renderer.validateNow();
            i++;
         }
      }
      
      protected function populateRendererData(renderer:Button, index:uint) : void
      {
         renderer.label = this.itemToLabel(this._dataProvider.requestItemAt(index));
         renderer.data = this._dataProvider.requestItemAt(index);
         renderer.selected = index == this.selectedIndex;
      }
      
      protected function setupRenderer(renderer:Button, index:uint) : void
      {
         renderer.owner = this;
         renderer.focusable = false;
         renderer.focusTarget = this;
         renderer.toggle = true;
         renderer.allowDeselect = false;
      }
      
      protected function handleButtonGroupChange(event:Event) : void
      {
         var ev:ButtonEvent = event as ButtonEvent;
         if(this._group.selectedIndex != this.selectedIndex)
         {
            selectedIndex = this._group.selectedIndex;
            dispatchEvent(new ButtonBarEvent(ButtonBarEvent.BUTTON_SELECT,false,true,this._selectedIndex,event.target as Button));
         }
      }
      
      protected function handleDataChange(event:Event) : void
      {
         invalidate(InvalidationType.DATA);
      }
      
      override protected function changeFocus() : void
      {
         var renderer:Button = this._renderers[this._selectedIndex] as Button;
         if(renderer == null)
         {
            return;
         }
         renderer.displayFocus = _focused > 0;
      }
   }
}

