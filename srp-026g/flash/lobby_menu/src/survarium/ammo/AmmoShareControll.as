package survarium.ammo
{
   import flash.display.Sprite;
   import flash.events.Event;
   import scaleform.clik.interfaces.IDataProvider;
   import scaleform.gfx.MouseEventEx;
   import survarium.controls.GangShop;
   import survarium.controls.InventoryList;
   import survarium.controls.PlayerInfo;
   import survarium.core.Icon;
   import survarium.data.ItemData;
   import survarium.interfaces.IAmmoControll;
   import survarium.interfaces.ILocalPaperDoll;
   import survarium.interfaces.ILocalPlayerProfile;
   import survarium.item.ItemFactory;
   import survarium.player.LocalPaperDoll;
   
   public class AmmoShareControll extends Sprite implements IAmmoControll
   {
      
      protected static var _itemsAutoBuy:Array = [];
      
      protected var _targetAmmo:Array = new Array();
      
      protected var _targetSlots:Array = new Array();
      
      protected var _targetId:int = -1;
      
      protected var _compassionItems:Array = new Array();
      
      protected var _inventory:InventoryList;
      
      protected var _shop:GangShop;
      
      protected var _slotsN:int = 0;
      
      protected var _slotsA:int = 0;
      
      protected var _totalAmmo1:Number = 0;
      
      protected var _totalAmmo2:Number = 0;
      
      protected var _activeIndex:int = -1;
      
      protected var _pdData:Array;
      
      protected var _softUpdate:Boolean = false;
      
      protected var _slotItems:Array = new Array();
      
      public function AmmoShareControll() : void
      {
         super();
      }
      
      public function setSlotTarget(value:uint, value_array:Array = null) : void
      {
         if((value_array))
         {
            this._targetAmmo.push({
               weapon_slot:value,
               ammo_slots:value_array
            });
         }
         else
         {
            this._targetSlots.push({slotId:value});
         }
      }
      
      public function registerShop(value:*) : void
      {
         _shop = value;
         if((this._shop))
         {
            this._shop.addEventListener(Event.CHANGE,this.update);
         }
      }
      
      public function registerInventory(value:*) : void
      {
         _inventory = value;
         this._inventory.addEventListener(Event.CHANGE,this.update);
      }
      
      public function openSlots(slotsN:int, slotsA:int) : void
      {
         _slotsN = slotsN;
         slotsA = slotsA;
      }
      
      public function fillItems(value:Array) : void
      {
         _itemsAutoBuy = value;
         var pp:ILocalPlayerProfile = this.parent.parent as ILocalPlayerProfile;
         var _ammoShare:AmmoShareLite = pp.get_ammo_share() as AmmoShareLite;
         if((_ammoShare.parent))
         {
            this.update();
         }
      }
      
      public function reflow(event:Event = null, caller:String = "null", ind:int = -2) : void
      {
         _activeIndex = ind != -2 ? ind : (parent as ILocalPaperDoll).selectedIndex;
         var _ammoShare:AmmoShareLite = AmmoShareLite.getInstance();
         _ammoShare.activeIndex = this._activeIndex;
         _ammoShare.reflowElements();
      }
      
      public function update(event:Event = null, caller:String = "null") : String
      {
         trace("updateShareComponent::",(event) ? event.target : "no_target",caller);
         if((event) && event.target == this._inventory)
         {
            _softUpdate = true;
         }
         else
         {
            _softUpdate = false;
         }
         if(!hasEventListener(Event.RENDER))
         {
            addEventListener(Event.RENDER,this._update);
         }
         return "false";
      }
      
      public function updatePaperDollData(dataProvider:IDataProvider, slots:Array) : void
      {
         if((this._pdData))
         {
            this._pdData.length = 0;
         }
         _pdData = new Array();
         for(var i:uint = 0; i < dataProvider.length; i++)
         {
            this._pdData[dataProvider[i].slotId] = dataProvider[i];
         }
         for(i = 0; i < slots.length; i++)
         {
            if(!this._pdData[slots[i].index])
            {
               var tempData:ItemData;
               tempData = new ItemData();
               tempData.index = slots[i].index;
               this._pdData[slots[i].index] = tempData;
            }
            else
            {
               this._pdData[slots[i].index].index = slots[i].index;
            }
         }
      }
      
      protected function fillSlots() : void
      {
         var l:int = (this._targetSlots.length);
         for(var i:uint = 0; i < l; i++)
         {
            var itemData:ItemData;
            itemData = this._pdData[this._targetSlots[i].slotId];
            if((itemData) && (itemData.itemId))
            {
               trace("control_add_item:",itemData.name,itemData.itemId,itemData.canBuy,"st:",itemData.slotId,itemData.slotTarget,"count:",itemData.count,itemData.value);
               this._slotItems.push(itemData);
            }
         }
      }
      
      protected function showShareComponent(e:MouseEventEx = null) : void
      {
         trace("showShareComponent:",e);
         _activeIndex = (parent as ILocalPaperDoll).selectedIndex;
         if(!this._targetAmmo.length)
         {
            return;
         }
         _totalAmmo1 = 0;
         _totalAmmo2 = 0;
         this._compassionItems.length = 0;
         this._slotItems.length = 0;
         this._inventory.forEachAmmo(this.findAviableItems);
         this._shop.forEachAmmo(this.findAviableItems);
         this.fillSlots();
         var pp:ILocalPlayerProfile = this.parent.parent as ILocalPlayerProfile;
         var _ammoShare:AmmoShareLite = pp.get_ammo_share() as AmmoShareLite;
         trace("setAmmoActiveIndex:",this._activeIndex,_ammoShare.activeIndex);
         _ammoShare.aviableWeight = PlayerInfo.maxWeight - PlayerInfo.curWeight;
         _ammoShare.ammoBags = PlayerInfo.ammoBags;
         _ammoShare.pocketSize = PlayerInfo.pocketSize;
         trace("ammoShare::",_ammoShare,"founded:",this._compassionItems,"shop:",this._shop);
         trace("calc_free_weight:",_ammoShare.aviableWeight,PlayerInfo.curWeight);
         for(var i:int = 0; i < this._targetAmmo.length; i++)
         {
            if(i == 1)
            {
               continue;
            }
            _mainSlot = this._pdData[this._targetAmmo[i].weapon_slot];
            if((_mainSlot) && (_mainSlot.id))
            {
               if((this._targetAmmo[i].ammo_slots.length))
               {
                  for(j = 0; j < this._targetAmmo[i].ammo_slots.length; j++)
                  {
                     _targetSlot = this._pdData[this._targetAmmo[i].ammo_slots[j]];
                     if((_targetSlot) && (_targetSlot.id))
                     {
                        _ammoShare.aviableWeight += _targetSlot.condition_or_stack / _targetSlot.clip_size * _targetSlot.weight;
                     }
                  }
               }
               else
               {
                  _ammoShare.aviableWeight += _mainSlot.weight * _mainSlot.condition_or_stack;
               }
            }
         }
         trace("calc_free_weight_end:",_ammoShare.aviableWeight,PlayerInfo.curWeight);
         var wp1:Object = this._pdData[this._targetAmmo[0].weapon_slot];
         var wp2:Object = this._pdData[this._targetAmmo[1].weapon_slot];
         var _mainSlot:Object;
         var j:int;
         var _targetSlot:Object;
         if((wp1 && wp1.dictId != undefined) && (wp2) && wp2.dictId != undefined)
         {
            if(ItemFactory.isSame(wp1.dictId,wp2.dictId))
            {
               _ammoShare.setShared(true);
            }
         }
         else
         {
            _ammoShare.setShared(false);
         }
         _ammoShare.setSlots(this._slotsN);
         _ammoShare.setData(this._compassionItems,this._slotItems,true,this._softUpdate);
         _ammoShare.setProfileId(pp.profileId);
      }
      
      protected function _update(event:Event = null) : void
      {
         trace("ASC::update",parent);
         if(!parent)
         {
            return;
         }
         _activeIndex = (parent as ILocalPaperDoll).selectedIndex;
         removeEventListener(Event.RENDER,this._update);
         if(!this._targetAmmo || !this._pdData)
         {
            return;
         }
         var totalW:Number = 0;
         var _max:Number = 0;
         var _sum1:Number = 0;
         var pp:ILocalPlayerProfile = this.parent.parent as ILocalPlayerProfile;
         var _ammoShare:AmmoShareLite = pp.get_ammo_share() as AmmoShareLite;
         var _isEquivalent:Boolean = false;
         this.showShareComponent();
         dispatchEvent(new Event(Event.CHANGE,true));
         trace("ammoASC:",this._totalAmmo1 + this._totalAmmo2,"sh:",_ammoShare.getShared(),_sum1,"mo:",this._totalAmmo1,this._totalAmmo2,_max,totalW,_max / totalW * 100,int(_max / totalW * 100).toString() + "%","\n");
      }
      
      protected function findAviableItems(inv:Object = null) : void
      {
         if(!this._pdData)
         {
            return;
         }
         var nm:int = this._targetAmmo.length;
         for(var i:int = 0; i < nm; i++)
         {
            var isQuickslot:Boolean;
            var main_data:ItemData;
            var canBePlaced:Boolean;
            var slot_target:int;
            var h:int;
            var count:int;
            var _icon:Icon;
            var clips:Number;
            var auto_buy:Boolean;
            var itemData:ItemData;
            var j:int;
            var ammo_data:ItemData;
            isQuickslot = this._targetAmmo[i].ammo_slots.length == 0;
            if(this._pdData[this._targetAmmo[i].weapon_slot].dictId || isQuickslot)
            {
               main_data = this._pdData[this._targetAmmo[i].weapon_slot];
               if(!isQuickslot)
               {
                  _targetId = main_data.dictId;
                  canBePlaced = ItemFactory.getPair(inv.dictId,this._targetId);
               }
               else
               {
                  _targetId = this._targetAmmo[i].weapon_slot;
                  canBePlaced = LocalPaperDoll.getSlotPair(inv.category,this._targetId);
               }
               if(canBePlaced)
               {
                  slot_target = 0;
                  for(h = 0; h < this._compassionItems.length; h++)
                  {
                     if(this._compassionItems[h].dictId == inv.dictId)
                     {
                        if(this._compassionItems[h].canBuy == false && inv.canBuy == true || this._compassionItems[h].canBuy == undefined)
                        {
                           this._compassionItems[h].canBuy = inv.canBuy;
                        }
                        return;
                     }
                  }
                  count = inv.value;
                  if(isQuickslot)
                  {
                     if(main_data && inv.itemId != 0 && inv.itemId == main_data.itemId)
                     {
                        slot_target = this._targetAmmo[i].weapon_slot;
                        count = main_data.value;
                     }
                     else if(i == nm - 1)
                     {
                        slot_target = 0;
                     }
                     else
                     {
                        continue;
                     }
                  }
                  else
                  {
                     for(j = 0; j < this._targetAmmo[i].ammo_slots.length; j++)
                     {
                        ammo_data = this._pdData[this._targetAmmo[i].ammo_slots[j]];
                        if(ammo_data && inv.itemId != 0 && inv.itemId == ammo_data.itemId)
                        {
                           slot_target = this._targetAmmo[i].ammo_slots[j];
                           count = ammo_data.value;
                           break;
                        }
                        else
                        {
                           count = 0;
                        }
                     }
                  }
                  if(!isQuickslot && (inv.value))
                  {
                     if(i == 0)
                     {
                        _totalAmmo1 += inv.value;
                     }
                     else
                     {
                        _totalAmmo2 += inv.value;
                     }
                  }
                  if(inv.clipSize == 0)
                  {
                     inv.clipSize = 1;
                  }
                  if((inv.icon))
                  {
                     _icon = (inv.icon as Icon).clone();
                  }
                  else
                  {
                     _icon = new Icon();
                  }
                  clips = 1;
                  if((inv.clipsInBag))
                  {
                     clips = (inv.clipsInBag);
                  }
                  else if((inv.quickSlotItemSize))
                  {
                     clips = 1 / inv.quickSlotItemSize;
                  }
                  auto_buy = false;
                  for(j = 0; j < _itemsAutoBuy.length; j++)
                  {
                     auto_buy = _itemsAutoBuy[j] == inv.itemId;
                     if(auto_buy)
                     {
                        break;
                     }
                  }
                  itemData = new ItemData();
                  itemData.name = inv.name;
                  itemData.dictId = inv.dictId;
                  itemData.category = inv.category;
                  itemData.itemId = inv.itemId;
                  itemData.isStack = inv.isStack;
                  itemData.value = inv.value;
                  itemData.itemDescription = inv.itemDescription;
                  itemData.properties = inv.properties;
                  itemData.clipSize = inv.clipSize;
                  itemData.weight = inv.weight;
                  itemData.cost = inv.cost;
                  itemData.premiumCost = inv.premiumCost;
                  itemData.lockText = inv.lockText;
                  itemData.unlockLevelLabel = inv.unlockLevelLabel;
                  itemData.canBuy = inv.canBuy;
                  itemData.itemLevel = inv.itemLevel;
                  itemData.clipsInBag = clips;
                  itemData.slotId = slot_target;
                  itemData.slotTarget = slot_target;
                  itemData.count = count;
                  itemData.icon = _icon;
                  itemData.dragIcon = _icon;
                  itemData.weaponTarget = Math.min(i + 1,3);
                  trace("control_add_ammo:",inv.name,"ll::",inv.unlockLevelLabel,inv.itemId,itemData.canBuy,"st:",slot_target,"count:",count,inv.value,"compare:",isQuickslot,inv.dictId,slot_target,itemData.weaponTarget,"cl:",itemData.clipsInBag,itemData.count,itemData.premiumCost,auto_buy);
                  this._compassionItems.push(itemData);
                  return;
               }
            }
         }
      }
   }
}

