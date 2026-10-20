package survarium.item
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextFieldAutoSize;
   import flash.utils.Timer;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.managers.DragManager;
   import scaleform.gfx.MouseEventEx;
   import survarium.Util;
   import survarium.constants.Events;
   import survarium.controls.ItemRendererCore;
   import survarium.controls.ListDragSlot;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IInventoryItemRenderer;
   import survarium.upgrade.UpgradeWindow;
   
   public class InventoryItem extends ListDragSlot implements IInventoryItemRenderer
   {
      
      public var itemRenderer:ItemRendererCore;
      
      public var glow:MovieClip;
      
      public var lockedText:MovieClip;
      
      protected var _isBundle:Boolean = false;
      
      protected var _hasListener:Boolean = false;
      
      protected var _startTime:uint = 0;
      
      protected var _mouseDown:int = 0;
      
      public function InventoryItem()
      {
         super();
      }
      
      override public function get data() : Object
      {
         return _data;
      }
      
      override public function set data(value:Object) : void
      {
         _data = value;
         invalidateData();
      }
      
      override public function set y(value:Number) : void
      {
         super.y = value;
         this.updateGlow();
      }
      
      override public function setData(data:Object) : void
      {
         var timer:Timer;
         this.data = data;
         trace("itemSetData",name);
         if(_data)
         {
            _descriptor = _data.descriptor;
            _isBundle = _data.descriptor == 300;
         }
         if(_data && _data.repairTime)
         {
            if(!this._hasListener)
            {
               _hasListener = true;
               timer = MainScene.getTimerInstance();
               timer.addEventListener(TimerEvent.TIMER,this.updateRepairTime);
               this.updateRepairTime();
               trace("addTimerEventListener::",this.updateRepairTime,_data.repairTime);
            }
         }
         else if(this._hasListener)
         {
            _hasListener = false;
            MainScene.getTimerInstance().removeEventListener(TimerEvent.TIMER,this.updateRepairTime);
         }
         this.setState(_state);
      }
      
      public function startItemDrag() : void
      {
         var dragStartEvent:DragEvent = new DragEvent(DragEvent.DRAG_START,_data,this,null,_content);
         dispatchEvent(dragStartEvent);
         this.handleDragStartEvent(dragStartEvent);
      }
      
      public function hover(value:Boolean = true) : void
      {
         trace("hover::",value,selected,_data ? _data.name : null);
         if(_selected)
         {
            return;
         }
         if(value == true)
         {
            this.setState("over");
         }
         else if(_state == "over")
         {
            this.setState("out");
         }
      }
      
      public function hideContent(hide:Boolean, full:Boolean) : void
      {
         var itemAlpha:Number = full ? 0 : (hide ? 0.5 : 1);
      }
      
      public function initDrag() : void
      {
         if(DragManager.inDrag() || !enabled)
         {
            return;
         }
         if(this.data != null)
         {
            trace("IIS::initDrag");
            stage.addEventListener(MouseEvent.MOUSE_MOVE,this.handleMouseMove,false,0,true);
            _mouseDownX = mouseX;
            _mouseDownY = mouseY;
         }
      }
      
      override public function handleDragStartEvent(e:DragEvent) : void
      {
         trace("II:handleDragStartEvent",e);
         ExternalInterface.call("handle_drag_start",_data ? _data.dictId : 0);
      }
      
      override public function handleDragEndEvent(e:DragEvent, wasValidDrop:Boolean) : void
      {
         if(!e.dropTarget || e.dropTarget is InventoryItem)
         {
            trace("handle_drag_end",_data ? _data.dictId : 0);
            ExternalInterface.call("handle_drag_end",_data ? _data.dictId : 0);
         }
      }
      
      override public function handleDropEvent(e:DragEvent) : Boolean
      {
         return false;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         addEventListener(MouseEvent.CLICK,this.handleMouseUp,false,0,true);
         addEventListener(MouseEvent.DOUBLE_CLICK,this.handleMouseUp,false,0,true);
         buttonMode = true;
         doubleClickEnabled = true;
         mouseChildren = false;
         dragEnabled = true;
         this.glow.mouseChildren = false;
         this.glow.mouseEnabled = false;
         this.lockedText.mouseEnabled = false;
      }
      
      override protected function cleanupDragListeners() : void
      {
         if(stage)
         {
            stage.removeEventListener(MouseEvent.MOUSE_MOVE,this.handleMouseMove,false);
         }
         else
         {
            trace("noStageOnCleanUp");
         }
         _mouseDownX = undefined;
         _mouseDownY = undefined;
      }
      
      override public function toString() : String
      {
         return "[InventoryItem " + name + "]";
      }
      
      override protected function handleMouseDown(event:MouseEvent) : void
      {
         var sfEvent:MouseEventEx = event as MouseEventEx;
         var mouseIdx:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
         var btnIdx:uint = sfEvent == null ? 0 : sfEvent.buttonIdx;
         _mouseDown |= 1 << mouseIdx;
         if(enabled)
         {
            this.setState("down");
            this.initDrag();
            var sfButtonEvent:ButtonEvent = new ButtonEvent(ButtonEvent.PRESS,true,false,mouseIdx,btnIdx,false,false);
            dispatchEvent(sfButtonEvent);
         }
      }
      
      override protected function handleMouseMove(e:MouseEvent) : void
      {
         var dragStartEvent:DragEvent;
         trace("IIS:handleMouseMove",dragEnabled);
         if(mouseX > _mouseDownX + 3 || mouseX < _mouseDownX - 3 || mouseY > _mouseDownY + 3 || mouseY < _mouseDownY - 3)
         {
            this.cleanupDragListeners();
            if(!dragEnabled)
            {
               return;
            }
            trace("IIS:dragStart");
            dragStartEvent = new DragEvent(DragEvent.DRAG_START,_data,this,null,_content);
            dispatchEvent(dragStartEvent);
            this.handleDragStartEvent(dragStartEvent);
         }
      }
      
      override protected function handleMouseUp(event:MouseEvent) : void
      {
         if(!enabled)
         {
            return;
         }
         var sfEvent:MouseEventEx = event as MouseEventEx;
         var mouseIdx:uint = sfEvent == null ? 0 : sfEvent.mouseIdx;
         var btnIdx:uint = sfEvent == null ? 0 : sfEvent.buttonIdx;
         _mouseDown ^= 1 << mouseIdx;
         this.setState("release");
         this.cleanupDragListeners();
         trace("IISlot_clciked");
         var sfButtonEvent:ButtonEvent = new ButtonEvent(ButtonEvent.CLICK,true,false,mouseIdx,btnIdx,false,false);
         dispatchEvent(sfButtonEvent);
      }
      
      protected function updateGlow() : void
      {
         var mask:Sprite;
         var scale:Number;
         if(parent && parent.parent.mask)
         {
            if(_data && _data.isNew)
            {
               mask = parent.parent.mask as Sprite;
               scale = parent.parent.scaleY;
               if(y < mask.height * scale)
               {
                  if(this.glow && this.glow.currentFrame == 1 || this.glow.currentFrame == this.glow.totalFrames)
                  {
                     this.glow.gotoAndPlay(2);
                  }
               }
               if(y + height - 5 < mask.height * scale)
               {
                  _data.isNew = false;
                  _data.showed = true;
                  dispatchEvent(new Event(Events.NEW_ITEMS_COUNT_CHANGED,true));
               }
            }
            else if((isInvalid(InvalidationType.DATA) && this.glow && _data) && this.glow.currentFrame != 1 && !_data.showed)
            {
               this.glow.gotoAndStop(1);
            }
         }
      }
      
      protected function removeGlow() : void
      {
         if(_data && _data.isNew == true && this.glow.currentFrame != 1)
         {
            _data.isNew = false;
            this.glow.gotoAndStop(1);
         }
      }
      
      protected function updateRepairTime(e:Event = null) : void
      {
         var _itemData:ItemData = this._data as ItemData;
         if(_itemData.descriptor == 100)
         {
            trace("updateRTfor::",_itemData.name,_itemData.repairTime,_itemData.repairTimeDelta,MainScene.getTimerInstance().currentCount);
            var delta:int = MainScene.getTimerInstance().currentCount - _itemData.repairTimeDelta;
            if(_itemData.repairTime != 0)
            {
               this.lockedText.visible = true;
               this.lockedText.textFieldLock.autoSize = TextFieldAutoSize.LEFT;
               this.lockedText.textFieldLock.text = LocalizationData.getName("repair_time") + ": " + Util.getTimeString(_itemData.repairTime - delta);
               this.lockedText.textFieldLock.width = Math.min(this.lockedText.textFieldLock.textWidth + 5,157);
               this.lockedText.x = (181 - this.lockedText.width) / 2;
            }
            else
            {
               this.lockedText.visible = false;
            }
         }
      }
      
      override protected function updateSlot() : void
      {
         var locked:Boolean = false;
         var _itemData:ItemData = this._data as ItemData;
         if(Boolean(_itemData))
         {
            locked = _itemData.descriptor == 200 && !_itemData.canBuy;
            this.lockedText.visible = locked || _itemData.repairTime != 0;
            if(locked)
            {
               this.lockedText.textFieldLock.autoSize = TextFieldAutoSize.LEFT;
               this.lockedText.textFieldLock.text = _data.unlockLevelLabel;
               this.lockedText.textFieldLock.width = Math.min(this.lockedText.textFieldLock.textWidth + 5,157);
               this.lockedText.x = (181 - this.lockedText.width) / 2;
            }
            _content = _itemData.icon;
         }
         else
         {
            _content = null;
         }
         if(Boolean(contentCanvas))
         {
            contentCanvas.visible = false;
         }
         if(Boolean(this.itemRenderer))
         {
            this.itemRenderer.data = _itemData;
            this.itemRenderer.invalidateData();
            this.itemRenderer.validateNow();
            this.itemRenderer.alpha = UpgradeWindow.greyed(_itemData) ? 0.35 : 1;
         }
      }
      
      override protected function updateSlotState() : void
      {
         var _itemData:ItemData = null;
         var locked:Boolean = false;
         trace("updateSlotState",this.itemRenderer);
         if(Boolean(this.itemRenderer) && Boolean(_data))
         {
            _itemData = this._data as ItemData;
            locked = _itemData.descriptor == 200 && !_itemData.canBuy || _itemData.repairTime != 0 || UpgradeWindow.greyed(_itemData);
            this.itemRenderer.alpha = UpgradeWindow.greyed(_itemData) ? 0.35 : 1;
            if(locked)
            {
               this.itemRenderer.updateState(2);
            }
            else
            {
               this.itemRenderer.updateState(_selected || _state == "over" ? 0 : 1);
            }
            this.itemRenderer.y = _state == "down" ? 1 : 0;
         }
      }
      
      protected function getStatePrefix() : String
      {
         if(_selected)
         {
            return "selected_";
         }
         return "";
      }
      
      override protected function setState(state:String) : void
      {
         _state = state;
         var states:Array = _stateMap[state];
         var prefix:String = this.getStatePrefix();
         if(states == null || states.length == 0)
         {
            return;
         }
         var sl:uint = states.length;
         for(var j:uint = 0; j < sl; j++)
         {
            var thisLabel:String = prefix + states[j];
            if(_labelHash[thisLabel])
            {
               _newFrame = thisLabel;
               invalidateState();
               return;
            }
         }
      }
      
      protected function handleDragEnabling() : void
      {
         var _itemData:ItemData = null;
         var locked:Boolean = false;
         if(Boolean(_data))
         {
            _itemData = this._data as ItemData;
            if(_itemData.descriptor == 300)
            {
               dragEnabled = false;
               return;
            }
            locked = _itemData.descriptor == 200 && !_itemData.canBuy && (!_itemData.isStack || _itemData.isStack && _itemData.value == 0);
            dragEnabled = !_itemData.repairTime && !locked && _itemData.descriptor != 0 && !UpgradeWindow.greyed(_itemData);
         }
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.DATA))
         {
            this.handleDragEnabling();
         }
         visible = _data != null;
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateGlow();
         }
         mouseChildren = false;
      }
   }
}

