package survarium.player
{
   import flash.display.DisplayObject;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.DragSlot;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.interfaces.IDataProvider;
   import scaleform.clik.interfaces.IListItemRenderer;
   import scaleform.gfx.MouseEventEx;
   import survarium.constants.Events;
   import survarium.data.ItemData;
   import survarium.interfaces.IAmmoControll;
   import survarium.interfaces.IInventoryItemFactory;
   import survarium.interfaces.ILocalPaperDoll;
   import survarium.interfaces.IPaperDollSlot;
   import survarium.item.ItemFactory;
   import survarium.paperdoll.PaperDollBase;
   
   public class LocalPaperDoll extends PaperDollBase implements ILocalPaperDoll
   {
      
      protected static var _slotAgreements:Object = {
         slotHead:[1],
         slotMask:[2],
         slotBody:[3],
         slotBack:[7],
         slotLegs:[4],
         slotArms:[5],
         slotFoot:[6],
         slotWpn1:[8],
         slotWpn1A1:[9],
         slotWpn1A2:[9],
         slotWpn2:[8],
         slotWpn2A1:[9],
         slotWpn2A2:[9],
         slotQck1:[10],
         slotQck2:[10],
         slotQck3:[10],
         slotQck4:[10],
         slotQck5:[10],
         slotQck6:[10],
         slotWpn5:[9],
         slotWpn6:[9],
         slotWpn7:[9],
         slotWpn8:[9]
      };
      
      protected var _inventory:IInventoryItemFactory;
      
      protected var _shareMode:Boolean = false;
      
      protected var _ammo_indexes:Array = [8,9,11,12,19,20];
      
      protected var _ammo1_indexes:Array = [8,9,11];
      
      protected var _ammo2_indexes:Array = [12,19,20];
      
      protected var _last_slot:IPaperDollSlot;
      
      protected var _last_name:String = "slotWpn1";
      
      protected var _slotSearch:Array = ["slotWpn1","slotWpn2","slotHead","slotMask","slotBody","slotBack","slotLegs","slotArms","slotFoot"];
      
      protected var _slots_index:Array = ["slotWpn1","slotWpn2","slotQck1","slotQck2","slotQck3","slotQck4","slotQck5","slotQck6"];
      
      protected var _ammoControl:IAmmoControll;
      
      protected var _hoverIndex:uint = 0;
      
      protected var _secondaryEnabled:Boolean = true;
      
      protected var _doubleTwoHanded:Boolean = false;
      
      protected var _contextIndex:int = -1;
      
      private var _lockedSlot:uint = 0;
      
      public function LocalPaperDoll()
      {
         super();
      }
      
      public static function getSlotPair(param1:int, param2:int) : Boolean
      {
         if((_slotAgreements[_slotNames[param2]] as Array).indexOf(param1) != -1)
         {
            return true;
         }
         return false;
      }
      
      public function set ammoControl(param1:IAmmoControll) : void
      {
         _ammoControl = param1;
         addChild(this._ammoControl as DisplayObject);
      }
      
      public function get ammoControl() : IAmmoControll
      {
         return this._ammoControl;
      }
      
      override public function set dataProvider(param1:IDataProvider) : void
      {
         super.dataProvider = param1;
         this._ammoControl.updatePaperDollData(_dataProvider,_slots);
         this._ammoControl.update(null,"paperdoll");
      }
      
      public function get selectedIndex() : int
      {
         return this._slots_index.indexOf(this._last_name);
      }
      
      public function hoverItem(param1:uint = 0) : void
      {
         trace("PD::hoverItem",param1);
         if(!param1 && this._hoverIndex)
         {
            getSlotAt(this._hoverIndex).hover(false);
            _hoverIndex = 0;
            return;
         }
         var _loc2_:uint = _slots.length;
         var _loc3_:int = 0;
         var _loc4_:Object;
         while(_loc3_ < _loc2_)
         {
            _loc4_ = _slots[_loc3_];
            if(_loc4_ && _loc4_.data && _loc4_.data.itemId == param1)
            {
               _loc4_.hover();
               _hoverIndex = _loc3_;
               return;
            }
            _loc3_++;
         }
      }
      
      public function enableSecondWeapon(param1:Boolean) : void
      {
         trace("enableSecondWeapon::",param1);
         _secondaryEnabled = param1;
         invalidate(InvalidationType.DATA);
      }
      
      public function allowSecondaryTwoHandedWeapon(param1:Boolean) : void
      {
         trace("allowSecondaryTwoHandedWeapon::",param1);
         _doubleTwoHanded = param1;
         invalidate(InvalidationType.DATA);
      }
      
      public function getSlot(param1:ItemData, param2:Boolean = false) : int
      {
         stackTrace("getSlot::" + String(param1));
         if(!param1)
         {
            return -1;
         }
         var _loc3_:int = 0;
         var _loc4_:uint;
         var _loc5_:IPaperDollSlot;
         var _loc6_:Boolean;
         var _loc7_:int;
         while(_loc3_ < _slots.length)
         {
            _loc4_ = param1.category;
            _loc6_ = (_slotAgreements[_slots[_loc3_].name] as Array).indexOf(_loc4_) != -1;
            if(_loc6_)
            {
               if(_loc4_ == 10 || _loc4_ == 11 || _loc4_ == 24)
               {
                  _loc7_ = 13;
                  while(_loc7_ < _slots.length)
                  {
                     if(param2)
                     {
                        trace("j::",_loc7_,_slotsN + 12);
                        if(!_slots[_loc7_].data || _loc7_ == _slotsN + 12)
                        {
                           _loc5_ = _slots[_loc7_];
                           return _loc7_;
                        }
                     }
                     else
                     {
                        if(_slots[_loc7_].data != null)
                        {
                           _loc5_ = _slots[_loc7_];
                           return _loc7_;
                        }
                        _loc5_ = _slots[13];
                        return 13;
                     }
                     _loc7_++;
                  }
               }
               _loc5_ = _slots[_loc3_];
               if(_loc5_ && _loc5_.highlighted)
               {
                  return _loc3_;
               }
            }
            _loc3_++;
         }
         return -1;
      }
      
      override public function toString() : String
      {
         return "[LocalPaperDoll " + name + "]";
      }
      
      public function setFillArray(param1:Array) : void
      {
         this._ammoControl.fillItems(param1);
      }
      
      public function setSlotsRestrictions(param1:Array) : void
      {
         var _loc2_:String;
         var _loc3_:int;
         var _loc4_:Object;
         for(_loc2_ in _slotAgreements)
         {
            _slotAgreements[_loc2_] = new Array();
         }
         _loc3_ = 0;
         while(_loc3_ < param1.length)
         {
            _loc4_ = param1[_loc3_];
            _slotAgreements[_slotNames[_loc4_.slot_id]].push(_loc4_.category_id);
            _loc3_++;
         }
      }
      
      public function registerInventory(param1:IInventoryItemFactory) : void
      {
         _inventory = param1;
      }
      
      override public function openSlots(param1:int, param2:int) : void
      {
         this._ammoControl.openSlots(param1,param2);
         super.openSlots(param1,param2);
      }
      
      public function hasAgreements(param1:uint) : uint
      {
         if(!enabled)
         {
            return 0;
         }
         var _loc2_:uint = _slots.length;
         var _loc3_:uint = 0;
         var _loc4_:int = 0;
         var _loc5_:Boolean;
         while(_loc4_ < _loc2_)
         {
            _loc5_ = false;
            if(_slotAgreements[_slots[_loc4_].name] && ((_slotAgreements[_slots[_loc4_].name] as Array).indexOf(param1) != -1) && _slots[_loc4_].enabled)
            {
               if(!_slots[_loc4_].data)
               {
                  return 1;
               }
               _loc3_++;
            }
            _loc4_++;
         }
         return _loc3_;
      }
      
      public function selectAgeements(param1:int, param2:int, param3:Boolean = false, param4:Boolean = false, param5:int = 0) : void
      {
         trace("selectAgeements",param1,param2,param3,param5);
         if(!enabled)
         {
            return;
         }
         var _loc6_:Boolean = false;
         var _loc7_:uint = _slots.length;
         var _loc8_:int = 0;
         var _loc9_:Boolean;
         var _loc10_:IPaperDollSlot;
         var _loc11_:Boolean;
         while(_loc8_ < _loc7_)
         {
            _loc9_ = false;
            if(_slotAgreements[_slots[_loc8_].name] && ((_slotAgreements[_slots[_loc8_].name] as Array).indexOf(param1) != -1) && _slots[_loc8_].enabled)
            {
               if(param1 == 9 || param1 == 18 || param1 == 19 || param1 == 20 || param1 == 21)
               {
                  if(_loc8_ == 8 || _loc8_ == 9)
                  {
                     _loc10_ = _slots[7];
                  }
                  else if(_loc8_ == 11 || _loc8_ == 12)
                  {
                     _loc10_ = _slots[10];
                  }
                  if(this._doubleTwoHanded)
                  {
                     _loc9_ = false;
                  }
                  else
                  {
                     _loc9_ = _slots[7].data && _slots[7].data.oneHanded == false || _slots[10].data && _slots[10].data.oneHanded == false;
                  }
                  trace("isSlotFilled::",this._doubleTwoHanded,_loc9_,param3);
                  if(_loc10_.data && (ItemFactory.getPair(param2,_loc10_.data.dictId)) && (!_loc9_ || _loc9_ && param3))
                  {
                     _slots[_loc8_].highlighted = true;
                     trace("a");
                  }
                  else
                  {
                     _slots[_loc8_].highlighted = true;
                  }
               }
               else
               {
                  trace("b",this._doubleTwoHanded);
                  _loc11_ = param1 == 8 || param1 == 13 || param1 == 14 || param1 == 15 || param1 == 16 || param1 == 17;
                  if(_loc11_ && !param4)
                  {
                     if(this._doubleTwoHanded)
                     {
                        _loc9_ = false;
                     }
                     else
                     {
                        _loc9_ = _slots[7].data && _slots[7].data.oneHanded == false || _slots[10].data && _slots[10].data.oneHanded == false;
                     }
                     _loc6_ = _slots[7].data && _slots[7].data.dictId == param2 && _loc8_ == 10 || _slots[10].data && _slots[10].data.dictId == param2 && _loc8_ == 7;
                  }
                  if(!_slots[_loc8_].enabled)
                  {
                  }
                  if(!_loc11_ || !_loc6_ && (!_loc9_ || _loc9_ && (param3 || _slots[_loc8_].data != null && _slots[_loc8_].data.oneHanded == false)))
                  {
                     _slots[_loc8_].highlighted = true;
                  }
               }
            }
            _loc8_++;
         }
      }
      
      public function deselectAgeements() : void
      {
         var _loc1_:int = 0;
         while(_loc1_ < _slots.length)
         {
            if(_slots[_loc1_].enabled)
            {
               _slots[_loc1_].highlighted = false;
            }
            _loc1_++;
         }
      }
      
      public function clearSelection() : void
      {
         if(this._last_slot)
         {
            trace("PD:clearSelection");
            this._last_slot.selected = false;
            _last_name = "";
         }
      }
      
      override protected function configSlot(param1:IPaperDollSlot) : void
      {
      }
      
      override protected function handleSlotListeners(param1:IPaperDollSlot) : void
      {
         super.handleSlotListeners(param1);
         param1.addEventListener(MouseEvent.CLICK,this.onMouseItemClick);
         param1.addEventListener(ButtonEvent.CLICK,this.updateSelectedSlot);
         param1.addEventListener(Events.DROP_IN,this.updateSelectedSlot);
         param1.addEventListener(ButtonEvent.CLICK,this.newItemClick);
         param1.addEventListener(Events.DROP_IN,this.newItemClick);
      }
      
      override protected function configUI() : void
      {
         trace("LPD_base::configUI");
         super.configUI();
         var _loc1_:Array = [_slotNames.indexOf("slotWpn1A1"),_slotNames.indexOf("slotWpn1A2"),_slotNames.indexOf("slotWpn2A1"),_slotNames.indexOf("slotWpn2A2"),_slotNames.indexOf("slotWpn5"),_slotNames.indexOf("slotWpn6"),_slotNames.indexOf("slotWpn7"),_slotNames.indexOf("slotWpn8")];
         trace("LPD::configUI");
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotWpn1"),_loc1_);
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotWpn2"),_loc1_);
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotQck1"));
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotQck2"));
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotQck3"));
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotQck4"));
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotQck5"));
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotQck6"));
         this._ammoControl.addEventListener(Event.CHANGE,this.updateQuickSlots);
      }
      
      protected function updateQuickSlots(param1:Event) : void
      {
         trace("PD:updateQuickSlots");
         var _loc2_:int = 1;
         var _loc3_:ItemData;
         var _loc4_:ItemData;
         var _loc5_:Number;
         while(_loc2_ < 7)
         {
            if(_slots[_loc2_ + 12].data)
            {
               _loc3_ = _slots[_loc2_ + 12].data;
               _loc4_ = this._inventory.getInventoryItem(_loc3_.itemId);
               _loc5_ = _loc4_ ? _loc4_.value : 0;
               _loc3_.count = _loc5_;
               _slots[_loc2_ + 12].invalidateData();
            }
            _loc2_++;
         }
      }
      
      override protected function sortAmmo() : void
      {
         var _loc1_:int = -1;
         var _loc2_:int = -1;
         var _loc3_:int = 0;
         var _loc4_:int = 0;
         if(!_dataProvider)
         {
            return;
         }
         var _loc5_:uint = 0;
         var _loc6_:uint;
         var _loc7_:ItemData;
         var _loc8_:ItemData;
         var _loc9_:int;
         while(_loc5_ < _dataProvider.length)
         {
            if(_dataProvider[_loc5_].slotId == 7)
            {
               _loc1_ = (_loc5_);
            }
            else if(_dataProvider[_loc5_].slotId == 10)
            {
               _loc2_ = (_loc5_);
            }
            if(_loc1_ != -1 && _loc2_ != -1)
            {
               break;
            }
            _loc5_++;
         }
         if(_loc1_ != -1 && _loc1_ != -1 && _dataProvider[_loc1_] && _dataProvider[_loc2_] && ItemFactory.isSame(_dataProvider[_loc1_].dictId,_dataProvider[_loc2_].dictId))
         {
            _loc6_ = 1;
         }
         else
         {
            _loc6_ = 2;
         }
         for(_loc5_ = 0; _loc5_ < _dataProvider.length; _loc5_++)
         {
            if(_loc5_ == _loc1_ || _loc5_ == _loc2_)
            {
               continue;
            }
            _loc7_ = _dataProvider[_loc5_];
            if(!getSlotPair(_loc7_.category,8))
            {
               continue;
            }
            for(_loc9_ = 0; _loc9_ < _loc6_; _loc9_++)
            {
               _loc8_ = _loc9_ == 0 ? _dataProvider[_loc1_] : _dataProvider[_loc2_];
               if(!_loc8_)
               {
                  continue;
               }
               if(_loc8_)
               {
                  if(ItemFactory.getPair(_loc8_.dictId,_loc7_.dictId))
                  {
                     if(_loc7_.value == 0)
                     {
                        _loc7_.slotId = 999;
                     }
                     else
                     {
                        _loc7_.slotId = _loc9_ == 0 ? (this._ammo1_indexes[_loc3_]) : (this._ammo2_indexes[_loc4_]);
                        if(_loc9_ == 0)
                        {
                           _loc3_++;
                        }
                        else
                        {
                           _loc4_++;
                        }
                     }
                     continue;
                  }
               }
            }
         }
      }
      
      override protected function draw() : void
      {
         var slot:*;
         var isSlotFilled:Boolean;
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            if(_dataProvider != null)
            {
               function check_slot(param1:IPaperDollSlot, param2:IPaperDollSlot):void
               {
                  if(param1.data && param2.data && ItemFactory.isSame(param1.data.dictId,param2.data.dictId))
                  {
                     _shareMode = true;
                  }
                  else
                  {
                     _shareMode = false;
                  }
               }
               var i:int = 0;
               while(i < this._ammo_indexes.length)
               {
                  slot = _slots[this._ammo_indexes[i]];
                  slot.dragEnabled = false;
                  slot.mouseEnabled = false;
                  slot.mouseChildren = false;
                  i++;
               }
               slot = getChildByName("slotWpn2") as IPaperDollSlot;
               trace("pd_secondaryEnabled",this._secondaryEnabled,slot);
               slot.enabled = this._secondaryEnabled && (slot.data && slot.data.enabled || !slot.data);
               check_slot(getChildByName("slotWpn1"),getChildByName("slotWpn2"));
               i = 3;
               while(i < this._ammo_indexes.length)
               {
                  _slots[this._ammo_indexes[i]].visible = !this._shareMode;
                  i++;
               }
            }
            isSlotFilled = this._doubleTwoHanded ? false : _slots[7].data && !_slots[7].data.oneHanded || _slots[10].data && !_slots[10].data.oneHanded;
            _slots[7].doubleEnabled = !isSlotFilled;
            _slots[10].doubleEnabled = !isSlotFilled;
            dispatchEvent(new Event(PAPERDOLL_UPDATED));
         }
      }
      
      protected function onMouseItemClick(param1:MouseEventEx) : void
      {
         var _loc2_:IPaperDollSlot = param1.currentTarget as IPaperDollSlot;
         trace("onMouseItemClick:::",param1.target,param1.currentTarget,_loc2_,_loc2_ ? _loc2_.enabled : "nl",_loc2_ ? _loc2_.data : "nl");
         if(!_loc2_ || _loc2_.enabled == false || !_loc2_.data)
         {
            return;
         }
         if(param1.buttonIdx == 0)
         {
            return;
         }
         _contextIndex = _loc2_.index;
         this.updateSelectedSlot(param1);
         this._inventory.showContextMenu(_loc2_ as DragSlot);
      }
      
      private function newItemClick(param1:Event) : void
      {
         var _loc2_:* = param1.target;
         dispatchEvent(new ListEvent(ListEvent.ITEM_CLICK,true,false,param1.target.index,-1,-1,param1.target as IListItemRenderer,param1.target.data));
      }
      
      public function lockSlot(param1:Boolean, param2:uint) : void
      {
         var _loc3_:*;
         var _loc4_:uint;
         while(_loc4_ < _slots.length)
         {
            _loc3_ = _slots[_loc4_];
            if(_loc3_)
            {
               if(param1)
               {
                  _loc3_.hideContent(true,false);
               }
               else
               {
                  trace("showSlot::",_loc3_.name,_loc3_.isTemponary());
                  _loc3_.hideContent(false,false);
                  if((_loc3_.isTemponary()))
                  {
                     _loc3_.reset();
                  }
               }
            }
            _loc4_++;
         }
         _lockedSlot = param2;
      }
      
      public function onItemUpdated(param1:uint = 0) : void
      {
         trace("onItemUpdated",_dataProvider);
         var _loc2_:ItemData = null;
         var _loc3_:uint = 0;
         while(_loc3_ < _dataProvider.length)
         {
            if(_dataProvider[_loc3_] && _dataProvider[_loc3_].slotId == param1)
            {
               _loc2_ = _dataProvider[_loc3_];
            }
            _loc3_++;
         }
         trace("PD::data",_loc2_,this._last_slot);
         if(!this._last_slot || !this._last_slot.data || !this._last_slot.selected)
         {
            return;
         }
         var _loc4_:int = (this._last_slot.data.slotId);
         trace("PD:onItemUpdated::",_loc4_,param1,this._last_slot.data.name,"ups:",this._last_slot.data.upgrades);
         if(_loc4_ == param1)
         {
            dispatchEvent(new ListEvent(ListEvent.ITEM_CLICK,true,false,_loc4_,-1,-1,this._last_slot as IListItemRenderer,_loc2_));
         }
      }
      
      private function selectAvailableSlot() : void
      {
         trace("PD:selectAvailableSlot",this._last_name,this._last_slot);
         _last_name = "";
         var _loc1_:uint = 0;
         var _loc2_:int;
         var _loc3_:int;
         while(_loc1_ < this._slotSearch.length)
         {
            _loc2_ = _slotNames.indexOf(this._slotSearch[_loc1_]);
            if(_slots[_loc2_].data != null)
            {
               if(this._last_slot)
               {
                  this._last_slot.selected = false;
               }
               _last_slot = _slots[_loc2_];
               this._last_slot.selected = true;
               _last_name = this._slotSearch[_loc1_];
               _loc3_ = this._slots_index.indexOf(this._last_name);
               dispatchEvent(new ListEvent(ListEvent.ITEM_CLICK,true,false,_loc2_,-1,-1,this._last_slot as IListItemRenderer,this._last_slot.data));
               break;
            }
            _loc1_++;
         }
         if(this._last_name == "" && this._last_slot)
         {
            this._last_slot.selected = false;
         }
      }
      
      private function updateSelectedSlot(param1:Event = null) : void
      {
         trace("PD:updateSelectedSlot:",param1.type,param1.target.name,param1.target.data,param1.currentTarget,this._last_name);
         var _loc2_:* = param1.currentTarget;
         if(!_loc2_)
         {
            return;
         }
         var _loc3_:String = _loc2_.name;
         var _loc4_:int;
         var _loc5_:ItemData;
         if(this._last_name != _loc3_)
         {
            _loc4_ = this._slots_index.indexOf(_loc3_);
            if(_loc4_ < 2 && !_loc2_.data && param1.type != Events.DROP_IN)
            {
               this._inventory.selectCategory(_slotAgreements[_loc3_],true);
               _loc5_ = new ItemData();
               this.clearSelection();
               this._inventory.setPrimaryItem(_loc5_);
               _last_slot = _loc2_;
               _loc2_.selected = true;
               return;
            }
            _last_name = _loc3_;
            if(this._last_slot)
            {
               this._last_slot.selected = false;
            }
            _last_slot = _loc2_;
            _loc2_.selected = true;
            trace("updateSelectorSlot::",_loc4_,_loc2_.index);
            if(_loc2_ && param1.type != Events.DROP_IN)
            {
               this._inventory.selectCategory(_slotAgreements[_loc3_],!_loc2_.data);
               if(_loc2_.data)
               {
                  this._inventory.selectItem(_loc2_.data.itemId);
               }
            }
         }
         else if(_loc2_ && param1.type != Events.DROP_IN)
         {
            this._inventory.selectCategory(_slotAgreements[_loc3_],!_loc2_.data);
            if(_loc2_.data)
            {
               this._inventory.selectItem(_loc2_.data.itemId);
            }
         }
      }
   }
}

