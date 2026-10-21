package survarium.controls
{
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.DragSlot;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.ListData;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.interfaces.IListItemRenderer;
   
   public class ListDragSlot extends DragSlot implements IListItemRenderer
   {
      
      protected var _index:uint = 0;
      
      protected var _owner:UIComponent;
      
      protected var _selectable:Boolean = true;
      
      protected var _selected:Boolean = false;
      
      protected var _draggable:Boolean = true;
      
      public var slotId:int;
      
      public function ListDragSlot()
      {
         super();
         _stateMap = {
            up:["up"],
            locked:["locked"],
            over:["over"],
            down:["down"],
            release:["over"],
            out:["up"],
            disabled:["disabled"],
            selecting:["over"]
         };
      }
      
      override public function get enabled() : Boolean
      {
         return super.enabled;
      }
      
      override public function set enabled(param1:Boolean) : void
      {
         var _loc2_:String;
         super.enabled = param1;
         mouseChildren = true;
         if(super.enabled)
         {
            _loc2_ = _displayFocus || _focused ? "over" : "up";
         }
         else
         {
            _loc2_ = "disabled";
         }
         setState(_loc2_);
      }
      
      public function get owner() : UIComponent
      {
         return this._owner;
      }
      
      public function set owner(param1:UIComponent) : void
      {
         _owner = param1;
      }
      
      public function get selectable() : Boolean
      {
         return this._selectable;
      }
      
      public function set selectable(param1:Boolean) : void
      {
         _selectable = param1;
      }
      
      public function get selected() : Boolean
      {
         return this._selected;
      }
      
      public function set selected(param1:Boolean) : void
      {
         _selected = param1;
      }
      
      public function set disabled(param1:Boolean) : void
      {
         enabled = !param1;
         if(param1 == true)
         {
            setState("disabled");
         }
         else
         {
            setState("out");
         }
      }
      
      public function get disabled() : Boolean
      {
         return !this.enabled;
      }
      
      override public function get data() : Object
      {
         return _data;
      }
      
      override public function set data(param1:Object) : void
      {
         if(_data == param1)
         {
            return;
         }
         _data = param1;
         invalidateData();
      }
      
      public function get index() : uint
      {
         return this._index;
      }
      
      public function set index(param1:uint) : void
      {
         _index = param1;
         _descriptor = param1;
      }
      
      public function setListData(param1:ListData) : void
      {
         index = param1.index;
         selected = param1.selected;
      }
      
      public function setData(param1:Object) : void
      {
         this.data = param1;
      }
      
      override public function handleDragStartEvent(param1:DragEvent) : void
      {
         setState("over");
      }
      
      protected function updateSlot() : void
      {
      }
      
      protected function updateSlotState() : void
      {
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateSlot();
         }
         if(isInvalid(InvalidationType.STATE))
         {
            this.updateSlotState();
         }
      }
      
      override public function toString() : String
      {
         return "[ListDragSlot " + name + "]";
      }
   }
}

