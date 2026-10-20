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
         var _loc1_:int = 0;
         var _loc2_:Number = 0;
         while(_loc2_ < this._renderers.length)
         {
            trace("ButtonBat::getMax::",this._renderers.length,_loc1_,this._renderers[_loc2_].width,_width,actualWidth);
            if(this._renderers[_loc2_].width > _loc1_)
            {
               _loc1_ = (this._renderers[_loc2_].width);
            }
            _loc2_++;
         }
         return _loc1_;
      }
      
      override protected function initialize() : void
      {
         super.initialize();
         dataProvider = new DataProvider();
         _renderers = [];
      }
      
      [Inspectable(defaultValue="true")]
      override public function get enabled() : Boolean
      {
         return super.enabled;
      }
      
      override public function set enabled(param1:Boolean) : void
      {
         if(this.enabled == param1)
         {
            return;
         }
         super.enabled = param1;
         var _loc2_:Number = 0;
         while(_loc2_ < this._renderers.length)
         {
            if(this._itemRendererClass)
            {
               (this._renderers[_loc2_] as this._itemRendererClass).enabled = param1;
            }
            else
            {
               (this._renderers[_loc2_] as UIComponent).enabled = param1;
            }
            _loc2_++;
         }
      }
      
      [Inspectable(defaultValue="true")]
      override public function get focusable() : Boolean
      {
         return _focusable;
      }
      
      override public function set focusable(param1:Boolean) : void
      {
         super.focusable = param1;
      }
      
      public function get dataProvider() : IDataProvider
      {
         return this._dataProvider;
      }
      
      public function set dataProvider(param1:IDataProvider) : void
      {
         if(this._dataProvider == param1)
         {
            return;
         }
         if(this._dataProvider != null)
         {
            this._dataProvider.removeEventListener(Event.CHANGE,this.handleDataChange,false);
         }
         _dataProvider = param1;
         if(this._dataProvider == null)
         {
            return;
         }
         this._dataProvider.addEventListener(Event.CHANGE,this.handleDataChange,false,0,true);
         invalidateData();
      }
      
      [Inspectable(name="itemRenderer",defaultValue="Button")]
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
      
      [Inspectable(defaultValue="0")]
      public function get spacing() : Number
      {
         return this._spacing;
      }
      
      public function set spacing(param1:Number) : void
      {
         _spacing = param1;
         this.invalidateSettings();
      }
      
      [Inspectable(defaultValue="horizontal",type="list",enumeration="horizontal,vertical")]
      public function get direction() : String
      {
         return this._direction;
      }
      
      public function set direction(param1:String) : void
      {
         _direction = param1;
         this.invalidateSettings();
      }
      
      [Inspectable(type="String",enumeration="none,left,center,right",defaultValue="none")]
      public function get autoSize() : String
      {
         return this._autoSize;
      }
      
      public function set autoSize(param1:String) : void
      {
         if(param1 == this._autoSize)
         {
            return;
         }
         _autoSize = param1;
         var _loc2_:Number = 0;
         while(_loc2_ < this._renderers.length)
         {
            (this._renderers[_loc2_] as this._itemRendererClass).autoSize = this._autoSize;
            _loc2_++;
         }
         this.invalidateSettings();
      }
      
      [Inspectable(defaultValue="0")]
      public function get buttonWidth() : Number
      {
         return this._buttonWidth;
      }
      
      public function set buttonWidth(param1:Number) : void
      {
         _buttonWidth = param1;
         invalidate();
      }
      
      public function get selectedIndex() : int
      {
         return this._selectedIndex;
      }
      
      public function set selectedIndex(param1:int) : void
      {
         if(param1 == this._selectedIndex)
         {
            return;
         }
         var _loc2_:int = this._selectedIndex;
         var _loc3_:Button = this._renderers[_loc2_] as Button;
         if(_loc3_)
         {
            _loc3_.selected = false;
         }
         _selectedIndex = param1;
         _loc3_ = this._renderers[this._selectedIndex] as Button;
         if(_loc3_)
         {
            _loc3_.selected = true;
         }
         dispatchEvent(new IndexEvent(IndexEvent.INDEX_CHANGE,true,true,this._selectedIndex,_loc2_,this._dataProvider[this._selectedIndex]));
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
      
      public function set labelField(param1:String) : void
      {
         _labelField = param1;
         invalidateData();
      }
      
      public function get labelFunction() : Function
      {
         return this._labelFunction;
      }
      
      public function set labelFunction(param1:Function) : void
      {
         _labelFunction = param1;
         invalidateData();
      }
      
      public function invalidateSettings() : void
      {
         invalidate(InvalidationType.SETTINGS);
      }
      
      public function itemToLabel(param1:Object) : String
      {
         if(param1 == null)
         {
            return "";
         }
         if(this._labelFunction != null)
         {
            return this._labelFunction(param1);
         }
         if(param1 is String)
         {
            return param1 as String;
         }
         if(this._labelField != null && param1[this._labelField] != null)
         {
            return param1[this._labelField];
         }
         return param1.toString();
      }
      
      public function getButtonAt(param1:int) : Button
      {
         if(param1 >= 0 && param1 < this._renderers.length)
         {
            return this._renderers[param1];
         }
         return null;
      }
      
      override public function handleInput(param1:InputEvent) : void
      {
         if(param1.handled)
         {
            return;
         }
         var _loc2_:Button = this._renderers[this._selectedIndex] as Button;
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
         if(!_loc4_)
         {
            return;
         }
         var _loc5_:Boolean = false;
         var _loc6_:Number;
         switch(_loc3_.navEquivalent)
         {
            case NavigationCode.LEFT:
               if(this._direction == DIRECTION_HORIZONTAL)
               {
                  _loc6_ = this._selectedIndex - 1;
                  _loc5_ = true;
               }
               break;
            case NavigationCode.RIGHT:
               if(this._direction == DIRECTION_HORIZONTAL)
               {
                  _loc6_ = this._selectedIndex + 1;
                  _loc5_ = true;
               }
               break;
            case NavigationCode.UP:
               if(this._direction == DIRECTION_VERTICAL)
               {
                  _loc6_ = this._selectedIndex - 1;
                  _loc5_ = true;
               }
               break;
            case NavigationCode.DOWN:
               if(this._direction == DIRECTION_VERTICAL)
               {
                  _loc6_ = this._selectedIndex + 1;
                  _loc5_ = true;
               }
               break;
            default:
               break;
         }
         if(_loc5_)
         {
            _loc6_ = Math.max(0,Math.min(this._dataProvider.length - 1,_loc6_));
            if(_loc6_ != this._selectedIndex)
            {
               selectedIndex = _loc6_;
               param1.handled = true;
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
         var _loc1_:Number;
         var _loc2_:Button;
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
               _loc1_ = 0;
               while(_loc1_ < this._renderers.length)
               {
                  _loc2_ = this._renderers[_loc1_];
                  trace("setWidth::",this._buttonWidth,width);
                  _loc2_.autoSize = TextFieldAutoSize.NONE;
                  _loc2_.width = this._buttonWidth;
                  _loc2_.validateNow();
                  _loc1_++;
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
      
      public function setMaxFit(param1:Boolean) : void
      {
         _maxFit = param1;
      }
      
      public function setMaxSize(param1:int) : void
      {
         _maxSize = param1;
      }
      
      protected function updateRenderers() : void
      {
         var _loc1_:Number = 0;
         var _loc2_:Number = 0;
         var _loc3_:int = -1;
         if(this._renderers[0] is this._itemRendererClass)
         {
            while(this._renderers.length > this._dataProvider.length)
            {
               _loc5_ = this._renderers.length - 1;
               if(this.container.contains(this._renderers[_loc5_]))
               {
                  this.container.removeChild(this._renderers[_loc5_]);
               }
               this._renderers.splice(_loc5_--,1);
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
         var _loc4_:uint = 0;
         var _loc5_:int;
         var _loc6_:Button;
         var _loc7_:Boolean;
         var _loc8_:int;
         var _loc9_:Button;
         while(_loc4_ < this._dataProvider.length && _loc3_ == -1)
         {
            _loc7_ = false;
            if(_loc4_ < this._renderers.length)
            {
               _loc6_ = this._renderers[_loc4_];
            }
            else
            {
               _loc6_ = new this._itemRendererClass();
               this.setupRenderer(_loc6_,_loc4_);
               _loc7_ = true;
            }
            this.populateRendererData(_loc6_,_loc4_);
            if(this._autoSize == TextFieldAutoSize.NONE && this._buttonWidth > 0)
            {
               _loc6_.width = this._buttonWidth;
            }
            else if(this._autoSize != TextFieldAutoSize.NONE)
            {
               _loc6_.autoSize = this._autoSize;
            }
            _loc6_.validateNow();
            if(this._direction == DIRECTION_HORIZONTAL)
            {
               if(_width > _loc6_.width + this._spacing + _loc1_ || this._maxFit)
               {
                  _loc6_.y = 0;
                  _loc6_.x = _loc1_;
                  _loc1_ += _loc6_.width + this._spacing;
               }
               else
               {
                  _loc3_ = (_loc4_);
                  _loc6_ = null;
               }
            }
            else if(_height > _loc6_.height + this._spacing + _loc2_)
            {
               _loc6_.x = 0;
               _loc6_.y = _loc2_;
               _loc2_ += _loc6_.height + this._spacing;
            }
            else
            {
               _loc3_ = (_loc4_);
               _loc6_ = null;
            }
            if(_loc7_ && _loc6_ != null)
            {
               _loc6_.group = this._group;
               this.container.addChild(_loc6_);
               this._renderers.push(_loc6_);
            }
            _loc4_++;
         }
         if(_loc3_ > -1)
         {
            _loc8_ = this._renderers.length - 1;
            while(_loc8_ >= _loc3_)
            {
               _loc9_ = this._renderers[_loc8_];
               if(_loc9_)
               {
                  if(this.container.contains(_loc9_))
                  {
                     this.container.removeChild(_loc9_);
                  }
                  this._renderers.splice(_loc8_,1);
               }
               _loc8_--;
            }
         }
         selectedIndex = Math.min(this._dataProvider.length - 1,this._selectedIndex);
      }
      
      public function set autoResize(param1:Boolean) : void
      {
         _autoResize = param1;
      }
      
      public function get autoResize() : Boolean
      {
         return this._autoResize;
      }
      
      protected function populateData(param1:Array) : void
      {
         var _loc2_:uint = 0;
         var _loc3_:Button;
         while(_loc2_ < this._renderers.length)
         {
            _loc3_ = this._renderers[_loc2_] as Button;
            this.populateRendererData(_loc3_,_loc2_);
            _loc3_.validateNow();
            _loc2_++;
         }
      }
      
      protected function populateRendererData(param1:Button, param2:uint) : void
      {
         param1.label = this.itemToLabel(this._dataProvider.requestItemAt(param2));
         param1.data = this._dataProvider.requestItemAt(param2);
         param1.selected = param2 == this.selectedIndex;
      }
      
      protected function setupRenderer(param1:Button, param2:uint) : void
      {
         param1.owner = this;
         param1.focusable = false;
         param1.focusTarget = this;
         param1.toggle = true;
         param1.allowDeselect = false;
      }
      
      protected function handleButtonGroupChange(param1:Event) : void
      {
         var _loc2_:ButtonEvent = param1 as ButtonEvent;
         if(this._group.selectedIndex != this.selectedIndex)
         {
            selectedIndex = this._group.selectedIndex;
            dispatchEvent(new ButtonBarEvent(ButtonBarEvent.BUTTON_SELECT,false,true,this._selectedIndex,param1.target as Button));
         }
      }
      
      protected function handleDataChange(param1:Event) : void
      {
         invalidate(InvalidationType.DATA);
      }
      
      override protected function changeFocus() : void
      {
         var _loc1_:Button = this._renderers[this._selectedIndex] as Button;
         if(_loc1_ == null)
         {
            return;
         }
         _loc1_.displayFocus = _focused > 0;
      }
   }
}

