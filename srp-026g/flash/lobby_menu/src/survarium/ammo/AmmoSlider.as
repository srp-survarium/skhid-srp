package survarium.ammo
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.geom.Point;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.Slider;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.events.SliderEvent;
   import scaleform.clik.ui.InputDetails;
   import survarium.data.LocalizationData;
   
   public class AmmoSlider extends Slider
   {
      
      public var trackMax:Button;
      
      public var trackCurrent:Button;
      
      public var trackMin:Button;
      
      public var trackSelect:Button;
      
      public var request_pos:Sprite;
      
      public var avi_pos:Sprite;
      
      public var shiftInterval:int = 4;
      
      protected var _trackWidth:int = 0;
      
      protected var _value_storage:int = 0;
      
      protected var _min_request:int = 0;
      
      protected var _maxSize:uint = 0;
      
      protected var _minimum_storage:int = 0;
      
      protected var _available:int = 0;
      
      protected var _total:int = 0;
      
      protected var _selectValue:uint = 0;
      
      protected var _itemRenderer:Class;
      
      protected var _cells:Array = new Array();
      
      protected var _scaleRenderers:Boolean = false;
      
      private var _hoverCells:Boolean = false;
      
      public function AmmoSlider()
      {
         super();
         thumb = new Button();
         if(track is ToolTip)
         {
            track.mouseEnabled = true;
            (track as ToolTip).tooltipRendererName = "SrvTextHintRenderer";
         }
      }
      
      public function set minimum_storage(value:int) : void
      {
         _minimum_storage = value;
      }
      
      public function get minimum_storage() : int
      {
         return this._minimum_storage;
      }
      
      public function set required(value:int) : void
      {
         if(value < minimum)
         {
            value = minimum;
         }
         else if(value > maximum)
         {
            value = maximum;
         }
         _min_request = value;
      }
      
      public function get required() : int
      {
         return this._min_request;
      }
      
      public function set available(value:int) : void
      {
         _available = value;
      }
      
      public function get available() : int
      {
         return this._available;
      }
      
      public function set total(value:int) : void
      {
         _total = value;
         this.updateThumb();
      }
      
      public function get total() : int
      {
         return this._total;
      }
      
      public function set maxSize(value:uint) : void
      {
         _maxSize = value;
      }
      
      public function get maxSize() : uint
      {
         return this._maxSize;
      }
      
      public function set value_storage(newValue:int) : void
      {
         if(newValue < this._minimum_storage)
         {
            newValue = this._minimum_storage;
         }
         if(newValue < this.available)
         {
            newValue = this.available;
         }
         if(newValue < minimum)
         {
            newValue = minimum;
         }
         _value_storage = newValue;
         if(value > this._value_storage)
         {
            value = this._value_storage;
         }
         dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,newValue));
         this.draw();
      }
      
      public function get value_storage() : int
      {
         return this._value_storage;
      }
      
      public function set renderer(value:String) : void
      {
         if(value == "")
         {
            return;
         }
         var rendererClass:Class = getDefinitionByName(value) as Class;
         if(rendererClass != null)
         {
            _itemRenderer = rendererClass;
         }
         else
         {
            trace("Error: " + this + ", The class " + value + " cannot be found in your library. Please ensure it is there.");
         }
      }
      
      override public function set value(newValue:Number) : void
      {
         if(newValue < minimum)
         {
            newValue = minimum;
         }
         else if(newValue > maximum)
         {
            newValue = maximum;
         }
         _value = newValue;
         if(this._value_storage < newValue)
         {
            value_storage = newValue;
         }
         _selectValue = _value;
         dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,_value));
         this.draw();
      }
      
      public function set scaleRenders(value:Boolean) : void
      {
         _scaleRenderers = value;
      }
      
      override public function toString() : String
      {
         "[ AmmoSlider: min:" + minimum;
         return " max:" + maximum + " tot:" + this.total + " val:" + value + " sel:" + this._selectValue + " store:" + this.value_storage + " avi:" + this.available + " min_stor:" + this._minimum_storage + " snap:" + snapInterval + " mx:" + this._maxSize + " ]";
      }
      
      override protected function configUI() : void
      {
         track.addEventListener(MouseEvent.MOUSE_OUT,this.onMouseOut);
         track.addEventListener(MouseEvent.MOUSE_OVER,this.onMouseOver);
         constraints.removeAllElements();
         super.configUI();
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
               if(keyPress && value < this.total * _snapInterval)
               {
                  value = value + _snapInterval;
                  event.handled = true;
               }
               break;
            case NavigationCode.LEFT:
               if(keyPress)
               {
                  value = value - _snapInterval;
                  event.handled = true;
               }
               break;
            case NavigationCode.HOME:
               if(!keyPress)
               {
                  value = minimum;
                  event.handled = true;
               }
               break;
            case NavigationCode.END:
               if(!keyPress)
               {
                  value = maximum;
                  event.handled = true;
               }
            break;
            default:
               break;
         }
      }
      
      protected function onMouseOver(event:MouseEvent) : void
      {
         track.addEventListener(MouseEvent.MOUSE_MOVE,this.onMouseMove);
      }
      
      protected function onMouseOut(event:MouseEvent) : void
      {
         track.removeEventListener(MouseEvent.MOUSE_MOVE,this.onMouseMove);
         if(this._selectValue != value)
         {
            _selectValue = value;
            trace("set_mouse_cursor::",1);
            ExternalInterface.call("set_mouse_cursor",1);
            this.updateThumb();
         }
      }
      
      protected function onMouseMove(event:MouseEvent) : void
      {
         var localPoint:Point = globalToLocal(new Point(event.stageX,event.stageY));
         var posX:Number = localPoint.x - (_dragOffset ? _dragOffset.x : 0);
         var trackLength:Number = this._trackWidth - offsetLeft - offsetRight;
         var newValue:Number = (posX - offsetLeft) / trackLength * (_maximum - _minimum) + _minimum;
         var cursorId:uint;
         newValue = lockValue(newValue);
         if(newValue > this.total)
         {
            newValue = this.total;
         }
         if(this._selectValue != newValue)
         {
            _selectValue = newValue;
            cursorId = this._selectValue >= _value ? 10 : 11;
            track.data = {name:LocalizationData.getName(cursorId == 10 ? "add" : "remove")};
            track.invalidateData();
            ExternalInterface.call("set_mouse_cursor",cursorId);
            this.updateThumb();
         }
      }
      
      override protected function trackPress(event:MouseEvent) : void
      {
         _trackPressed = true;
         track.focused = _focused;
         var trackLength:Number = this._trackWidth - offsetLeft - offsetRight;
         var localPoint:Point = globalToLocal(new Point(event.stageX,event.stageY));
         var posX:Number = localPoint.x - (_dragOffset ? _dragOffset.x : 0);
         var newValue:Number = lockValue((posX - offsetLeft) / trackLength * (_maximum - _minimum) + _minimum);
         if(value == newValue || newValue > this.total)
         {
            return;
         }
         value = newValue;
         if(!liveDragging)
         {
            dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,_value));
         }
         _trackDragMouseIndex = 0;
         _dragOffset = {x:0};
      }
      
      override protected function doDrag(event:MouseEvent) : void
      {
         var localPoint:Point = globalToLocal(new Point(event.stageX,event.stageY));
         var posX:Number = localPoint.x - (_dragOffset ? _dragOffset.x : 0);
         var trackLength:Number = this._trackWidth - offsetLeft - offsetRight;
         var newValue:Number = (posX - offsetLeft) / trackLength * (_maximum - _minimum) + _minimum;
         var nextValue:Number;
         var snappedValue:Number = lockValue(newValue);
         if(newValue > snappedValue)
         {
            nextValue = snappedValue + _snapInterval;
         }
         else
         {
            nextValue = snappedValue - _snapInterval;
         }
         newValue = lockValue(newValue);
         if(event.target == thumb)
         {
            if(value == newValue)
            {
               return;
            }
            value = newValue;
         }
         this.updateThumb();
         if(liveDragging)
         {
            dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,_value));
         }
      }
      
      override protected function updateThumb() : void
      {
         trace("updateThumb",this._hoverCells,enabled);
         if(!enabled)
         {
            return;
         }
         setChildIndex(track,0);
         var i:uint = 0;
         var cell:MovieClip;
         var cellValue:uint;
         while(i < this._cells.length)
         {
            cell = this._cells[i];
            cellValue = i * snapInterval;
            if(cellValue >= this._selectValue && cellValue < value && this._selectValue < value || cellValue >= value && cellValue < this._selectValue && this._selectValue >= value)
            {
               cell.gotoAndStop(3);
            }
            else if(cellValue < value)
            {
               cell.gotoAndStop(4);
            }
            else if(cellValue < this.total)
            {
               cell.gotoAndStop(2);
               if(this._hoverCells)
               {
                  cell.gotoAndPlay("blink");
               }
            }
            else
            {
               cell.gotoAndStop(1);
            }
            i++;
         }
         if(cell)
         {
            track.height = cell.height;
            track.width = this.total / snapInterval * (cell.width + this.shiftInterval);
         }
      }
      
      public function hoverCells(hover:Boolean) : void
      {
         trace("hoverCells");
         _hoverCells = hover;
         this.updateThumb();
      }
      
      override protected function draw() : void
      {
         var i:uint;
         var cellCount:uint;
         var j:uint;
         var cell:MovieClip;
         var cellWidth:uint;
         gotoAndPlay(!enabled ? "disabled" : (_focused ? "focused" : "default"));
         if(isInvalid(InvalidationType.SETTINGS))
         {
            i = 0;
            while(i < this._cells.length)
            {
               cell = this._cells[i];
               cell.gotoAndStop(1);
               removeChild(cell);
               i++;
            }
            this._cells.length = 0;
            cellCount = maximum / snapInterval;
            j = 0;
            while(j < cellCount)
            {
               cell = new this._itemRenderer();
               cell.mouseEnabled = false;
               cell.mouseChildren = false;
               this._cells[j] = cell;
               cell.gotoAndStop(1);
               if(this._scaleRenderers)
               {
                  cellWidth = 20 * this._maxSize / (maximum / snapInterval);
                  cell.width = cellWidth;
               }
               else
               {
                  cellWidth = cell.width;
               }
               cell.x = 2 + j * (cellWidth + this.shiftInterval);
               addChild(cell);
               j++;
            }
            if(cell)
            {
               _trackWidth = cellCount * (cell.width + this.shiftInterval);
            }
         }
         this.updateThumb();
      }
   }
}

