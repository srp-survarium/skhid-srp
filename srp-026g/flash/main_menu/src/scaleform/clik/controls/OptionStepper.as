package scaleform.clik.controls
{
   import flash.events.Event;
   import flash.text.TextField;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ComponentEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.interfaces.IDataProvider;
   import scaleform.clik.ui.InputDetails;
   import scaleform.clik.utils.Constraints;
   
   [Event(name="change",type="flash.events.Event")]
   public class OptionStepper extends UIComponent
   {
      
      public var selectedItem:Object;
      
      protected var _dataProvider:IDataProvider;
      
      protected var _selectedIndex:Number = 0;
      
      protected var _newSelectedIndex:int = 0;
      
      protected var _labelField:String = "label";
      
      protected var _labelFunction:Function;
      
      protected var _state:String = "default";
      
      protected var _newFrame:String;
      
      protected var _constraintsDisabled:Boolean = false;
      
      public var textField:TextField;
      
      public var nextBtn:Button;
      
      public var prevBtn:Button;
      
      public function OptionStepper()
      {
         super();
      }
      
      override protected function preInitialize() : void
      {
         if(!this._constraintsDisabled)
         {
            constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         }
      }
      
      override protected function initialize() : void
      {
         dataProvider = new DataProvider();
         super.initialize();
      }
      
      [Inspectable(defaultValue="true")]
      override public function get enabled() : Boolean
      {
         return super.enabled;
      }
      
      override public function set enabled(value:Boolean) : void
      {
         if(value == super.enabled)
         {
            return;
         }
         super.enabled = value;
         mouseEnabled = this.enabled;
         tabEnabled = _focusable && this.enabled;
         gotoAndPlay(value ? (_focused > 0 ? "focused" : "default") : "disabled");
         if(!initialized)
         {
            return;
         }
         this.updateAfterStateChange();
         this.prevBtn.enabled = this.nextBtn.enabled = value;
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
      
      public function get dataProvider() : IDataProvider
      {
         return this._dataProvider;
      }
      
      public function set dataProvider(value:IDataProvider) : void
      {
         if(this._dataProvider != value)
         {
            if(this._dataProvider != null)
            {
               this._dataProvider.removeEventListener(Event.CHANGE,this.handleDataChange);
            }
            _dataProvider = value;
            selectedItem = null;
            if(this._dataProvider == null)
            {
               return;
            }
            this._dataProvider.addEventListener(Event.CHANGE,this.handleDataChange);
         }
         invalidateData();
         this.updateSelectedItem();
      }
      
      public function get selectedIndex() : int
      {
         return this._selectedIndex;
      }
      
      public function set selectedIndex(value:int) : void
      {
         var newIndex:Number = Math.max(0,Math.min(this._dataProvider.length - 1,value));
         if(newIndex == this._selectedIndex || newIndex == this._newSelectedIndex)
         {
            return;
         }
         _newSelectedIndex = newIndex;
         this.invalidateSelectedIndex();
      }
      
      public function get labelField() : String
      {
         return this._labelField;
      }
      
      public function set labelField(value:String) : void
      {
         _labelField = value;
         this.updateLabel();
      }
      
      public function get labelFunction() : Function
      {
         return this._labelFunction;
      }
      
      public function set labelFunction(value:Function) : void
      {
         _labelFunction = value;
         this.updateLabel();
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
         if(this._labelField != null && this._labelField in item && item[this._labelField] != null)
         {
            return item[this._labelField];
         }
         return item.toString();
      }
      
      public function invalidateSelectedIndex() : void
      {
         invalidate(InvalidationType.SELECTED_INDEX);
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.isDefaultPrevented())
         {
            return;
         }
         var details:InputDetails = event.details;
         var controllerIndex:uint = details.controllerIndex;
         var keyPress:Boolean = details.value == InputValue.KEY_DOWN || details.value == InputValue.KEY_HOLD;
         switch(details.navEquivalent)
         {
            case NavigationCode.RIGHT:
               if(this._selectedIndex < this._dataProvider.length - 1)
               {
                  if(keyPress)
                  {
                     this.onNext(null);
                  }
                  event.handled = true;
               }
               break;
            case NavigationCode.LEFT:
               if(this._selectedIndex > 0)
               {
                  if(keyPress)
                  {
                     this.onPrev(null);
                  }
                  event.handled = true;
               }
               break;
            case NavigationCode.HOME:
               if(!keyPress)
               {
                  selectedIndex = 0;
               }
               event.handled = true;
               break;
            case NavigationCode.END:
               if(!keyPress)
               {
                  selectedIndex = this._dataProvider.length - 1;
               }
               event.handled = true;
               break;
         }
      }
      
      override public function toString() : String
      {
         return "[CLIK OptionStepper " + name + "]";
      }
      
      override protected function configUI() : void
      {
         if(!this._constraintsDisabled)
         {
            constraints.addElement("textField",this.textField,Constraints.ALL);
         }
         addEventListener(InputEvent.INPUT,this.handleInput,false,0,true);
         this.nextBtn.addEventListener(ButtonEvent.CLICK,this.onNext,false,0,true);
         this.prevBtn.addEventListener(ButtonEvent.CLICK,this.onPrev,false,0,true);
         tabEnabled = _focusable;
         tabChildren = false;
         this.textField.tabEnabled = this.textField.mouseEnabled = false;
         this.prevBtn.enabled = this.nextBtn.enabled = this.enabled;
         this.prevBtn.autoRepeat = this.nextBtn.autoRepeat = true;
         this.prevBtn.focusable = this.nextBtn.focusable = false;
         this.prevBtn.focusTarget = this.nextBtn.focusTarget = this;
         this.prevBtn.tabEnabled = this.nextBtn.tabEnabled = false;
         this.prevBtn.mouseEnabled = this.nextBtn.mouseEnabled = true;
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.SELECTED_INDEX))
         {
            this.updateSelectedIndex();
         }
         if(isInvalid(InvalidationType.STATE))
         {
            if(this._newFrame)
            {
               gotoAndPlay(this._newFrame);
               _newFrame = null;
            }
            this.updateAfterStateChange();
            dispatchEvent(new ComponentEvent(ComponentEvent.STATE_CHANGE));
            invalidate(InvalidationType.DATA);
         }
         if(isInvalid(InvalidationType.DATA))
         {
            this.refreshData();
         }
         if(isInvalid(InvalidationType.SIZE))
         {
            setActualSize(_width,_height);
            if(!this._constraintsDisabled)
            {
               constraints.update(_width,_height);
            }
         }
      }
      
      override protected function changeFocus() : void
      {
         if(_focused || _displayFocus)
         {
            this.setState("focused","default");
         }
         else
         {
            this.setState("default");
         }
         this.prevBtn.displayFocus = this.nextBtn.displayFocus = _focused > 0;
      }
      
      protected function updateSelectedIndex() : void
      {
         if(this._selectedIndex == this._newSelectedIndex)
         {
            return;
         }
         var lastIndex:int = this._selectedIndex;
         _selectedIndex = this._newSelectedIndex;
         dispatchEventAndSound(new IndexEvent(IndexEvent.INDEX_CHANGE,true,false,this._selectedIndex,lastIndex,this.dataProvider[this._selectedIndex]));
         this.updateSelectedItem();
      }
      
      protected function refreshData() : void
      {
         this._dataProvider.requestItemAt(this._selectedIndex,this.populateText);
      }
      
      protected function handleDataChange(event:Event) : void
      {
         invalidate(InvalidationType.DATA);
      }
      
      protected function updateAfterStateChange() : void
      {
         invalidateSize();
         this.updateLabel();
         this.textField.tabEnabled = this.textField.mouseEnabled = false;
         if(constraints != null && !this._constraintsDisabled)
         {
            constraints.updateElement("textField",this.textField);
         }
      }
      
      protected function updateLabel() : void
      {
         if(this.selectedItem == null)
         {
            return;
         }
         if(this.textField != null)
         {
            this.textField.text = this.itemToLabel(this.selectedItem);
         }
      }
      
      protected function updateSelectedItem() : void
      {
         invalidateData();
      }
      
      protected function populateText(item:Object) : void
      {
         selectedItem = item;
         this.updateLabel();
         dispatchEventAndSound(new Event(Event.CHANGE));
      }
      
      protected function onNext(event:Object) : void
      {
         selectedIndex += 1;
         this.invalidateSelectedIndex();
      }
      
      protected function onPrev(event:Object) : void
      {
         selectedIndex -= 1;
         this.invalidateSelectedIndex();
      }
      
      protected function setState(... rest) : void
      {
         if(rest.length == 1)
         {
            frameLabel = rest[0].toString();
            if(this._state != frameLabel && _labelHash[frameLabel])
            {
               _state = _newFrame = frameLabel;
               invalidateState();
            }
            return;
         }
         var count:uint = (rest.length);
         var i:uint = 0;
         var frameLabel:String;
         var candidate:String;
         while(i < count)
         {
            candidate = rest[i].toString();
            if(_labelHash[candidate])
            {
               _state = _newFrame = candidate;
               invalidateState();
               break;
            }
            i++;
         }
      }
   }
}

