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
   import survarium.upgrade.UpgradeWindow;
   import survarium.constants.Events;
   import survarium.data.ItemData;
   import survarium.interfaces.IAmmoControll;
   import survarium.interfaces.IInventoryItemFactory;
   import survarium.interfaces.ILocalPaperDoll;
   import survarium.interfaces.IPaperDollSlot;
   import survarium.item.ItemFactory;
   import survarium.paperdoll.PaperDollBase;
   import survarium.constants.ItemCategory;
   import survarium.constants.ProfileSlot;
   import survarium.constants.AmmoView;
   import survarium.constants.QuickSlotSize;
   import survarium.controls.PlayerInfo;
   
   public class LocalPaperDoll extends PaperDollBase implements ILocalPaperDoll
   {
      
      // until the dictionary's slots_restrictions arrive (setSlotsRestrictions) a slot takes these
      // item categories; the developers' defaults, not the restrictions' (back, legs, arms and foot differ)
      protected static var _slotAgreements:Object = {
         slotHead:[ItemCategory.HELMETS],
         slotMask:[ItemCategory.MASKS],
         slotBody:[ItemCategory.BODY_ARMOR],
         slotBack:[ItemCategory.BOOTS],
         slotLegs:[ItemCategory.RIGS],
         slotArms:[ItemCategory.PANTS],
         slotFoot:[ItemCategory.GLOVES],
         slotWpn1:[ItemCategory.ASSAULT_RIFLES],
         slotWpn1A1:[ItemCategory.ASSAULT_RIFLE_AMMO],
         slotWpn1A2:[ItemCategory.ASSAULT_RIFLE_AMMO],
         slotWpn2:[ItemCategory.ASSAULT_RIFLES],
         slotWpn2A1:[ItemCategory.ASSAULT_RIFLE_AMMO],
         slotWpn2A2:[ItemCategory.ASSAULT_RIFLE_AMMO],
         slotQck1:[ItemCategory.MEDKITS],
         slotQck2:[ItemCategory.MEDKITS],
         slotQck3:[ItemCategory.MEDKITS],
         slotQck4:[ItemCategory.MEDKITS],
         slotQck5:[ItemCategory.MEDKITS],
         slotQck6:[ItemCategory.MEDKITS],
         slotWpn5:[ItemCategory.ASSAULT_RIFLE_AMMO],
         slotWpn6:[ItemCategory.ASSAULT_RIFLE_AMMO],
         slotWpn7:[ItemCategory.ASSAULT_RIFLE_AMMO],
         slotWpn8:[ItemCategory.ASSAULT_RIFLE_AMMO]
      };
      
      protected var _inventory:IInventoryItemFactory;
      
      protected var _shareMode:Boolean = false;
      
      protected var _ammo_indexes:Array = [ProfileSlot.WEAPON_1_AMMO_1,ProfileSlot.WEAPON_1_AMMO_2,ProfileSlot.WEAPON_2_AMMO_1,ProfileSlot.WEAPON_2_AMMO_2,ProfileSlot.AMMO_5,ProfileSlot.AMMO_6];
      
      protected var _ammo1_indexes:Array = [ProfileSlot.WEAPON_1_AMMO_1,ProfileSlot.WEAPON_1_AMMO_2,ProfileSlot.WEAPON_2_AMMO_1];
      
      protected var _ammo2_indexes:Array = [ProfileSlot.WEAPON_2_AMMO_2,ProfileSlot.AMMO_5,ProfileSlot.AMMO_6];
      
      // _slots_index starts with the two weapons
      private static const WEAPON_SLOTS:int = 2;
      
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
      
      public static function getSlotPair(category:int, slotIndex:int) : Boolean
      {
         if((_slotAgreements[_slotNames[slotIndex]] as Array).indexOf(category) != -1)
         {
            return true;
         }
         return false;
      }
      
      // An item type goes into one pocket of a kit (docs/notes/one-pocket-a-type.md): the units of
      // `item` a pocket holding `held` takes when one more is put in. The pocket's own type: one more,
      // within the pocket's size (one of an item that fills a whole pocket); another: one.
      // An int: the caller puts it in a move through an int local (ItemPaperDollSlot.handleDropEvent).
      public static function pocketUnits(held:ItemData, item:ItemData) : int
      {
         if(!held || held.dictId != item.dictId)
         {
            return 1;
         }
         var size:int = item.quickSlotItemSize == QuickSlotSize.WHOLE ? 1 : PlayerInfo.pocketSize;
         return int(Math.max(1,Math.min(held.value + 1,size)));
      }
      
      // The pocket holding an item of `dictId`, or -1.
      public function pocketOf(dictId:int) : int
      {
         var i:int = ProfileSlot.QUICK_1;
         while(i <= ProfileSlot.QUICK_6)
         {
            if(_slots[i] && _slots[i].data && _slots[i].data.dictId == dictId)
            {
               return i;
            }
            i++;
         }
         return -1;
      }
      
      public function set ammoControl(value:IAmmoControll) : void
      {
         _ammoControl = value;
         addChild(this._ammoControl as DisplayObject);
      }
      
      public function get ammoControl() : IAmmoControll
      {
         return this._ammoControl;
      }
      
      override public function set dataProvider(value:IDataProvider) : void
      {
         super.dataProvider = value;
         this._ammoControl.updatePaperDollData(_dataProvider,_slots);
         this._ammoControl.update(null,"paperdoll");
      }
      
      public function get selectedIndex() : int
      {
         return this._slots_index.indexOf(this._last_name);
      }
      
      // The kit's weapons (slots 1 and 2) a hovered sight fits look hovered too; -1 ends it
      // (weapon_scopes.md, section 4a).
      private var _linkedHovers:Array = [];

      public function hoverWeaponsFor(scopeDictId:int = -1) : void
      {
         for each(var linked:int in this._linkedHovers)
         {
            if(linked != this._hoverIndex)
            {
               getSlotAt(linked).hover(false);
            }
         }
         _linkedHovers = [];
         if(scopeDictId < 0)
         {
            return;
         }
         for each(var index:int in [ProfileSlot.WEAPON_1,ProfileSlot.WEAPON_2])
         {
            var slot:IPaperDollSlot = getSlotAt(index);
            if(slot != null && slot.data != null && UpgradeWindow.fits(int(slot.data.dictId),scopeDictId))
            {
               slot.hover();
               this._linkedHovers.push(index);
            }
         }
      }

      public function hoverItem(itemId:uint = 0) : void
      {
         trace("PD::hoverItem",itemId);
         if(!itemId && this._hoverIndex)
         {
            getSlotAt(this._hoverIndex).hover(false);
            _hoverIndex = 0;
            return;
         }
         var count:uint = _slots.length;
         var i:int = 0;
         var slot:Object;
         while(i < count)
         {
            slot = _slots[i];
            if(slot && slot.data && slot.data.itemId == itemId)
            {
               slot.hover();
               _hoverIndex = i;
               return;
            }
            i++;
         }
      }
      
      public function enableSecondWeapon(isEnabled:Boolean) : void
      {
         trace("enableSecondWeapon::",isEnabled);
         _secondaryEnabled = isEnabled;
         invalidate(InvalidationType.DATA);
      }
      
      public function allowSecondaryTwoHandedWeapon(isAllowed:Boolean) : void
      {
         trace("allowSecondaryTwoHandedWeapon::",isAllowed);
         _doubleTwoHanded = isAllowed;
         invalidate(InvalidationType.DATA);
      }
      
      public function getSlot(item:ItemData, findFree:Boolean = false) : int
      {
         stackTrace("getSlot::" + String(item));
         if(!item)
         {
            return -1;
         }
         var i:int = 0;
         var category:uint;
         var slot:IPaperDollSlot;
         var fits:Boolean;
         var j:int;
         while(i < _slots.length)
         {
            category = item.category;
            fits = (_slotAgreements[_slots[i].name] as Array).indexOf(category) != -1;
            if(fits)
            {
               if(category == ItemCategory.MEDKITS || category == ItemCategory.BOOBY_TRAPS || category == ItemCategory.GRENADES)
               {
                  // a type the kit has goes into its own pocket, never a second one
                  j = this.pocketOf(item.dictId);
                  if(j != -1)
                  {
                     return j;
                  }
                  j = ProfileSlot.QUICK_1;
                  while(j < _slots.length)
                  {
                     if(findFree)
                     {
                        trace("j::",j,_slotsN + 12);
                        if(!_slots[j].data || j == _slotsN + 12)
                        {
                           slot = _slots[j];
                           return j;
                        }
                     }
                     else
                     {
                        if(_slots[j].data != null)
                        {
                           slot = _slots[j];
                           return j;
                        }
                        slot = _slots[ProfileSlot.QUICK_1];
                        return ProfileSlot.QUICK_1;
                     }
                     j++;
                  }
               }
               slot = _slots[i];
               if(slot && slot.highlighted)
               {
                  return i;
               }
            }
            i++;
         }
         return -1;
      }
      
      override public function toString() : String
      {
         return "[LocalPaperDoll " + name + "]";
      }
      
      public function setFillArray(items:Array) : void
      {
         this._ammoControl.fillItems(items);
      }
      
      public function setSlotsRestrictions(restrictions:Array) : void
      {
         var slotName:String;
         var i:int;
         var restriction:Object;
         for(slotName in _slotAgreements)
         {
            _slotAgreements[slotName] = new Array();
         }
         i = 0;
         while(i < restrictions.length)
         {
            restriction = restrictions[i];
            _slotAgreements[_slotNames[restriction.slot_id]].push(restriction.category_id);
            i++;
         }
      }
      
      public function registerInventory(inventory:IInventoryItemFactory) : void
      {
         _inventory = inventory;
      }
      
      override public function openSlots(slotsN:int, slotsA:int) : void
      {
         this._ammoControl.openSlots(slotsN,slotsA);
         super.openSlots(slotsN,slotsA);
      }
      
      public function hasAgreements(category:uint) : uint
      {
         if(!enabled)
         {
            return 0;
         }
         var count:uint = _slots.length;
         var filledCount:uint = 0;
         var i:int = 0;
         var isSlotFilled:Boolean;
         while(i < count)
         {
            isSlotFilled = false;
            if(_slotAgreements[_slots[i].name] && ((_slotAgreements[_slots[i].name] as Array).indexOf(category) != -1) && _slots[i].enabled)
            {
               if(!_slots[i].data)
               {
                  return 1;
               }
               filledCount++;
            }
            i++;
         }
         return filledCount;
      }
      
      public function selectAgeements(category:int, dictId:int, oneHanded:Boolean = false, fromSlot:Boolean = false, itemId:int = 0) : void
      {
         trace("selectAgeements",category,dictId,oneHanded,itemId);
         if(!enabled)
         {
            return;
         }
         var isSameWeapon:Boolean = false;
         var count:uint = _slots.length;
         var i:int = 0;
         var isSlotFilled:Boolean;
         var weaponSlot:IPaperDollSlot;
         var isWeapon:Boolean;
         var pocket:int = this.pocketOf(dictId);
         var isOtherPocket:Boolean;
         while(i < count)
         {
            isSlotFilled = false;
            if(_slotAgreements[_slots[i].name] && ((_slotAgreements[_slots[i].name] as Array).indexOf(category) != -1) && _slots[i].enabled)
            {
               if(category == ItemCategory.ASSAULT_RIFLE_AMMO || category == ItemCategory.SNIPER_RIFLE_AMMO || category == ItemCategory.SHOTGUN_AMMO || category == ItemCategory.PISTOL_AMMO || category == ItemCategory.MACHINE_GUN_AMMO)
               {
                  if(i == ProfileSlot.WEAPON_1_AMMO_1 || i == ProfileSlot.WEAPON_1_AMMO_2)
                  {
                     weaponSlot = _slots[ProfileSlot.WEAPON_1];
                  }
                  else if(i == ProfileSlot.WEAPON_2_AMMO_1 || i == ProfileSlot.WEAPON_2_AMMO_2)
                  {
                     weaponSlot = _slots[ProfileSlot.WEAPON_2];
                  }
                  if(this._doubleTwoHanded)
                  {
                     isSlotFilled = false;
                  }
                  else
                  {
                     isSlotFilled = _slots[ProfileSlot.WEAPON_1].data && _slots[ProfileSlot.WEAPON_1].data.oneHanded == false || _slots[ProfileSlot.WEAPON_2].data && _slots[ProfileSlot.WEAPON_2].data.oneHanded == false;
                  }
                  trace("isSlotFilled::",this._doubleTwoHanded,isSlotFilled,oneHanded);
                  if(weaponSlot.data && (ItemFactory.getPair(dictId,weaponSlot.data.dictId)) && (!isSlotFilled || isSlotFilled && oneHanded))
                  {
                     _slots[i].highlighted = true;
                     trace("a");
                  }
                  else
                  {
                     _slots[i].highlighted = true;
                  }
               }
               else
               {
                  trace("b",this._doubleTwoHanded);
                  isWeapon = category == ItemCategory.ASSAULT_RIFLES || category == ItemCategory.SNIPER_RIFLES || category == ItemCategory.SHOTGUNS || category == ItemCategory.SUBMACHINE_GUNS || category == ItemCategory.MACHINE_GUNS || category == ItemCategory.PISTOLS;
                  if(isWeapon && !fromSlot)
                  {
                     if(this._doubleTwoHanded)
                     {
                        isSlotFilled = false;
                     }
                     else
                     {
                        isSlotFilled = _slots[ProfileSlot.WEAPON_1].data && _slots[ProfileSlot.WEAPON_1].data.oneHanded == false || _slots[ProfileSlot.WEAPON_2].data && _slots[ProfileSlot.WEAPON_2].data.oneHanded == false;
                     }
                     isSameWeapon = _slots[ProfileSlot.WEAPON_1].data && _slots[ProfileSlot.WEAPON_1].data.dictId == dictId && i == ProfileSlot.WEAPON_2 || _slots[ProfileSlot.WEAPON_2].data && _slots[ProfileSlot.WEAPON_2].data.dictId == dictId && i == ProfileSlot.WEAPON_1;
                  }
                  if(!_slots[i].enabled)
                  {
                  }
                  // one pocket a type: from the inventory or the shop, only the pocket that holds it
                  // (an item dragged from a pocket only changes pockets)
                  isOtherPocket = !fromSlot && i >= ProfileSlot.QUICK_1 && i <= ProfileSlot.QUICK_6 && pocket != -1 && pocket != i;
                  if(!isOtherPocket && (!isWeapon || !isSameWeapon && (!isSlotFilled || isSlotFilled && (oneHanded || _slots[i].data != null && _slots[i].data.oneHanded == false))))
                  {
                     _slots[i].highlighted = true;
                  }
               }
            }
            i++;
         }
      }
      
      public function deselectAgeements() : void
      {
         var i:int = 0;
         while(i < _slots.length)
         {
            if(_slots[i].enabled)
            {
               _slots[i].highlighted = false;
            }
            i++;
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
      
      override protected function configSlot(slot:IPaperDollSlot) : void
      {
      }
      
      override protected function handleSlotListeners(slot:IPaperDollSlot) : void
      {
         super.handleSlotListeners(slot);
         slot.addEventListener(MouseEvent.CLICK,this.onMouseItemClick);
         slot.addEventListener(ButtonEvent.CLICK,this.updateSelectedSlot);
         slot.addEventListener(Events.DROP_IN,this.updateSelectedSlot);
         slot.addEventListener(ButtonEvent.CLICK,this.newItemClick);
         slot.addEventListener(Events.DROP_IN,this.newItemClick);
      }
      
      override protected function configUI() : void
      {
         trace("LPD_base::configUI");
         super.configUI();
         var ammoSlots:Array = [_slotNames.indexOf("slotWpn1A1"),_slotNames.indexOf("slotWpn1A2"),_slotNames.indexOf("slotWpn2A1"),_slotNames.indexOf("slotWpn2A2"),_slotNames.indexOf("slotWpn5"),_slotNames.indexOf("slotWpn6"),_slotNames.indexOf("slotWpn7"),_slotNames.indexOf("slotWpn8")];
         trace("LPD::configUI");
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotWpn1"),ammoSlots);
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotWpn2"),ammoSlots);
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotQck1"));
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotQck2"));
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotQck3"));
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotQck4"));
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotQck5"));
         this._ammoControl.setSlotTarget(_slotNames.indexOf("slotQck6"));
         this._ammoControl.addEventListener(Event.CHANGE,this.updateQuickSlots);
      }
      
      protected function updateQuickSlots(event:Event) : void
      {
         trace("PD:updateQuickSlots");
         var i:int = 1;
         var item:ItemData;
         var inventoryItem:ItemData;
         var count:Number;
         while(i < 7)
         {
            if(_slots[i + 12].data)
            {
               item = _slots[i + 12].data;
               inventoryItem = this._inventory.getInventoryItem(item.itemId);
               count = inventoryItem ? inventoryItem.value : 0;
               item.count = count;
               _slots[i + 12].invalidateData();
            }
            i++;
         }
      }
      
      override protected function sortAmmo() : void
      {
         var primaryIndex:int = -1;
         var secondaryIndex:int = -1;
         var primaryAmmoCount:int = 0;
         var secondaryAmmoCount:int = 0;
         if(!_dataProvider)
         {
            return;
         }
         var i:uint = 0;
         var weaponCount:uint;
         var item:ItemData;
         var weapon:ItemData;
         var j:int;
         while(i < _dataProvider.length)
         {
            if(_dataProvider[i].slotId == ProfileSlot.WEAPON_1)
            {
               primaryIndex = (i);
            }
            else if(_dataProvider[i].slotId == ProfileSlot.WEAPON_2)
            {
               secondaryIndex = (i);
            }
            if(primaryIndex != -1 && secondaryIndex != -1)
            {
               break;
            }
            i++;
         }
         if(primaryIndex != -1 && primaryIndex != -1 && _dataProvider[primaryIndex] && _dataProvider[secondaryIndex] && ItemFactory.isSame(_dataProvider[primaryIndex].dictId,_dataProvider[secondaryIndex].dictId))
         {
            weaponCount = 1;
         }
         else
         {
            weaponCount = 2;
         }
         for(i = 0; i < _dataProvider.length; i++)
         {
            if(i == primaryIndex || i == secondaryIndex)
            {
               continue;
            }
            item = _dataProvider[i];
            if(!getSlotPair(item.category,ProfileSlot.WEAPON_1_AMMO_1))
            {
               continue;
            }
            for(j = 0; j < weaponCount; j++)
            {
               weapon = j == 0 ? _dataProvider[primaryIndex] : _dataProvider[secondaryIndex];
               if(!weapon)
               {
                  continue;
               }
               if(weapon)
               {
                  if(ItemFactory.getPair(weapon.dictId,item.dictId))
                  {
                     if(item.value == 0)
                     {
                        item.slotId = ProfileSlot.NONE;
                     }
                     else
                     {
                        item.slotId = j == 0 ? (this._ammo1_indexes[primaryAmmoCount]) : (this._ammo2_indexes[secondaryAmmoCount]);
                        if(j == 0)
                        {
                           primaryAmmoCount++;
                        }
                        else
                        {
                           secondaryAmmoCount++;
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
               function check_slot(slotA:IPaperDollSlot, slotB:IPaperDollSlot):void
               {
                  if(slotA.data && slotB.data && ItemFactory.isSame(slotA.data.dictId,slotB.data.dictId))
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
               i = AmmoView.SLOTS_PER_WEAPON;
               while(i < this._ammo_indexes.length)
               {
                  _slots[this._ammo_indexes[i]].visible = !this._shareMode;
                  i++;
               }
            }
            isSlotFilled = this._doubleTwoHanded ? false : _slots[ProfileSlot.WEAPON_1].data && !_slots[ProfileSlot.WEAPON_1].data.oneHanded || _slots[ProfileSlot.WEAPON_2].data && !_slots[ProfileSlot.WEAPON_2].data.oneHanded;
            _slots[ProfileSlot.WEAPON_1].doubleEnabled = !isSlotFilled;
            _slots[ProfileSlot.WEAPON_2].doubleEnabled = !isSlotFilled;
            dispatchEvent(new Event(PAPERDOLL_UPDATED));
         }
      }
      
      protected function onMouseItemClick(event:MouseEventEx) : void
      {
         var slot:IPaperDollSlot = event.currentTarget as IPaperDollSlot;
         trace("onMouseItemClick:::",event.target,event.currentTarget,slot,slot ? slot.enabled : "nl",slot ? slot.data : "nl");
         if(!slot || slot.enabled == false || !slot.data)
         {
            return;
         }
         if(event.buttonIdx == MouseEventEx.LEFT_BUTTON)
         {
            return;
         }
         _contextIndex = slot.index;
         this.updateSelectedSlot(event);
         this._inventory.showContextMenu(slot as DragSlot);
      }
      
      private function newItemClick(event:Event) : void
      {
         var target:* = event.target;
         dispatchEvent(new ListEvent(ListEvent.ITEM_CLICK,true,false,event.target.index,-1,-1,event.target as IListItemRenderer,event.target.data));
      }
      
      public function lockSlot(isLocked:Boolean, lockedSlot:uint) : void
      {
         var slot:*;
         var i:uint;
         while(i < _slots.length)
         {
            slot = _slots[i];
            if(slot)
            {
               if(isLocked)
               {
                  slot.hideContent(true,false);
               }
               else
               {
                  trace("showSlot::",slot.name,slot.isTemponary());
                  slot.hideContent(false,false);
                  if((slot.isTemponary()))
                  {
                     slot.reset();
                  }
               }
            }
            i++;
         }
         _lockedSlot = lockedSlot;
      }
      
      public function onItemUpdated(slotId:uint = 0) : void
      {
         trace("onItemUpdated",_dataProvider);
         var item:ItemData = null;
         var i:uint = 0;
         while(i < _dataProvider.length)
         {
            if(_dataProvider[i] && _dataProvider[i].slotId == slotId)
            {
               item = _dataProvider[i];
            }
            i++;
         }
         trace("PD::data",item,this._last_slot);
         if(!this._last_slot || !this._last_slot.data || !this._last_slot.selected)
         {
            return;
         }
         var lastSlotId:int = (this._last_slot.data.slotId);
         trace("PD:onItemUpdated::",lastSlotId,slotId,this._last_slot.data.name,"ups:",this._last_slot.data.upgrades);
         if(lastSlotId == slotId)
         {
            dispatchEvent(new ListEvent(ListEvent.ITEM_CLICK,true,false,lastSlotId,-1,-1,this._last_slot as IListItemRenderer,item));
         }
      }
      
      private function selectAvailableSlot() : void
      {
         trace("PD:selectAvailableSlot",this._last_name,this._last_slot);
         _last_name = "";
         var i:uint = 0;
         var slotIndex:int;
         var listIndex:int;
         while(i < this._slotSearch.length)
         {
            slotIndex = _slotNames.indexOf(this._slotSearch[i]);
            if(_slots[slotIndex].data != null)
            {
               if(this._last_slot)
               {
                  this._last_slot.selected = false;
               }
               _last_slot = _slots[slotIndex];
               this._last_slot.selected = true;
               _last_name = this._slotSearch[i];
               listIndex = this._slots_index.indexOf(this._last_name);
               dispatchEvent(new ListEvent(ListEvent.ITEM_CLICK,true,false,slotIndex,-1,-1,this._last_slot as IListItemRenderer,this._last_slot.data));
               break;
            }
            i++;
         }
         if(this._last_name == "" && this._last_slot)
         {
            this._last_slot.selected = false;
         }
      }
      
      private function updateSelectedSlot(event:Event = null) : void
      {
         trace("PD:updateSelectedSlot:",event.type,event.target.name,event.target.data,event.currentTarget,this._last_name);
         var slot:* = event.currentTarget;
         if(!slot)
         {
            return;
         }
         var slotName:String = slot.name;
         var listIndex:int;
         var emptyItem:ItemData;
         if(this._last_name != slotName)
         {
            listIndex = this._slots_index.indexOf(slotName);
            if(listIndex < WEAPON_SLOTS && !slot.data && event.type != Events.DROP_IN)
            {
               this._inventory.selectCategory(_slotAgreements[slotName],true);
               emptyItem = new ItemData();
               this.clearSelection();
               this._inventory.setPrimaryItem(emptyItem);
               _last_slot = slot;
               slot.selected = true;
               return;
            }
            _last_name = slotName;
            if(this._last_slot)
            {
               this._last_slot.selected = false;
            }
            _last_slot = slot;
            slot.selected = true;
            trace("updateSelectorSlot::",listIndex,slot.index);
            if(slot && event.type != Events.DROP_IN)
            {
               this._inventory.selectCategory(_slotAgreements[slotName],!slot.data);
               if(slot.data)
               {
                  this._inventory.selectItem(slot.data.itemId);
               }
            }
         }
         else if(slot && event.type != Events.DROP_IN)
         {
            this._inventory.selectCategory(_slotAgreements[slotName],!slot.data);
            if(slot.data)
            {
               this._inventory.selectItem(slot.data.itemId);
            }
         }
      }
   }
}

