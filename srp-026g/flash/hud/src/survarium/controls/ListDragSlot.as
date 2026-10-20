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
      
      override public function set enabled(value:Boolean) : void
      {
         var state:String;
         super.enabled = value;
         mouseChildren = true;
         if(super.enabled)
         {
            state = _displayFocus || _focused ? "over" : "up";
         }
         else
         {
            state = "disabled";
         }
         setState(state);
      }
      
      public function get owner() : UIComponent
      {
         return this._owner;
      }
      
      public function set owner(value:UIComponent) : void
      {
         _owner = value;
      }
      
      public function get selectable() : Boolean
      {
         return this._selectable;
      }
      
      public function set selectable(value:Boolean) : void
      {
         _selectable = value;
      }
      
      public function get selected() : Boolean
      {
         return this._selected;
      }
      
      public function set selected(value:Boolean) : void
      {
         _selected = value;
      }
      
      public function set disabled(value:Boolean) : void
      {
         enabled = !value;
         if(value == true)
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
      
      override public function set data(value:Object) : void
      {
         if(_data == value)
         {
            return;
         }
         _data = value;
         invalidateData();
      }
      
      public function get index() : uint
      {
         return this._index;
      }
      
      public function set index(value:uint) : void
      {
         _index = value;
         _descriptor = value;
      }
      
      public function setListData(listData:ListData) : void
      {
         index = listData.index;
         selected = listData.selected;
      }
      
      public function setData(data:Object) : void
      {
         this.data = data;
      }
      
      override public function handleDragStartEvent(e:DragEvent) : void
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

