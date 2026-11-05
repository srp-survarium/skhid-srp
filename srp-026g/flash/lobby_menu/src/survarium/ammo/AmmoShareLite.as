package survarium.ammo
{
   import flash.display.DisplayObject;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.SliderEvent;
   import survarium.data.Relocation;
   import survarium.controls.PlayerInfo;
   import survarium.controls.ScrollContent;
   import survarium.data.ItemData;
   import survarium.interfaces.IAmmoShare;
   import survarium.constants.AmmoView;
   import survarium.constants.ItemPlace;
   import survarium.constants.ProfileSlot;
   
   public class AmmoShareLite extends UIComponent implements IAmmoShare
   {
      
      public static const GROUP_AMMO:uint = 1;
      
      public static const GROUP_ITEMS:uint = 2;
      
      public static const PAIR_AMMO1:uint = 1;
      
      public static const PAIR_AMMO2:uint = 2;
      
      public static const PAIR_ITEMS:uint = 3;
      
      protected static var _id:AmmoShareLite;
      
      protected static var _itemCard:*;
      
      public var tx_weigth:TextField;
      
      public var activeIndex:int = 0;
      
      public var aviableWeight:Number = 0;
      
      protected var _autoBuyCheckBox:Button;
      
      protected var _updateFunction:Function;
      
      protected var _softUpdate:Boolean = false;
      
      protected var _slotsN:int = 0;
      
      protected var _ammo_indexes:Array = [ProfileSlot.WEAPON_1_AMMO_1,ProfileSlot.WEAPON_1_AMMO_2,ProfileSlot.WEAPON_2_AMMO_1,ProfileSlot.WEAPON_2_AMMO_2,ProfileSlot.AMMO_5,ProfileSlot.AMMO_6,ProfileSlot.AMMO_7,ProfileSlot.AMMO_8];
      
      protected var _ammoList:Array;
      
      protected var _profileID:uint;
      
      protected var _data:Array;
      
      protected var _dataSlots:Array;
      
      protected var _isShared:Boolean = false;
      
      protected var _relocateArray:Array;
      
      protected var _isReflow:Boolean = false;
      
      protected var _tempAmmoLst:Array = new Array();
      
      protected var _elemetns:Array = new Array();
      
      protected var _elemetnsSlots:Array = new Array();
      
      protected var _ammoBags:uint = 0;
      
      protected var _pocketSize:uint = 0;
      
      protected var _targetElement:AmmoShareElement;
      
      protected var _items_container:ScrollContent = new ScrollContent();
      
      protected var _btn_yes:Button;
      
      protected var _btn_cancel:Button;
      
      protected var _totalPrice:uint = 0;
      
      protected var _totalPricePremium:uint = 0;
      
      protected var _unsetted_slot:int = 0;
      
      protected var _emptyElements:Array = new Array();
      
      protected var _renderer:Class;
      
      public function AmmoShareLite()
      {
         super();
         _id = this;
         addEventListener(Event.ENTER_FRAME,this.config);
         name = "AmmoShareLite instance";
      }
      
      public static function getInstance() : AmmoShareLite
      {
         if(!_id)
         {
            new AmmoShareLite();
         }
         return _id;
      }
      
      public static function setItemCard(itemCard:*) : void
      {
         _itemCard = itemCard;
         _itemCard.addEventListener(Event.COMPLETE,getInstance().acceptItemCard);
      }
      
      public function set updateFunction(value:Function) : void
      {
         _updateFunction = value;
      }
      
      public function registerWeightTextField(weightField:TextField) : void
      {
         tx_weigth = weightField;
      }
      
      public function registerRenderer(rendererClass:Class) : void
      {
         _renderer = rendererClass;
         var i:uint = 0;
         var element:AmmoShareElement;
         while(i < AmmoView.SLOTS)
         {
            element = new this._renderer();
            this._emptyElements.push(element);
            i++;
         }
      }
      
      protected function getEmptyElement() : AmmoShareElement
      {
         var i:uint = 0;
         var element:AmmoShareElement;
         while(i < this._emptyElements.length)
         {
            element = this._emptyElements[i];
            if(!element.parent)
            {
               return element;
            }
            i++;
         }
         return null;
      }
      
      public function set autoBuyCheckBox(value:Button) : void
      {
         _autoBuyCheckBox = value;
      }
      
      public function get autoBuyCheckBox() : Button
      {
         return this._autoBuyCheckBox;
      }
      
      public function set acceptButton(value:Button) : void
      {
         _btn_yes = value;
         this._btn_yes.enabled = false;
         this._btn_yes.addEventListener(ButtonEvent.CLICK,this.setAmmo,false,0,true);
      }
      
      public function set cancelButton(value:Button) : void
      {
         _btn_cancel = value;
         this._btn_cancel.enabled = false;
         this._btn_cancel.addEventListener(ButtonEvent.CLICK,this.cancel,false,0,true);
      }
      
      public function set ammoBags(value:uint) : void
      {
         _ammoBags = value;
      }
      
      public function set pocketSize(value:uint) : void
      {
         _pocketSize = value;
      }
      
      public function getItemsList() : *
      {
         return null;
      }
      
      public function getAmmoRenderer(elementIndex:uint) : DisplayObject
      {
         trace("getAmmoRenderer",elementIndex);
         if(this._elemetns)
         {
            trace("_elemetns",this._elemetns[elementIndex]);
            if(this._elemetns[elementIndex])
            {
               this._elemetns[elementIndex].arrayIndex = elementIndex;
            }
            return this._elemetns[elementIndex];
         }
         trace("return _null");
         return null;
      }
      
      public function highlightAmmoRendererSlider(elementIndex:uint) : DisplayObject
      {
         trace("getAmmoRendererSlider",elementIndex);
         if(this._elemetns)
         {
            trace("_elemetns",this._elemetns[elementIndex]);
            if(this._elemetns[elementIndex])
            {
               this._elemetns[elementIndex].arrayIndex = elementIndex;
            }
            return this._elemetns[elementIndex].slider;
         }
         trace("return _null");
         return null;
      }
      
      public function highlightAmmoRendererCell(elementIndex:uint, hover:Boolean) : void
      {
         trace("highlightAmmoRendererCell",elementIndex);
         if(this._elemetns)
         {
            trace("_elemetns",this._elemetns[elementIndex]);
            if(this._elemetns[elementIndex])
            {
               this._elemetns[elementIndex].slider.hoverCells(hover);
            }
         }
      }
      
      public function swapElements(fromSlot:int, toSlot:int) : void
      {
         var fromElement:Object;
         var toElement:Object;
         var count:uint = this._elemetnsSlots.length;
         var i:int = 0;
         var slotElement:Object;
         while(i < count)
         {
            slotElement = this._elemetnsSlots[i];
            if(slotElement.index == fromSlot)
            {
               fromElement = slotElement;
            }
            if(slotElement.index == toSlot)
            {
               toElement = slotElement;
            }
            if(fromElement && toElement)
            {
               break;
            }
            i++;
         }
         trace("swapElements",fromElement,toElement,"|",fromSlot,toSlot);
         if(fromElement && toElement)
         {
            trace("swapElements",fromElement.data ? fromElement.data.name : "null",toElement.data ? toElement.data.name : "null");
            fromElement.index = toSlot;
            if(fromElement.data)
            {
               fromElement.data.slotId = toSlot;
            }
            toElement.index = fromSlot;
            if(toElement.data)
            {
               toElement.data.slotId = fromSlot;
            }
         }
         this.setAmmo();
      }
      
      public function updateElement(move:Object, newItem:ItemData = null) : void
      {
         trace("updateElement::",this._ammoList.length,this._elemetns.length,this._elemetnsSlots.length,this._dataSlots.length,newItem ? newItem.count : "no_new_data");
         var item:ItemData = null;
         var i:int = 0;
         while(i < this._dataSlots.length)
         {
            trace("searchForVO::",move.dict_id,this._dataSlots[i].dictId,this._dataSlots[i].name);
            if(this._dataSlots[i].dictId == move.dict_id)
            {
               // the target pocket's own item before another pocket's of its type: taking the first
               // pocket's for a slider moved in a second pocket of the type (a kit saved before the
               // rule of one pocket a type) emptied the second pocket
               if(!item || this._dataSlots[i].slotId == move.targetSlot)
               {
                  item = this._dataSlots[i];
               }
               if(item.slotId == move.targetSlot)
               {
                  break;
               }
            }
            i++;
         }
         if(!item)
         {
            if(newItem)
            {
               item = newItem;
            }
            else
            {
               return;
            }
         }
         else if(newItem && ProfileSlot.isPocket(move.targetSlot))
         {
            // a buy onto the pocket of its type: newItem holds the stack and the pocket after it
            // (LobbyMenu.relocate_to_paperdoll)
            item.count = newItem.count;
            item.value = newItem.value;
         }
         else if(newItem)
         {
            item.count += newItem.value;
         }
         var j:int = 0;
         var slotElement:Object;
         while(j < this._elemetnsSlots.length)
         {
            slotElement = this._elemetnsSlots[j];
            if(slotElement.index == move.targetSlot)
            {
               trace("update::",item.slotId,item.count);
               item.descriptor = move.sourceSlot;
               slotElement.data = item;
               this.setAmmo();
               break;
            }
            j++;
         }
      }
      
      protected function acceptItemCard(event:Event = null) : void
      {
         trace("acceptItemCardTotal::",_itemCard,this._elemetns);
         _relocateArray = new Array();
         trace("ASL::apply_profile_items");
         ExternalInterface.call("apply_profile_items",this._relocateArray);
      }
      
      public function closePopup() : void
      {
         this.prepare_new_data();
      }
      
      public function setProfileId(profileId:int) : void
      {
         _profileID = profileId;
      }
      
      public function setSlots(slotCount:int = 0, unused:uint = 0) : void
      {
         _slotsN = slotCount;
      }
      
      public function setShared(shared:Boolean) : void
      {
         _isShared = shared;
      }
      
      public function getShared() : Boolean
      {
         return this._isShared;
      }
      
      public function reflow(source:AmmoShareElement) : void
      {
         trace("ammmo_reflow_st",source,source ? source.index : "no_tg",this._isReflow);
         if(this._isReflow)
         {
            return;
         }
         _isReflow = true;
         var otherCount:Number = 0;
         var sum:Number = 0;
         var allEmpty:Boolean = true;
         var element:AmmoShareElement;
         var other:AmmoShareElement;
         var count:uint = this._elemetns.length;
         var i:int = 0;
         var j:uint;
         while(i < count)
         {
            element = this._elemetns[i];
            if(element.group != GROUP_ITEMS && (!source || element.pair == source.pair && element.index != source.index))
            {
               otherCount = 0;
               j = 0;
               while(j < count)
               {
                  other = this._elemetns[j];
                  if(other.index != element.index && element.group == other.group)
                  {
                     otherCount += other.getCount();
                  }
                  j++;
               }
               element.setCount(otherCount,true);
               if(allEmpty && element.getCount() != 0)
               {
                  allEmpty = false;
               }
            }
            i++;
         }
         _totalPrice = 0;
         _totalPricePremium = 0;
         for(i = 0; i < count; i++)
         {
            element = this._elemetns[i];
            if(element.isPremium())
            {
               _totalPricePremium += element.get_required_price();
            }
            else
            {
               _totalPrice += element.get_required_price();
            }
            if(element.group == GROUP_ITEMS)
            {
               continue;
            }
            if(source && element.pair != source.pair)
            {
               otherCount = 0;
               j = 0;
               while(j < count)
               {
                  other = this._elemetns[j];
                  if(other.index != element.index && element.group == other.group)
                  {
                     otherCount += other.getCount();
                  }
                  j++;
               }
               if(allEmpty)
               {
                  element.setCount(otherCount,true);
               }
               else
               {
                  element.setCount(otherCount,false);
               }
            }
         }
         if(this._updateFunction is Function)
         {
            this._updateFunction(this._totalPrice,this._totalPricePremium);
         }
         j = 0;
         while(j < count)
         {
            element = this._elemetns[j];
            element.updateStorage();
            j++;
         }
         _isReflow = false;
         trace("reflow_end");
      }
      
      public function getSlotData(slot:uint) : ItemData
      {
         var i:int;
         trace("getSlotData::",this._ammoList);
         if(this._ammoList)
         {
            i = 0;
            while(i < this._ammoList.length)
            {
               if(this._ammoList[i] && this._ammoList[i].slot == slot)
               {
                  return this._ammoList[i];
               }
               i++;
            }
         }
         return null;
      }
      
      public function getAmmoList(weapon:int, excludeUsed:Boolean = false, exceptSlot:int = -1) : Array
      {
         trace("getAmmoList",weapon,this._ammoList,name,exceptSlot,excludeUsed);
         this._tempAmmoLst.length = 0;
         var i:int = 0;
         var skip:Boolean;
         var j:int;
         var element:AmmoShareElement;
         var elementData:ItemData;
         while(i < this._ammoList.length)
         {
            skip = false;
            if(this._ammoList[i].weaponTarget != weapon)
            {
               skip = true;
            }
            else
            {
               this._ammoList[i].enabled = true;
               if(exceptSlot == -1 || this._ammoList[i].slotId != exceptSlot)
               {
                  j = 0;
                  while(j < this._elemetns.length)
                  {
                     element = this._elemetns[j];
                     elementData = element.getData();
                     if(elementData)
                     {
                        if(this._ammoList[i].dictId == elementData.dictId)
                        {
                           if(excludeUsed)
                           {
                              skip = true;
                              break;
                           }
                           else
                           {
                              this._ammoList[i].enabled = false;
                           }
                        }
                     }
                     j++;
                  }
               }
            }
            if(!skip)
            {
               this._tempAmmoLst.push(this._ammoList[i]);
            }
            i++;
         }
         trace("tempAmmoList:",this._tempAmmoLst);
         return this._tempAmmoLst;
      }
      
      public function setAmmo(event:ButtonEvent = null) : void
      {
         _relocateArray = new Array();
         var i:int = 0;
         var element:AmmoShareElement;
         while(i < this._elemetns.length)
         {
            element = this._elemetns[i];
            element.updated = false;
            this.relocateAmmo(element);
            i++;
         }
         i = 0;
         while(i < this._elemetnsSlots.length)
         {
            this.relocateItem(this._elemetnsSlots[i].data);
            i++;
         }
         trace("ASL::apply_profile_items");
         ExternalInterface.call("apply_profile_items",this._relocateArray);
         this.clear();
         if(this._updateFunction is Function)
         {
            this._updateFunction(0,0);
         }
         this._btn_yes.enabled = this._btn_cancel.enabled = false;
      }
      
      public function reflowElements() : void
      {
         activeIndex = AmmoView.ALL;
         var stepY:int = 144 + 5;
         var stepX:int = 275 + 5;
         var shiftY:int = 0;
         var shiftX:int = 0;
         var element:AmmoShareElement;
         trace("reflowElemnts",this.activeIndex);
         for(var i:uint = 0; i < this._elemetns.length; i++)
         {
            element = this._elemetns[i];
            if(!element)
            {
               continue;
            }
            if(element.pair == PAIR_AMMO1 || element.pair == PAIR_AMMO2)
            {
               if(this.activeIndex == AmmoView.PRIMARY && element.pair == PAIR_AMMO1 || this.activeIndex == AmmoView.SECONDARY && element.pair == PAIR_AMMO2 || this.activeIndex == AmmoView.ALL)
               {
                  if(!element.parent)
                  {
                     element.y = shiftY;
                     this._items_container.addContent(element);
                  }
               }
               else if(element.parent)
               {
                  this._items_container.removeContent(element);
               }
            }
            else if(this.activeIndex == -1)
            {
               element.y = shiftY;
               this._items_container.addContent(element);
            }
            else
            {
               this._items_container.removeContent(element);
            }
            if(!element.parent)
            {
               continue;
            }
            element.y = shiftY;
            element.x = shiftX;
            shiftY += stepY;
            if(shiftY / stepY == 3)
            {
               shiftY = 0;
               shiftX += stepX;
            }
         }
      }
      
      public function setData(ammoList:Array, slotItems:Array, unused:Boolean = false, softUpdate:Boolean = false) : void
      {
         _data = ammoList;
         _dataSlots = slotItems;
         _softUpdate = softUpdate;
         _ammoList = this._data;
         trace("invalidateASLData",this._data,this._softUpdate);
         invalidateData();
      }
      
      internal function checkApplyButton() : void
      {
         trace("checkApplyButton",this._totalPrice);
         if(this._totalPrice > PlayerInfo.genericMoney)
         {
            this._btn_yes.enabled = false;
            this._btn_cancel.enabled = false;
         }
         else
         {
            this._btn_yes.enabled = true;
            this._btn_cancel.enabled = true;
         }
      }
      
      internal function getCount(exclude:AmmoShareElement) : Number
      {
         var count:Number = 0;
         var i:int = 0;
         var element:AmmoShareElement;
         for(; i < this._elemetns.length; i++)
         {
            element = this._elemetns[i];
            if(!element)
            {
               continue;
            }
            if(element.group == GROUP_ITEMS)
            {
               continue;
            }
            if(element.index != exclude.index)
            {
               count += element.getCount();
            }
         }
         return count;
      }
      
      internal function getCost(exclude:AmmoShareElement) : Number
      {
         var cost:Number = 0;
         var i:int = 0;
         var element:AmmoShareElement;
         while(i < this._elemetns.length)
         {
            element = this._elemetns[i];
            if(element.index != exclude.index)
            {
               cost += element.isPremium() ? 0 : element.get_required_price();
            }
            i++;
         }
         return cost;
      }
      
      protected function config(event:Event) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.config);
         addEventListener(Event.CHANGE,this.onChange);
         addEventListener(SliderEvent.VALUE_CHANGE,this.onChange);
         this._items_container.name = "items_container";
         this._items_container.elemntShift = 15;
         addChild(this._items_container);
         this._items_container.width = 575;
         this._items_container.height = 500;
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            if(this._data)
            {
               this._setData(this._data);
            }
         }
      }
      
      protected function updateAutoBuy(event:Event) : void
      {
         trace("ASl::updateAutoBuy",this._autoBuyCheckBox.selected);
         ExternalInterface.call("ammo_autobuy_changed",this._autoBuyCheckBox.selected);
      }
      
      protected function onChange(event:Event) : void
      {
         var totalWeight:Number = 0;
         var count:int = (this._elemetns.length);
         var i:int = 0;
         var element:AmmoShareElement;
         var weight:Number;
         var slotItem:ItemData;
         while(i < count)
         {
            element = this._elemetns[i];
            weight = 0;
            if((element.getData()))
            {
               weight = element.getWeight();
               totalWeight += weight;
            }
            i++;
         }
         count = (this._elemetnsSlots.length);
         i = 0;
         while(i < this._elemetnsSlots.length)
         {
            slotItem = this._elemetnsSlots[i].data;
            if(slotItem)
            {
               weight = slotItem.value * slotItem.weight;
               totalWeight += weight;
            }
            i++;
         }
         trace("updateWeightOnChange::",this.tx_weigth,PlayerInfo.curWeight,this.aviableWeight,totalWeight,PlayerInfo.maxWeight,"calculated",Number(PlayerInfo.maxWeight - this.aviableWeight + totalWeight).toPrecision(2));
         if(this.tx_weigth)
         {
         }
      }
      
      protected function onControlChanges(event:Event) : void
      {
         this.checkApplyButton();
      }
      
      protected function clear() : void
      {
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      protected function trace_relocate(relocations:Array) : void
      {
         trace("trace_relocate",relocations.length);
         var i:int = 0;
         while(i < relocations.length)
         {
            trace("items is:",relocations[i].profile,relocations[i].id,relocations[i].dict_id,"sr:",relocations[i].sourceSlot,"tg:",relocations[i].targetSlot,relocations[i].count);
            i++;
         }
      }
      
      protected function prepare_new_data() : void
      {
         var i:int = 0;
         var element:AmmoShareElement;
         while(i < this._elemetns.length)
         {
            element = this._elemetns[i];
            element.disable();
            i++;
         }
         this.clear();
      }
      
      protected function cancel(event:ButtonEvent = null) : void
      {
         this.prepare_new_data();
         this._setData(this._ammoList);
         this._btn_cancel.enabled = this._btn_yes.enabled = false;
      }
      
      protected function clearElements() : void
      {
         var i:int = 0;
         var element:AmmoShareElement;
         while(i < this._elemetns.length)
         {
            element = this._elemetns[i];
            element.disable();
            this._items_container.removeContent(element);
            i++;
         }
         this._elemetns.length = 0;
      }
      
      protected function relocateItem(item:ItemData) : void
      {
         var slotItem:ItemData = item;
         var itemId:uint;
         var profileId:uint;
         var dictId:uint;
         var targetSlotId:uint;
         var sourceSlotId:uint;
         var inventoryCount:uint;
         var moveCount:uint;
         var move:Object;
         if(slotItem)
         {
            itemId = slotItem.itemId == -1 ? 0 : slotItem.itemId;
            profileId = this._profileID;
            dictId = slotItem.dictId;
            targetSlotId = slotItem.slotId;
            sourceSlotId = slotItem.descriptor != ItemPlace.NONE ? slotItem.descriptor : ItemPlace.INVENTORY;
            inventoryCount = slotItem.count;
            moveCount = slotItem.value;
            trace("prepare relocate item:",item.name,"target:",targetSlotId,"count:",moveCount,inventoryCount,"sr:",sourceSlotId,"name",slotItem.name,itemId,slotItem.itemId,"ds:",slotItem.descriptor);
            if(inventoryCount)
            {
               move = Relocation.move(profileId,itemId,dictId,sourceSlotId,targetSlotId,moveCount);
               move.count_in_inventory = inventoryCount;
               move.auto_buy = this.autoBuyCheckBox.selected;
               this._relocateArray.push(move);
            }
         }
      }
      
      protected function relocateAmmo(element:AmmoShareElement) : void
      {
         var ammo:Object = element.getData();
         var itemId:uint;
         var profileId:uint;
         var dictId:uint;
         var targetSlotId:uint;
         var sourceSlotId:uint;
         var moveCount:uint;
         var move:Object;
         var inventoryCount:uint;
         if(ammo)
         {
            itemId = ammo.itemId == -1 ? 0 : (ammo.itemId);
            profileId = this._profileID;
            dictId = (ammo.dictId);
            targetSlotId = element.index;
            sourceSlotId = ammo.source_slot != undefined ? (ammo.source_slot) : ItemPlace.INVENTORY;
            moveCount = element.slider.position;
            inventoryCount = (element.slider.value_storage);
            trace("prepare relocate:",element.name,"target:",targetSlotId,"source:",sourceSlotId,"count:",moveCount,inventoryCount,"name",ammo.name,element.pair,itemId,ammo.itemId,this.autoBuyCheckBox.selected);
            if(inventoryCount)
            {
               move = Relocation.move(profileId,itemId,dictId,sourceSlotId,targetSlotId,moveCount);
               move.count_in_inventory = inventoryCount;
               move.auto_buy = this.autoBuyCheckBox.selected;
               this._relocateArray.push(move);
            }
         }
      }
      
      protected function clearData(event:Event) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.clearData);
         this.prepare_new_data();
      }
      
      protected function updateData(event:Event) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.updateData);
         this._setData(this._data);
      }
      
      protected function _setData(value:Array) : void
      {
         trace("AmmoShareSetData:",this.activeIndex,this._softUpdate,"|",value,"|",this._elemetns,"|",this._items_container);
         _ammoList = value;
         if(value.length)
         {
            value.sortOn(["weapon_target","slot"],Array.NUMERIC);
         }
         var _ammoSlots:uint = AmmoView.SLOTS;
         var _shift:uint = 0;
         var _element:AmmoShareElement;
         var current_index:uint = 0;
         var st_index:uint = 0;
         var _slotToIterate:uint = _ammoSlots;
         var ln:int;
         var ln2:int;
         var ammoL:int;
         var slotsL:int;
         var itemsL:int;
         trace("updateA");
         ln = (this._elemetns.length);
         i = 0;
         while(i < ln)
         {
            _element = this._elemetns[i];
            _element.index = this._unsetted_slot;
            _element.slider.removeEventListener(SliderEvent.VALUE_CHANGE,this.onChange);
            _element.removeEventListener(Event.CHANGE,this.onControlChanges);
            if(_element.parent)
            {
               this._items_container.removeContent(_element);
            }
            i++;
         }
         trace("updateB");
         var _start_elements:Array = this._elemetns.concat();
         this._elemetns.length = 0;
         var ammo1_slots:int = 0;
         var ammo2_slots:int = 0;
         var i:int;
         ln = (value.length);
         i = 0;
         while(i < ln)
         {
            itemData = value[i] as ItemData;
            if(itemData.weaponTarget != PAIR_ITEMS)
            {
               _element = null;
               ln2 = (_start_elements.length);
               for(k = 0; k < ln2; k++)
               {
                  if(!_start_elements[k])
                  {
                     continue;
                  }
                  elData = _start_elements[k].getData();
                  if(elData && elData.dictId == itemData.dictId)
                  {
                     _element = _start_elements[k];
                     trace("use_old_element",_element.index,_element.updated);
                     _start_elements.splice(k,1);
                  }
               }
               if(!_element)
               {
                  trace("create_new_element",itemData.slotId);
                  _element = new this._renderer();
                  _element.owner = this;
               }
               _element.index = itemData.slotId;
               this._elemetns[current_index] = _element;
               current_index += 1;
               _element.ignoreChange();
               _element.slider.renderer = "ItemRoundCell";
               _element.available = this._ammoBags;
               _element.group = GROUP_AMMO;
               st_index += 1;
               _element.pair = itemData.weaponTarget;
               if(_element.pair == PAIR_AMMO1)
               {
                  ammo1_slots++;
               }
               else
               {
                  ammo2_slots++;
               }
               if(this.activeIndex == AmmoView.PRIMARY || this.activeIndex == AmmoView.SECONDARY || this.activeIndex == AmmoView.ALL)
               {
                  if(!_element.parent)
                  {
                     this._items_container.addContent(_element);
                  }
               }
               else if(_element.parent)
               {
                  this._items_container.removeContent(_element);
               }
            }
            i++;
         }
         ln = (this._elemetns.length);
         i = 0;
         while(i < ln)
         {
            _element = this._elemetns[i];
            if(_element.pair != PAIR_ITEMS && _element.index == this._unsetted_slot)
            {
               function getIndexOwned(slotIndex:int):Boolean
               {
                  var slotPos:int = 0;
                  var entry:*;
                  while(slotPos < ln)
                  {
                     entry = _elemetns[slotPos];
                     if(entry.pair != PAIR_ITEMS && slotIndex == entry.index)
                     {
                        return true;
                     }
                     slotPos++;
                  }
                  return false;
               }
               var _elementSlot:Object;
               var pos_of_last_ammo:uint;
               var j:int;
               var list:Array;
               index_iterator = 0;
               new_index = (this._ammo_indexes[0]);
               while(getIndexOwned(new_index))
               {
                  index_iterator++;
                  new_index = (this._ammo_indexes[index_iterator]);
               }
               _element.index = new_index;
            }
            i++;
         }
         ln = this._elemetns.length - 1;
         trace("removeRestedAmmoSlots");
         i = ln;
         while(i > st_index)
         {
            _element = this._elemetns[i];
            if(_element.pair != PAIR_ITEMS)
            {
               if(_element.parent)
               {
                  this._items_container.removeContent(_element);
                  _element.clear();
               }
               this._elemetns.splice(i,1);
            }
            i--;
         }
         ln = (_start_elements.length);
         i = 0;
         while(i < ln)
         {
            _element = _start_elements[i];
            if(_element && _element.pair != PAIR_ITEMS)
            {
               if(_element.parent)
               {
                  this._items_container.removeContent(_element);
               }
               _start_elements.splice(i,1);
               i--;
            }
            i++;
         }
         trace("addIgnorentAmmoSlots",_slotToIterate);
         i = 0;
         while(i < _slotToIterate)
         {
            if(!this._elemetns[i])
            {
               _element = this.getEmptyElement();
               _element.group = GROUP_AMMO;
               _element.index = ProfileSlot.NONE;
               if(ammo1_slots < AmmoView.SLOTS_PER_WEAPON)
               {
                  _element.pair = PAIR_AMMO1;
                  ammo1_slots++;
               }
               else
               {
                  _element.pair = PAIR_AMMO2;
                  ammo2_slots++;
               }
               if(!_element.parent)
               {
                  this._items_container.addContent(_element);
               }
               this._elemetns[current_index] = _element;
               current_index += 1;
               st_index += 1;
            }
            i++;
         }
         i = 0;
         while(i < this._slotsN)
         {
            _elementSlot = null;
            ln = (this._elemetnsSlots.length);
            k = 0;
            while(k < ln)
            {
               if(this._elemetnsSlots[k].index == i + ProfileSlot.QUICK_1)
               {
                  _elementSlot = this._elemetnsSlots[k];
                  break;
               }
               k++;
            }
            if(!_elementSlot)
            {
               _elementSlot = new Object();
               this._elemetnsSlots.push(_elementSlot);
            }
            _elementSlot.index = i + ProfileSlot.QUICK_1;
            i++;
         }
         this._elemetnsSlots.length = this._slotsN;
         ln = (this._elemetns.length);
         st_index += i;
         i = ln;
         while(i > st_index)
         {
            _element = this._elemetns.pop();
            _element.removeEventListener(Event.CHANGE,this.onControlChanges);
            _element.slider.removeEventListener(SliderEvent.VALUE_CHANGE,this.onChange);
            if(_element.parent)
            {
               this._items_container.removeContent(_element);
            }
            i--;
         }
         _start_elements.length = 0;
         k = 0;
         while(i < ln)
         {
            if(_element.getData() == null)
            {
               (this._elemetns[k] as AmmoShareElement).setData(this._elemetns[k]);
            }
            else
            {
               _element.disable();
            }
            k++;
         }
         if(this._ammoBags == this._slotsN)
         {
            pos_of_last_ammo = (this._elemetns.indexOf(_element));
         }
         this._elemetns.sortOn(["pair","index"],Array.NUMERIC);
         ln2 = (value.length);
         i = 0;
         while(i < ln2)
         {
            itemData = value[i];
            j = 0;
            while(j < ln)
            {
               _element = this._elemetns[j];
               if(_element && (itemData.slotId != 0 && itemData.slotId == _element.index || (itemData.slotId == 0 && _element.getData() && itemData.dictId == _element.getData().dictId)))
               {
                  if((this._softUpdate || _element.updated) && _element.getData() && itemData.dictId == _element.getData().dictId)
                  {
                     _element.name = value[i].name;
                     _element.update(value[i],!this._softUpdate);
                     break;
                  }
                  else
                  {
                     _element.name = value[i].name;
                     _element.setData(value[i],true);
                     break;
                  }
               }
               j++;
            }
            i++;
         }
         i = 0;
         while(i < this._slotsN)
         {
            _elementSlot = this._elemetnsSlots[i];
            _elementSlot.data = null;
            i++;
         }
         itemsL = (this._dataSlots.length);
         trace("fill_dataSlots",itemsL,"|",this._slotsN);
         i = 0;
         while(i < itemsL)
         {
            itemData = this._dataSlots[i];
            trace("itemData",itemData.name,itemData.slotId);
            j = 0;
            while(j < this._slotsN)
            {
               _elementSlot = this._elemetnsSlots[j];
               trace("_elementSlot",_elementSlot.index,itemData.slotId);
               if(_elementSlot.index == itemData.slotId)
               {
                  _elementSlot.data = itemData;
                  break;
               }
               j++;
            }
            i++;
         }
         i = 0;
         while(i < ln)
         {
            _element = this._elemetns[i];
            if(!_element.getData() && _element.pair != PAIR_ITEMS && _element.index != ProfileSlot.NONE)
            {
               list = this.getAmmoList(_element.pair,true);
               if(list.length && list[0])
               {
                  _element.name = "[]" + list[0].name;
                  _element.setData(list[0],true);
               }
            }
            i++;
         }
         i = 0;
         while(i < ln)
         {
            _element = this._elemetns[i];
            if(!_element.getData())
            {
               trace("clean_element::",_element.name);
               _element.disable();
            }
            i++;
         }
         this.reflow(null);
         this.onChange(null);
         i = 0;
         while(i < ln)
         {
            _element = this._elemetns[i];
            _element.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.onChange,false,0,true);
            _element.addEventListener(Event.CHANGE,this.onControlChanges,false,0,true);
            i++;
         }
         this._elemetns.sortOn(["pair","itemLevel","index"],Array.NUMERIC);
         var step_y:int = 144 + 5;
         var step_x:int = 275 + 5;
         var shift_y:int = 0;
         var shift_x:int = 0;
         var itemData:ItemData;
         var k:int;
         var elData:ItemData;
         var index_iterator:int;
         var new_index:uint;
         for(i = 0; i < ln; i++)
         {
            _element = this._elemetns[i];
            if(!_element || !_element.parent)
            {
               continue;
            }
            _element.x = shift_x;
            _element.y = shift_y;
            shift_y += step_y;
            if(shift_y / step_y == 3)
            {
               shift_y = 0;
               shift_x += step_x;
            }
         }
         trace("ASL::updated");
      }
   }
}

