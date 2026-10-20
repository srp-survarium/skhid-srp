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
      
      public function set minimum_storage(param1:int) : void
      {
         _minimum_storage = param1;
      }
      
      public function get minimum_storage() : int
      {
         return this._minimum_storage;
      }
      
      public function set required(param1:int) : void
      {
         if(param1 < minimum)
         {
            param1 = minimum;
         }
         else if(param1 > maximum)
         {
            param1 = maximum;
         }
         _min_request = param1;
      }
      
      public function get required() : int
      {
         return this._min_request;
      }
      
      public function set available(param1:int) : void
      {
         _available = param1;
      }
      
      public function get available() : int
      {
         return this._available;
      }
      
      public function set total(param1:int) : void
      {
         _total = param1;
         this.updateThumb();
      }
      
      public function get total() : int
      {
         return this._total;
      }
      
      public function set maxSize(param1:uint) : void
      {
         _maxSize = param1;
      }
      
      public function get maxSize() : uint
      {
         return this._maxSize;
      }
      
      public function set value_storage(param1:int) : void
      {
         if(param1 < this._minimum_storage)
         {
            param1 = this._minimum_storage;
         }
         if(param1 < this.available)
         {
            param1 = this.available;
         }
         if(param1 < minimum)
         {
            param1 = minimum;
         }
         _value_storage = param1;
         if(value > this._value_storage)
         {
            value = this._value_storage;
         }
         dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,param1));
         this.draw();
      }
      
      public function get value_storage() : int
      {
         return this._value_storage;
      }
      
      public function set renderer(param1:String) : void
      {
         if(param1 == "")
         {
            return;
         }
         var _loc2_:Class = getDefinitionByName(param1) as Class;
         if(_loc2_ != null)
         {
            _itemRenderer = _loc2_;
         }
         else
         {
            trace("Error: " + this + ", The class " + param1 + " cannot be found in your library. Please ensure it is there.");
         }
      }
      
      override public function set value(param1:Number) : void
      {
         if(param1 < minimum)
         {
            param1 = minimum;
         }
         else if(param1 > maximum)
         {
            param1 = maximum;
         }
         _value = param1;
         if(this._value_storage < param1)
         {
            value_storage = param1;
         }
         _selectValue = _value;
         dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,_value));
         this.draw();
      }
      
      public function set scaleRenders(param1:Boolean) : void
      {
         _scaleRenderers = param1;
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
      
      override public function handleInput(param1:InputEvent) : void
      {
         if(param1.isDefaultPrevented())
         {
            return;
         }
         var _loc2_:InputDetails = param1.details;
         var _loc3_:uint = _loc2_.controllerIndex;
         var _loc4_:Boolean = _loc2_.value == InputValue.KEY_DOWN || _loc2_.value == InputValue.KEY_HOLD;
         switch(_loc2_.navEquivalent)
         {
            case NavigationCode.RIGHT:
               if(_loc4_ && value < this.total * _snapInterval)
               {
                  value = value + _snapInterval;
                  param1.handled = true;
               }
               break;
            case NavigationCode.LEFT:
               if(_loc4_)
               {
                  value = value - _snapInterval;
                  param1.handled = true;
               }
               break;
            case NavigationCode.HOME:
               if(!_loc4_)
               {
                  value = minimum;
                  param1.handled = true;
               }
               break;
            case NavigationCode.END:
               if(!_loc4_)
               {
                  value = maximum;
                  param1.handled = true;
               }
            break;
            default:
               break;
         }
      }
      
      protected function onMouseOver(param1:MouseEvent) : void
      {
         track.addEventListener(MouseEvent.MOUSE_MOVE,this.onMouseMove);
      }
      
      protected function onMouseOut(param1:MouseEvent) : void
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
      
      protected function onMouseMove(param1:MouseEvent) : void
      {
         var _loc2_:Point = globalToLocal(new Point(param1.stageX,param1.stageY));
         var _loc3_:Number = _loc2_.x - (_dragOffset ? _dragOffset.x : 0);
         var _loc4_:Number = this._trackWidth - offsetLeft - offsetRight;
         var _loc5_:Number = (_loc3_ - offsetLeft) / _loc4_ * (_maximum - _minimum) + _minimum;
         var _loc6_:uint;
         _loc5_ = lockValue(_loc5_);
         if(_loc5_ > this.total)
         {
            _loc5_ = this.total;
         }
         if(this._selectValue != _loc5_)
         {
            _selectValue = _loc5_;
            _loc6_ = this._selectValue >= _value ? 10 : 11;
            track.data = {name:LocalizationData.getName(_loc6_ == 10 ? "add" : "remove")};
            track.invalidateData();
            ExternalInterface.call("set_mouse_cursor",_loc6_);
            this.updateThumb();
         }
      }
      
      override protected function trackPress(param1:MouseEvent) : void
      {
         _trackPressed = true;
         track.focused = _focused;
         var _loc2_:Number = this._trackWidth - offsetLeft - offsetRight;
         var _loc3_:Point = globalToLocal(new Point(param1.stageX,param1.stageY));
         var _loc4_:Number = _loc3_.x - (_dragOffset ? _dragOffset.x : 0);
         var _loc5_:Number = lockValue((_loc4_ - offsetLeft) / _loc2_ * (_maximum - _minimum) + _minimum);
         if(value == _loc5_ || _loc5_ > this.total)
         {
            return;
         }
         value = _loc5_;
         if(!liveDragging)
         {
            dispatchEvent(new SliderEvent(SliderEvent.VALUE_CHANGE,false,true,_value));
         }
         _trackDragMouseIndex = 0;
         _dragOffset = {x:0};
      }
      
      override protected function doDrag(param1:MouseEvent) : void
      {
         var _loc2_:Point = globalToLocal(new Point(param1.stageX,param1.stageY));
         var _loc3_:Number = _loc2_.x - (_dragOffset ? _dragOffset.x : 0);
         var _loc4_:Number = this._trackWidth - offsetLeft - offsetRight;
         var _loc5_:Number = (_loc3_ - offsetLeft) / _loc4_ * (_maximum - _minimum) + _minimum;
         var _loc6_:Number;
         var _loc7_:Number = lockValue(_loc5_);
         if(_loc5_ > _loc7_)
         {
            _loc6_ = _loc7_ + _snapInterval;
         }
         else
         {
            _loc6_ = _loc7_ - _snapInterval;
         }
         _loc5_ = lockValue(_loc5_);
         if(param1.target == thumb)
         {
            if(value == _loc5_)
            {
               return;
            }
            value = _loc5_;
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
         var _loc1_:uint = 0;
         var _loc2_:MovieClip;
         var _loc3_:uint;
         while(_loc1_ < this._cells.length)
         {
            _loc2_ = this._cells[_loc1_];
            _loc3_ = _loc1_ * snapInterval;
            if(_loc3_ >= this._selectValue && _loc3_ < value && this._selectValue < value || _loc3_ >= value && _loc3_ < this._selectValue && this._selectValue >= value)
            {
               _loc2_.gotoAndStop(3);
            }
            else if(_loc3_ < value)
            {
               _loc2_.gotoAndStop(4);
            }
            else if(_loc3_ < this.total)
            {
               _loc2_.gotoAndStop(2);
               if(this._hoverCells)
               {
                  _loc2_.gotoAndPlay("blink");
               }
            }
            else
            {
               _loc2_.gotoAndStop(1);
            }
            _loc1_++;
         }
         if(_loc2_)
         {
            track.height = _loc2_.height;
            track.width = this.total / snapInterval * (_loc2_.width + this.shiftInterval);
         }
      }
      
      public function hoverCells(param1:Boolean) : void
      {
         trace("hoverCells");
         _hoverCells = param1;
         this.updateThumb();
      }
      
      override protected function draw() : void
      {
         var _loc1_:uint;
         var _loc2_:uint;
         var _loc3_:uint;
         var _loc4_:MovieClip;
         var _loc5_:uint;
         gotoAndPlay(!enabled ? "disabled" : (_focused ? "focused" : "default"));
         if(isInvalid(InvalidationType.SETTINGS))
         {
            _loc1_ = 0;
            while(_loc1_ < this._cells.length)
            {
               _loc4_ = this._cells[_loc1_];
               _loc4_.gotoAndStop(1);
               removeChild(_loc4_);
               _loc1_++;
            }
            this._cells.length = 0;
            _loc2_ = maximum / snapInterval;
            _loc3_ = 0;
            while(_loc3_ < _loc2_)
            {
               _loc4_ = new this._itemRenderer();
               _loc4_.mouseEnabled = false;
               _loc4_.mouseChildren = false;
               this._cells[_loc3_] = _loc4_;
               _loc4_.gotoAndStop(1);
               if(this._scaleRenderers)
               {
                  _loc5_ = 20 * this._maxSize / (maximum / snapInterval);
                  _loc4_.width = _loc5_;
               }
               else
               {
                  _loc5_ = _loc4_.width;
               }
               _loc4_.x = 2 + _loc3_ * (_loc5_ + this.shiftInterval);
               addChild(_loc4_);
               _loc3_++;
            }
            if(_loc4_)
            {
               _trackWidth = _loc2_ * (_loc4_.width + this.shiftInterval);
            }
         }
         this.updateThumb();
      }
   }
}

