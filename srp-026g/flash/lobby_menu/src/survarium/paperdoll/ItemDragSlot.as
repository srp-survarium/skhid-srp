package survarium.paperdoll
{
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.managers.DragManager;
   import scaleform.gfx.MouseEventEx;
   import survarium.constants.Events;
   import survarium.controls.ItemRendererCore;
   import survarium.controls.ListDragSlot;
   import survarium.data.ItemData;
   import survarium.interfaces.IPaperDollSlot;
   
   public class ItemDragSlot extends ListDragSlot implements IPaperDollSlot
   {
      
      public var itemRenderer:ItemRendererCore;
      
      public var textFieldLabel:TextField;
      
      protected var _artifact:Boolean = false;
      
      protected var _label:String = "";
      
      protected var _highlighted:Boolean = false;
      
      protected var _isTemponary:Boolean = false;
      
      protected var statesEmpty:String = "empty_";
      
      protected var _realData:Object;
      
      protected var _rendererY:int;
      
      protected var _drawMultilineType:Boolean = false;
      
      protected var statesHighlighted:String = "highlighted_";
      
      protected var statesHighlightedEmpty:String = "highlighted_empty_";
      
      public function ItemDragSlot()
      {
         super();
      }
      
      public function set label(param1:String) : void
      {
         _label = param1;
         invalidateState();
         invalidateData();
      }
      
      public function get label() : String
      {
         return this._label;
      }
      
      public function get artifact() : Boolean
      {
         return this._artifact;
      }
      
      public function set artifact(param1:Boolean) : void
      {
         _artifact = param1;
      }
      
      public function get highlighted() : Boolean
      {
         return this._highlighted;
      }
      
      public function set highlighted(param1:Boolean) : void
      {
         _highlighted = param1;
         if(enabled || param1)
         {
            this.setState("out");
         }
         else
         {
            this.setState("disabled");
         }
      }
      
      override public function get data() : Object
      {
         return _data;
      }
      
      override public function set data(param1:Object) : void
      {
         if(_data != param1 && param1 && selected)
         {
            _data = param1;
            addEventListener(Event.RENDER,this.onResetClick);
         }
         else
         {
            _data = param1;
         }
         this.setState(_state);
         invalidateData();
      }
      
      public function hover(param1:Boolean = true) : void
      {
         if(selected)
         {
            return;
         }
         this.setState(param1 ? "over" : "out");
      }
      
      public function hideContent(param1:Boolean, param2:Boolean) : void
      {
         var _loc3_:Number = param2 ? 0 : (param1 ? 0.5 : 1);
      }
      
      public function drawMultilineType(param1:Boolean) : void
      {
         _drawMultilineType = param1;
      }
      
      override public function setData(param1:Object) : void
      {
         _isTemponary = false;
         _realData = null;
         this.data = param1;
      }
      
      public function reset() : void
      {
         this.data = this._realData;
      }
      
      public function isTemponary() : Boolean
      {
         return this._isTemponary;
      }
      
      public function dropSlot(param1:Boolean = true) : void
      {
         if(!this.data)
         {
            return;
         }
         _realData = _data as ItemData;
         this.data = null;
         _isTemponary = true;
         dispatchEvent(new Event(Events.DROPED_IN));
      }
      
      override public function handleDragStartEvent(param1:DragEvent) : void
      {
         this.setState("over");
         this.data = null;
         dispatchEvent(new Event(Events.DROPED_IN));
      }
      
      override public function handleDragEndEvent(param1:DragEvent, param2:Boolean) : void
      {
         this.data = param1.dragData;
         if(param1.dropTarget == null || param1.dropTarget is IPaperDollSlot || !param2 && param1.dropTarget != null && !(param1.dropTarget is IPaperDollSlot))
         {
            this.dropSlot(false);
         }
      }
      
      override public function handleDropEvent(param1:DragEvent) : Boolean
      {
         if(enabled == false || !param1.dropTarget)
         {
            return false;
         }
         trace("relocateFabrica:DROP_IN",name);
         dispatchEvent(new Event(Events.DROP_IN));
         _realData = _data as ItemData;
         _isTemponary = true;
         this.data = (param1.dragData as ItemData).clone();
         dispatchEvent(new Event(Events.DROPED_IN));
         return true;
      }
      
      override public function toString() : String
      {
         return "[ItemDragSlot " + name + "]";
      }
      
      protected function trace_relocate(param1:Array) : void
      {
         trace("trace_relocate",param1.length);
         var _loc2_:int = 0;
         while(_loc2_ < param1.length)
         {
            trace("items is:",param1[_loc2_].profile,param1[_loc2_].id,param1[_loc2_].dict_id,"sr:",param1[_loc2_].sourceSlot,"tg:",param1[_loc2_].targetSlot,param1[_loc2_].count);
            _loc2_++;
         }
      }
      
      override protected function handleMouseUp(param1:MouseEvent) : void
      {
         this.setState("up");
         super.handleMouseUp(param1);
      }
      
      override protected function handleMouseDown(param1:MouseEvent) : void
      {
         this.setState("down");
         var _loc2_:MouseEventEx = param1 as MouseEventEx;
         var _loc3_:uint = _loc2_ == null ? 0 : _loc2_.mouseIdx;
         var _loc4_:uint = _loc2_ == null ? 0 : _loc2_.buttonIdx;
         if(DragManager.inDrag() || !enabled || _loc4_ != 0)
         {
            return;
         }
         if(dragEnabled)
         {
            stage.addEventListener(MouseEvent.MOUSE_UP,this.handleMouseUp,false,0,true);
            stage.addEventListener(MouseEvent.MOUSE_MOVE,handleMouseMove,false,0,true);
            _mouseDownX = mouseX;
            _mouseDownY = mouseY;
         }
      }
      
      protected function onResetClick(param1:Event) : void
      {
         removeEventListener(Event.RENDER,this.onResetClick);
         dispatchEvent(new Event(Events.DROP_IN));
      }
      
      override protected function updateSlot() : void
      {
         var _loc1_:ItemData = this._data as ItemData;
         trace("ItemDragSlot::updateData",name,_loc1_);
         if(_loc1_)
         {
            _content = _loc1_.icon;
            contentCanvas.visible = false;
         }
         else
         {
            _content = null;
         }
         if(this.itemRenderer)
         {
            this.itemRenderer.data = _loc1_;
            this.itemRenderer.invalidateData();
            this.itemRenderer.validateNow();
         }
      }
      
      override protected function updateSlotState() : void
      {
         trace("updateSlotState",this.itemRenderer);
         if(this.itemRenderer)
         {
            this.itemRenderer.updateState(_selected || _state == "over" ? 0 : 1);
            this.itemRenderer.y = this._rendererY + (_state == "down" ? 1 : 0);
         }
      }
      
      protected function getStatePrefix() : String
      {
         if(!_data)
         {
            return this._highlighted ? this.statesHighlightedEmpty : this.statesEmpty;
         }
         if(_selected || _state == "over" && this._highlighted)
         {
            return "";
         }
         return this._highlighted || _selected ? this.statesHighlighted : "";
      }
      
      override protected function setState(param1:String) : void
      {
         _state = param1;
         var _loc2_:Array = _stateMap[param1];
         var _loc3_:String = this.getStatePrefix();
         trace("IDS::setState",param1,_loc3_);
         if(_loc2_ == null || _loc2_.length == 0)
         {
            return;
         }
         var _loc4_:uint = _loc2_.length;
         var _loc5_:uint = 0;
         var _loc6_:String;
         while(_loc5_ < _loc4_)
         {
            _loc6_ = _loc3_ + _loc2_[_loc5_];
            if(_labelHash[_loc6_])
            {
               _newFrame = _loc6_;
               invalidateState();
               return;
            }
            _loc5_++;
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.setState("up");
         if(this.itemRenderer)
         {
            _rendererY = this.itemRenderer.y;
         }
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(!_data && isInvalid(InvalidationType.STATE) && this.textFieldLabel)
         {
            this.textFieldLabel.text = this.label;
         }
      }
   }
}

