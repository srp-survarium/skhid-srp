package survarium.paperdoll
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.events.SliderEvent;
   import survarium.constants.Events;
   import survarium.data.ItemData;
   import survarium.interfaces.ILocalPaperDoll;
   import survarium.interfaces.ILocalPlayerProfile;
   import survarium.interfaces.IPaperDollSlot;
   
   public class ItemPaperDollSlot extends ItemDragSlot
   {
      
      protected static var _relocateArray:Array = new Array();
      
      public function ItemPaperDollSlot()
      {
         super();
      }
      
      override public function set highlighted(param1:Boolean) : void
      {
         trace("IPD::set_highlighted",name,param1,_state);
         _highlighted = param1;
         if(param1)
         {
            setState("over");
         }
         else
         {
            setState("up");
         }
      }
      
      override public function dropSlot(param1:Boolean = true) : void
      {
         if(!data)
         {
            return;
         }
         var _loc2_:ILocalPlayerProfile = this.parent.parent as ILocalPlayerProfile;
         var _loc3_:uint;
         var _loc4_:uint;
         var _loc5_:uint;
         var _loc6_:uint;
         var _loc7_:Boolean;
         var _loc8_:Object;
         if(_loc2_)
         {
            _loc3_ = (_data.itemId);
            _loc4_ = (_loc2_.profileId);
            _loc5_ = (_data.dictId);
            _loc6_ = 100;
            _loc7_ = true;
            _loc8_ = {
               profile:_loc4_,
               id:_loc3_,
               dict_id:_loc5_,
               sourceSlot:index,
               targetSlot:_loc6_,
               count:_data.condition_or_stack as uint
            };
            _relocateArray = [_loc8_];
            _loc2_._relocateFunction(_relocateArray);
         }
         super.dropSlot(param1);
      }
      
      override public function handleDragStartEvent(param1:DragEvent) : void
      {
         var _loc2_:ItemData = _data as ItemData;
         trace("IPD:handleDragStartEvent::",param1);
         if(owner is ILocalPaperDoll && _data)
         {
            (owner as ILocalPaperDoll).selectAgeements(data.category,data.dictId,data.oneHanded,true,data.itemId);
         }
         setState("over");
         this.data = null;
      }
      
      override public function handleDragEndEvent(param1:DragEvent, param2:Boolean) : void
      {
         if(owner is ILocalPaperDoll)
         {
            (owner as ILocalPaperDoll).deselectAgeements();
         }
         this.data = param1.dragData;
         trace("IPD:handleDragEndEvent::",param2,param1.dropTarget);
         if(param1.dropTarget == null || !param2 && param1.dropTarget != null && !(param1.dropTarget is IPaperDollSlot))
         {
            this.dropSlot(false);
         }
      }
      
      override public function handleDropEvent(param1:DragEvent) : Boolean
      {
         trace("PDS:handleDropEvent::",name,enabled,highlighted,param1.dropTarget);
         if(enabled == false || highlighted == false || !param1.dropTarget)
         {
            return false;
         }
         var _loc2_:ILocalPlayerProfile = this.parent.parent as ILocalPlayerProfile;
         var _loc3_:ItemData = param1.dragData as ItemData;
         if(!_loc2_)
         {
            dispatchEvent(new Event(Events.DROP_IN));
            _realData = _data as ItemData;
            this.data = (_loc3_ as ItemData).clone();
            dispatchEvent(new Event(Events.DROPED_IN));
            return true;
         }
         _relocateArray.length = 0;
         var _loc4_:uint = _loc3_.itemId;
         var _loc5_:uint = (_loc2_.profileId);
         var _loc6_:uint = _loc3_.dictId;
         var _loc7_:uint = param1.dragTarget.descriptor;
         var _loc8_:uint;
         var _loc9_:*;
         var _loc10_:int;
         var _loc11_:Object;
         var _loc12_:Object;
         if(param1.dropTarget)
         {
            _loc8_ = param1.dropTarget.descriptor;
         }
         else
         {
            _loc8_ = 100;
         }
         trace("PDS:handleDropEvent::",param1.dragTarget,param1.dropTarget,enabled,highlighted,_loc2_._relocateFunction,"source:",_loc7_);
         if(_loc3_.descriptor == 200 && enabled == true && highlighted == true)
         {
            _loc10_ = (_loc3_.count);
            if(_loc3_.isStack)
            {
               _loc10_ = -1;
            }
            if(index == 7 || index == 10)
            {
               _loc11_ = {
                  profile:_loc5_,
                  id:_loc4_,
                  dict_id:_loc6_,
                  sourceSlot:_loc7_,
                  targetSlot:_loc8_,
                  count:1
               };
               trace_relocate(_relocateArray);
            }
            trace("descriptors::",param1.dragTarget.descriptor,param1.dropTarget.descriptor,_loc3_.descriptor);
            MainScene.buyFunction(_loc2_.profileId,_loc3_.itemId,_loc3_.dictId,_loc3_.descriptor,param1.dropTarget.descriptor,_loc10_,_loc3_.cost,_loc3_.factionId);
            dispatchEvent(new Event(Events.DROP_IN));
            return false;
         }
         if(!_loc3_.isStack)
         {
            _loc12_ = {
               profile:_loc5_,
               id:_loc4_,
               dict_id:_loc6_,
               sourceSlot:_loc7_,
               targetSlot:_loc8_,
               count:1
            };
            trace("relocateFabrica:a");
            _relocateArray = [_loc12_];
            if(index != 7 && index != 10)
            {
               trace_relocate(_relocateArray);
               _loc9_ = _loc2_._relocateFunction(_relocateArray);
            }
            else if(_loc8_ != 100)
            {
               trace_relocate(_relocateArray);
               _loc9_ = _loc2_._relocateFunction(_relocateArray);
            }
            else
            {
               trace_relocate(_relocateArray);
               _loc9_ = _loc2_._relocateFunction(_relocateArray);
            }
         }
         else
         {
            trace("relocateFabrica:isStack");
            if(enabled == true && highlighted == true)
            {
               if(param1.dropTarget && !(param1.dragTarget is IPaperDollSlot))
               {
                  trace("relocateFabrica:other");
                  _relocateArray = [{
                     profile:_loc5_,
                     id:_loc4_,
                     dict_id:_loc6_,
                     sourceSlot:_loc7_,
                     targetSlot:_loc8_,
                     count:1
                  }];
                  trace_relocate(_relocateArray);
                  _loc9_ = _loc2_._relocateFunction(_relocateArray);
               }
               else
               {
                  trace("relocateFabrica:swapQuickSlots");
                  _loc9_ = _loc2_.swapQuickSlots(_loc7_,_loc8_);
               }
            }
         }
         trace("IPD:::DROP_IN");
         dispatchEvent(new Event(Events.DROP_IN));
         _realData = _data as ItemData;
         _isTemponary = true;
         this.data = (_loc3_ as ItemData).clone();
         return true;
      }
      
      override protected function getStatePrefix() : String
      {
         if(!_data && !_realData)
         {
            return _highlighted ? statesHighlightedEmpty : statesEmpty;
         }
         if(_selected || _state == "over" && _highlighted)
         {
            return "";
         }
         return _highlighted || _selected ? statesHighlighted : "";
      }
      
      override protected function updateSlot() : void
      {
         var _loc1_:MovieClip = itemRenderer as MovieClip;
         if(_loc1_ && _loc1_.slider)
         {
            _loc1_.slider.removeEventListener(SliderEvent.VALUE_CHANGE,this.onRendererSliderChange);
         }
         if(_data)
         {
            _content = _data.icon;
            contentCanvas.visible = false;
         }
         else
         {
            _content = null;
         }
         super.updateSlot();
         if(_loc1_ && _loc1_.slider)
         {
            trace("itemRenderer.addEventListener",name,_loc1_.slider);
            _loc1_.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.onRendererSliderChange);
         }
      }
      
      override protected function draw() : void
      {
         trace("IPD:draw",_newFrame);
         super.draw();
      }
      
      private function onRendererSliderChange(param1:Event) : void
      {
         var _loc2_:MovieClip = itemRenderer as MovieClip;
         trace("PDS::onSlidreChange",_loc2_.slider);
         (_data as ItemData).value = int(_loc2_.slider.value / _loc2_.slider.snapInterval);
         dispatchEvent(new ButtonEvent(ButtonEvent.CLICK));
         dispatchEvent(new Event(Event.COMPLETE,true));
      }
   }
}

