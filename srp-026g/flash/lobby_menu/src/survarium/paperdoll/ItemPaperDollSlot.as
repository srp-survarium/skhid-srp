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
      
      override public function set highlighted(value:Boolean) : void
      {
         trace("IPD::set_highlighted",name,value,_state);
         _highlighted = value;
         if(value)
         {
            setState("over");
         }
         else
         {
            setState("up");
         }
      }
      
      override public function dropSlot(forced:Boolean = true) : void
      {
         if(!data)
         {
            return;
         }
         var playerProfile:ILocalPlayerProfile = this.parent.parent as ILocalPlayerProfile;
         var itemId:uint;
         var profileId:uint;
         var dictId:uint;
         var targetDescriptor:uint;
         var isDropped:Boolean;
         var relocation:Object;
         if(playerProfile)
         {
            itemId = (_data.itemId);
            profileId = (playerProfile.profileId);
            dictId = (_data.dictId);
            targetDescriptor = 100;
            isDropped = true;
            relocation = {
               profile:profileId,
               id:itemId,
               dict_id:dictId,
               sourceSlot:index,
               targetSlot:targetDescriptor,
               count:_data.condition_or_stack as uint
            };
            _relocateArray = [relocation];
            playerProfile._relocateFunction(_relocateArray);
         }
         super.dropSlot(forced);
      }
      
      override public function handleDragStartEvent(event:DragEvent) : void
      {
         var item:ItemData = _data as ItemData;
         trace("IPD:handleDragStartEvent::",event);
         if(owner is ILocalPaperDoll && _data)
         {
            (owner as ILocalPaperDoll).selectAgeements(data.category,data.dictId,data.oneHanded,true,data.itemId);
         }
         setState("over");
         this.data = null;
      }
      
      override public function handleDragEndEvent(event:DragEvent, wasValidDrop:Boolean) : void
      {
         if(owner is ILocalPaperDoll)
         {
            (owner as ILocalPaperDoll).deselectAgeements();
         }
         this.data = event.dragData;
         trace("IPD:handleDragEndEvent::",wasValidDrop,event.dropTarget);
         if(event.dropTarget == null || !wasValidDrop && event.dropTarget != null && !(event.dropTarget is IPaperDollSlot))
         {
            this.dropSlot(false);
         }
      }
      
      override public function handleDropEvent(event:DragEvent) : Boolean
      {
         trace("PDS:handleDropEvent::",name,enabled,highlighted,event.dropTarget);
         if(enabled == false || highlighted == false || !event.dropTarget)
         {
            return false;
         }
         var playerProfile:ILocalPlayerProfile = this.parent.parent as ILocalPlayerProfile;
         var item:ItemData = event.dragData as ItemData;
         if(!playerProfile)
         {
            dispatchEvent(new Event(Events.DROP_IN));
            _realData = _data as ItemData;
            this.data = (item as ItemData).clone();
            dispatchEvent(new Event(Events.DROPED_IN));
            return true;
         }
         _relocateArray.length = 0;
         var itemId:uint = item.itemId;
         var profileId:uint = (playerProfile.profileId);
         var dictId:uint = item.dictId;
         var sourceDescriptor:uint = event.dragTarget.descriptor;
         var targetDescriptor:uint;
         var result:*;
         var buyCount:int;
         var relocation:Object;
         var move:Object;
         if(event.dropTarget)
         {
            targetDescriptor = event.dropTarget.descriptor;
         }
         else
         {
            targetDescriptor = 100;
         }
         trace("PDS:handleDropEvent::",event.dragTarget,event.dropTarget,enabled,highlighted,playerProfile._relocateFunction,"source:",sourceDescriptor);
         if(item.descriptor == 200 && enabled == true && highlighted == true)
         {
            buyCount = (item.count);
            if(item.isStack)
            {
               buyCount = -1;
            }
            if(index == 7 || index == 10)
            {
               relocation = {
                  profile:profileId,
                  id:itemId,
                  dict_id:dictId,
                  sourceSlot:sourceDescriptor,
                  targetSlot:targetDescriptor,
                  count:1
               };
               trace_relocate(_relocateArray);
            }
            trace("descriptors::",event.dragTarget.descriptor,event.dropTarget.descriptor,item.descriptor);
            MainScene.buyFunction(playerProfile.profileId,item.itemId,item.dictId,item.descriptor,event.dropTarget.descriptor,buyCount,item.cost,item.factionId);
            dispatchEvent(new Event(Events.DROP_IN));
            return false;
         }
         if(!item.isStack)
         {
            move = {
               profile:profileId,
               id:itemId,
               dict_id:dictId,
               sourceSlot:sourceDescriptor,
               targetSlot:targetDescriptor,
               count:1
            };
            trace("relocateFabrica:a");
            _relocateArray = [move];
            if(index != 7 && index != 10)
            {
               trace_relocate(_relocateArray);
               result = playerProfile._relocateFunction(_relocateArray);
            }
            else if(targetDescriptor != 100)
            {
               trace_relocate(_relocateArray);
               result = playerProfile._relocateFunction(_relocateArray);
            }
            else
            {
               trace_relocate(_relocateArray);
               result = playerProfile._relocateFunction(_relocateArray);
            }
         }
         else
         {
            trace("relocateFabrica:isStack");
            if(enabled == true && highlighted == true)
            {
               if(event.dropTarget && !(event.dragTarget is IPaperDollSlot))
               {
                  trace("relocateFabrica:other");
                  _relocateArray = [{
                     profile:profileId,
                     id:itemId,
                     dict_id:dictId,
                     sourceSlot:sourceDescriptor,
                     targetSlot:targetDescriptor,
                     count:1
                  }];
                  trace_relocate(_relocateArray);
                  result = playerProfile._relocateFunction(_relocateArray);
               }
               else
               {
                  trace("relocateFabrica:swapQuickSlots");
                  result = playerProfile.swapQuickSlots(sourceDescriptor,targetDescriptor);
               }
            }
         }
         trace("IPD:::DROP_IN");
         dispatchEvent(new Event(Events.DROP_IN));
         _realData = _data as ItemData;
         _isTemponary = true;
         this.data = (item as ItemData).clone();
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
         var renderer:MovieClip = itemRenderer as MovieClip;
         if(renderer && renderer.slider)
         {
            renderer.slider.removeEventListener(SliderEvent.VALUE_CHANGE,this.onRendererSliderChange);
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
         if(renderer && renderer.slider)
         {
            trace("itemRenderer.addEventListener",name,renderer.slider);
            renderer.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.onRendererSliderChange);
         }
      }
      
      override protected function draw() : void
      {
         trace("IPD:draw",_newFrame);
         super.draw();
      }
      
      private function onRendererSliderChange(event:Event) : void
      {
         var renderer:MovieClip = itemRenderer as MovieClip;
         trace("PDS::onSlidreChange",renderer.slider);
         (_data as ItemData).value = int(renderer.slider.value / renderer.slider.snapInterval);
         dispatchEvent(new ButtonEvent(ButtonEvent.CLICK));
         dispatchEvent(new Event(Event.COMPLETE,true));
      }
   }
}

