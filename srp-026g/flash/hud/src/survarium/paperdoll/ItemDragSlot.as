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
      
      public function set label(value:String) : void
      {
         _label = value;
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
      
      public function set artifact(value:Boolean) : void
      {
         _artifact = value;
      }
      
      public function get highlighted() : Boolean
      {
         return this._highlighted;
      }
      
      public function set highlighted(value:Boolean) : void
      {
         _highlighted = value;
         if(enabled || value)
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
      
      override public function set data(value:Object) : void
      {
         if(_data != value && value && selected)
         {
            _data = value;
            addEventListener(Event.RENDER,this.onResetClick);
         }
         else
         {
            _data = value;
         }
         this.setState(_state);
         invalidateData();
      }
      
      public function hover(value:Boolean = true) : void
      {
         if(selected)
         {
            return;
         }
         this.setState(value ? "over" : "out");
      }
      
      public function hideContent(hide:Boolean, full:Boolean) : void
      {
         var itemAlpha:Number = full ? 0 : (hide ? 0.5 : 1);
      }
      
      public function drawMultilineType(value:Boolean) : void
      {
         _drawMultilineType = value;
      }
      
      override public function setData(data:Object) : void
      {
         _isTemponary = false;
         _realData = null;
         this.data = data;
      }
      
      public function reset() : void
      {
         this.data = this._realData;
      }
      
      public function isTemponary() : Boolean
      {
         return this._isTemponary;
      }
      
      public function dropSlot(full:Boolean = true) : void
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
      
      override public function handleDragStartEvent(e:DragEvent) : void
      {
         this.setState("over");
         this.data = null;
         dispatchEvent(new Event(Events.DROPED_IN));
      }
      
      override public function handleDragEndEvent(e:DragEvent, wasValidDrop:Boolean) : void
      {
         this.data = e.dragData;
         if(e.dropTarget == null || e.dropTarget is IPaperDollSlot || !wasValidDrop && e.dropTarget != null && !(e.dropTarget is IPaperDollSlot))
         {
            this.dropSlot(false);
         }
      }
      
      override public function handleDropEvent(e:DragEvent) : Boolean
      {
         if(enabled == false || !e.dropTarget)
         {
            return false;
         }
         trace("relocateFabrica:DROP_IN",name);
         dispatchEvent(new Event(Events.DROP_IN));
         _realData = _data as ItemData;
         _isTemponary = true;
         this.data = (e.dragData as ItemData).clone();
         dispatchEvent(new Event(Events.DROPED_IN));
         return true;
      }
      
      override public function toString() : String
      {
         return "[ItemDragSlot " + name + "]";
      }
      
      protected function trace_relocate(value:Array) : void
      {
         trace("trace_relocate",value.length);
         for(var i:int = 0; i < value.length; i++)
         {
            trace("items is:",value[i].profile,value[i].id,value[i].dict_id,"sr:",value[i].sourceSlot,"tg:",value[i].targetSlot,value[i].count);
         }
      }
      
      override protected function handleMouseUp(e:MouseEvent) : void
      {
         this.setState("up");
         super.handleMouseUp(e);
      }
      
      override protected function handleMouseDown(e:MouseEvent) : void
      {
         this.setState("down");
         var sfEvent:MouseEventEx = e as MouseEventEx;
         var mouseIdx:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
         var btnIdx:uint = sfEvent == null ? 0 : sfEvent.buttonIdx;
         if(DragManager.inDrag() || !enabled || btnIdx != 0)
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
      
      protected function onResetClick(e:Event) : void
      {
         removeEventListener(Event.RENDER,this.onResetClick);
         dispatchEvent(new Event(Events.DROP_IN));
      }
      
      override protected function updateSlot() : void
      {
         var _data:ItemData = this._data as ItemData;
         trace("ItemDragSlot::updateData",name,_data);
         if(_data)
         {
            _content = _data.icon;
            contentCanvas.visible = false;
         }
         else
         {
            _content = null;
         }
         if(this.itemRenderer)
         {
            this.itemRenderer.data = _data;
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
      
      override protected function setState(state:String) : void
      {
         _state = state;
         var states:Array = _stateMap[state];
         var prefix:String = this.getStatePrefix();
         trace("IDS::setState",state,prefix);
         if(states == null || states.length == 0)
         {
            return;
         }
         var sl:uint = states.length;
         var j:uint = 0;
         var thisLabel:String;
         while(j < sl)
         {
            thisLabel = prefix + states[j];
            if(_labelHash[thisLabel])
            {
               _newFrame = thisLabel;
               invalidateState();
               return;
            }
            j++;
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

