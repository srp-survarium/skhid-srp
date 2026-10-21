package survarium.upgrade
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.utils.Dictionary;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.DragSlot;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.DragEvent;
   import scaleform.gfx.MouseEventEx;
   import survarium.controls.InventoryList;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.constants.CosmeticSlot;
   import survarium.constants.ItemCategory;
   import survarium.constants.ItemDictId;
   
   public class UpgradeWindow extends UIComponent
   {
      
      protected static var _itemsAgreements:Dictionary = new Dictionary();
      
      public static var scopePairs:Dictionary = new Dictionary();
      
      public static var stockScopes:Dictionary = new Dictionary();
      
      public static var instance:UpgradeWindow;
      
      public static var hostDictId:int = -1;
      
      public static var textSizes:Dictionary = new Dictionary();
      
      public var item_compare:*;
      
      public var infoDescr:TextField;
      
      public var btnAccept:Button;
      
      public var btnCancel:Button;
      
      public var acceptDecalPosition:Button;
      
      public var cancelDecalPosition:Button;
      
      public var decalControl:MovieClip;
      
      public var container:MovieClip;
      
      public var infoContainer:MovieClip;
      
      public var dropSlots:MovieClip;
      
      public var decalSlot1:UpgradeSlot;
      
      public var decalSlot2:UpgradeSlot;
      
      public var decalSlot:UpgradeSlot;
      
      public var camoSlot:UpgradeSlot;
      
      protected var _slotAgreements:Object = {
         3:[ItemCategory.CAMOUFLAGES],
         0:[ItemCategory.DECALS],
         1:[ItemCategory.DECALS],
         2:[ItemCategory.DECALS]
      };
      
      protected var _upgrades:Array;
      
      protected var _data:ItemData;
      
      protected var _slots:Array = new Array();
      
      protected var _inventory:InventoryList;
      
      protected var _iconContainerBig:IconContainer;
      
      protected var _selectedItem:DragSlot;
      
      protected var _hasChanges:Boolean = false;
      
      protected var editData:Object = null;
      
      protected var _camoSlotY:Number;
      
      public function UpgradeWindow()
      {
         super();
      }
      
      public static function addCompatibility(weaponDictId:int, scopeDictId:int) : Boolean
      {
         var scopes:Array = null;
         if(scopeDictId >= ItemDictId.SHOP_SCOPE_FIRST && scopeDictId <= ItemDictId.SHOP_SCOPE_LAST)
         {
            scopes = scopePairs[weaponDictId] as Array;
            if(!scopes)
            {
               scopes = new Array();
               scopePairs[weaponDictId] = scopes;
            }
            if(scopes.indexOf(scopeDictId) == -1)
            {
               scopes.push(scopeDictId);
            }
            return true;
         }
         if(scopeDictId == ItemDictId.SCOPE_LEUPOLD || scopeDictId == ItemDictId.SCOPE_PSO_1 || scopeDictId == ItemDictId.SCOPE_GIPERON || scopeDictId == ItemDictId.SCOPE_EOTECH_552 || scopeDictId == ItemDictId.SCOPE_WALTHER_COMP2)
         {
            stockScopes[weaponDictId] = scopeDictId;
            return true;
         }
         return false;
      }
      
      public static function isWeaponCategory(category:int) : Boolean
      {
         return category == ItemCategory.ASSAULT_RIFLES || category == ItemCategory.SNIPER_RIFLES || category == ItemCategory.SHOTGUNS || category == ItemCategory.SUBMACHINE_GUNS || category == ItemCategory.MACHINE_GUNS || category == ItemCategory.PISTOLS;
      }
      
      public static function hasScopes(weaponDictId:int) : Boolean
      {
         return Boolean(scopePairs[weaponDictId]);
      }
      
      public static function fits(weaponDictId:int, scopeDictId:int) : Boolean
      {
         var scopes:Array = scopePairs[weaponDictId] as Array;
         return Boolean(scopes) && scopes.indexOf(scopeDictId) != -1;
      }
      
      public static function weaponsFor(scopeDictId:int) : Array
      {
         var weaponKey:String = null;
         var weapons:Array = new Array();
         for(weaponKey in scopePairs)
         {
            if((scopePairs[weaponKey] as Array).indexOf(scopeDictId) != -1)
            {
               weapons.push(int(weaponKey));
            }
         }
         weapons.sort(Array.NUMERIC);
         return weapons;
      }
      
      public static function greyed(item:Object) : Boolean
      {
         if(!instance || !instance.visible || hostDictId < 0 || !item || item.category != ItemCategory.SCOPES)
         {
            return false;
         }
         return !fits(hostDictId,int(item.dictId));
      }
      
      public static function inventoryItem(itemId:uint) : Object
      {
         if(!instance || !instance._inventory)
         {
            return null;
         }
         return instance._inventory.getInventoryItem(itemId);
      }
      
      public static function wearers(item:Object) : Array
      {
         var wearer:Object = null;
         if(!item || !item.slotTarget)
         {
            return null;
         }
         wearer = inventoryItem(uint(item.slotTarget));
         return Boolean(wearer) ? [wearer.name] : null;
      }
      
      public static function attachedScope(item:Object) : Object
      {
         var upgrade:Object = null;
         var scope:Object = null;
         if(!item || !item.upgrades)
         {
            return null;
         }
         var upgrades:Array = item.upgrades as Array;
         if(!upgrades)
         {
            return null;
         }
         var i:int = 0;
         while(i < upgrades.length)
         {
            upgrade = upgrades[i];
            if(Boolean(upgrade) && upgrade.param == CosmeticSlot.CAMOUFLAGE)
            {
               scope = inventoryItem(uint(upgrade.id));
               if(Boolean(scope) && scope.category == ItemCategory.SCOPES)
               {
                  return scope;
               }
            }
            i++;
         }
         return null;
      }
      
      public static function fitText(field:TextField, newText:String, maxWidth:Number) : void
      {
         var format:TextFormat = field.defaultTextFormat;
         if(!(field in textSizes))
         {
            textSizes[field] = int(format.size);
         }
         var fontSize:int = int(textSizes[field]);
         format.size = fontSize;
         field.defaultTextFormat = format;
         field.text = newText;
         while((field.textWidth > maxWidth || field.numLines > 1) && fontSize > 9)
         {
            fontSize--;
            format.size = fontSize;
            field.defaultTextFormat = format;
            field.text = newText;
         }
      }
      
      public static function getPair(firstDictId:int, secondDictId:int) : Boolean
      {
         var isPair:Boolean;
         if(_itemsAgreements[firstDictId] as Array && (_itemsAgreements[firstDictId] as Array).indexOf(secondDictId) != -1)
         {
            return true;
         }
         if(_itemsAgreements[secondDictId] as Array && (_itemsAgreements[secondDictId] as Array).indexOf(firstDictId) != -1)
         {
            return true;
         }
         return false;
      }
      
      override public function set x(value:Number) : void
      {
         super.x = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(value:Number) : void
      {
         super.y = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      public function setInventory(inventory:InventoryList) : void
      {
         _inventory = inventory;
         btnCancel = this.container.btnCancel;
         btnAccept = this.btnCancel;
         this.btnAccept.addEventListener(ButtonEvent.CLICK,this.acceptChanges);
         LocalizationData.addButton(this.btnAccept,"confirm");
      }
      
      public function onDrag(event:DragEvent) : void
      {
         trace("UW::onDrag",event);
         if(event == null || event.dragData == null)
         {
            return;
         }
         if(event.type == "dragStart")
         {
            this.selectAgeements(event.dragData.category,event.dragData.dictId);
         }
         else
         {
            this.deselectAgeements();
         }
      }
      
      public function exitDecalEdit() : void
      {
         trace("exitDecalEdit");
         this.lockControls(false);
      }
      
      public function setItemData(itemData:Object) : void
      {
      }
      
      public function getItemId() : uint
      {
         return this._data ? this._data.itemId : 0;
      }
      
      public function setData(itemData:ItemData, resetSlots:Boolean = true) : void
      {
         this._data = itemData;
         this._hasChanges = false;
         this.container.x = parent["inventory_list"].x - x - 190;
         this.container.y = parent["inventory_list"].y - y + 40;
         this.decalControl.x = (MainScene.stageWidth - this.decalControl.width - this.container.x + 320) / 2;
         this.decalControl.y = 160;
         this.infoContainer.x = (MainScene.stageWidth - this.infoContainer.width - this.container.x + 320) / 2;
         this.infoContainer.y = MainScene.stageHeight - this.infoContainer.height - 100;
         this.infoContainer.visible = true;
         hostDictId = Boolean(itemData) && isWeaponCategory(int(itemData.category)) ? int(itemData.dictId) : -1;
         if(hostDictId >= 0 && Boolean(this._inventory))
         {
            this._inventory.selectCategory([ItemCategory.SCOPES],false,true);
         }
         this.updateData(resetSlots);
      }
      
      private function updateData(resetSlots:Boolean = true) : void
      {
         var upgradeCount:int = 0;
         var i:int = 0;
         var slotCount:int = 0;
         var j:uint = 0;
         var slot:UpgradeSlot = null;
         var slotData:Object = null;
         if(Boolean(this._data))
         {
            trace("UW:upgrade_item_selected::",this._data.itemId,"up",this._data.upgrades.length,resetSlots,"decalSlots::",this._data.decalAvailable,"camoSlots::",this._data.camoAvailable);
            this.decalSlot.visible = this._data.decalAvailable > 0;
            this.decalSlot1.visible = this._data.decalAvailable > 1;
            this.decalSlot2.visible = this._data.decalAvailable > 2;
            this.camoSlot.visible = this._data.camoAvailable > 0;
            if(hostDictId >= 0)
            {
               this.decalSlot.visible = false;
               this.decalSlot1.visible = false;
               this.decalSlot2.visible = false;
               this.camoSlot.visible = true;
               if(isNaN(this._camoSlotY))
               {
                  this._camoSlotY = this.camoSlot.y;
               }
               this.camoSlot.y = this.decalSlot.y;
               this.camoSlot.label = Boolean(stockScopes[hostDictId]) ? LocalizationData.getName("default_scope") : LocalizationData.getName("scope_slot");
               this.container.textField.text = LocalizationData.getName("scope_window_title");
               this.infoContainer.infoDescr.text = LocalizationData.getName("scope_screen_hint");
               this.infoContainer.background.height = this.infoContainer.infoDescr.textHeight + 24;
               this.infoContainer.y = MainScene.stageHeight - this.infoContainer.height - 100;
            }
            else if(!isNaN(this._camoSlotY))
            {
               this.camoSlot.y = this._camoSlotY;
               this.container.textField.text = LocalizationData.getName("btn_upgrade_label");
               this.onLocal(null);
               this.infoContainer.y = MainScene.stageHeight - this.infoContainer.height - 100;
            }
            if(resetSlots || this._data.upgrades.length == 0)
            {
               this.cleanSlots();
            }
            if(Boolean(this._data.upgrades) && resetSlots)
            {
               upgradeCount = int(this._data.upgrades.length);
               i = 0;
               while(i < upgradeCount)
               {
                  slotCount = int(this._slots.length);
                  j = 0;
                  while(j < slotCount)
                  {
                     slot = this._slots[j];
                     trace("UW::slotId::",slot.visible,slot.index,this._data.upgrades[i].id,this._data.upgrades[i].param);
                     if(slot.visible && slot.index == this._data.upgrades[i].param || slot.index == CosmeticSlot.CAMOUFLAGE && this._data.upgrades[i].param == undefined)
                     {
                        if(!slot.data || slot.data.itemId != this._data.upgrades[i].id)
                        {
                           slotData = this.create_item(this._data.upgrades[i].id);
                           slot.data = slotData;
                           if(resetSlots)
                           {
                              slot.startData = slotData;
                           }
                        }
                        break;
                     }
                     j = ++j;
                  }
                  i++;
               }
            }
         }
         else
         {
            this.decalSlot2.visible = this.decalSlot1.visible = this.decalSlot.visible = this.camoSlot.visible = false;
            this.decalSlot2.data = this.decalSlot1.data = this.decalSlot.data = this.camoSlot.data = null;
         }
      }
      
      protected function cleanSlots() : void
      {
         var slotCount:int = (this._slots.length);
         var i:uint = 0;
         var slot:UpgradeSlot;
         while(i < slotCount)
         {
            slot = this._slots[i];
            slot.data = null;
            slot.startData = null;
            i++;
         }
      }
      
      public function setItemsCompatibility(pairs:Array) : void
      {
         var i:int = 0;
         var pair:Object;
         while(i < pairs.length)
         {
            pair = pairs[i];
            if(!_itemsAgreements[pair.first_item_dict_id])
            {
               _itemsAgreements[pair.first_item_dict_id] = new Array();
            }
            _itemsAgreements[pair.first_item_dict_id].push(pair.second_item_dict_id);
            i++;
         }
      }
      
      public function setSlotsRestrictions(restrictions:Array) : void
      {
         var i:int = 0;
         var restriction:Object;
         while(i < restrictions.length)
         {
            restriction = restrictions[i];
            if(!this._slotAgreements[restriction.slot_id])
            {
               this._slotAgreements[restriction.slot_id] = new Array();
            }
            this._slotAgreements[restriction.slot_id].push(restriction.category_id);
            i++;
         }
      }
      
      public function selectAgeements(category:int, dictId:int = 0) : void
      {
         trace("UW::selectAgeements",category,dictId,this._slots);
         if(hostDictId >= 0)
         {
            this.decalSlot.highlighted = false;
            this.decalSlot1.highlighted = false;
            this.decalSlot2.highlighted = false;
            this.camoSlot.highlighted = category == ItemCategory.SCOPES && fits(hostDictId,dictId) && Boolean(this.camoSlot.enabled);
            return;
         }
         var i:int = 0;
         while(i < this._slots.length)
         {
            trace("iterate::",this._slots[i].index,this._slotAgreements[this._slots[i].index],this._slots[i].enabled);
            if(Boolean(this._slotAgreements[this._slots[i].index]) && Boolean((this._slotAgreements[this._slots[i].index] as Array).indexOf(category) != -1) && Boolean(this._slots[i].enabled))
            {
               trace("highlighted true");
               this._slots[i].highlighted = true;
            }
            else
            {
               trace("highlighted false");
               this._slots[i].highlighted = false;
            }
            i++;
         }
      }
      
      public function deselectAgeements() : void
      {
         trace("UW::deselectAgeements");
         var i:int = 0;
         while(i < this._slots.length)
         {
            if(this._slots[i].enabled || this._slots[i].highlighted == true)
            {
               trace("highlighted false");
               this._slots[i].highlighted = false;
            }
            i++;
         }
      }
      
      public function closePopup() : void
      {
         trace("closeUpgadePopup",this.decalControl.visible,visible);
         hostDictId = -1;
         if(this.decalControl.visible)
         {
            this.onCancelDecalPosition(null);
         }
         else
         {
            dispatchEvent(new Event(Event.CLOSE));
         }
         this.infoContainer.visible = false;
         trace("infoDescr::hide");
      }
      
      public function acceptChangesItem(item:Object = null) : void
      {
         this.acceptChanges();
      }
      
      protected function create_item(itemId:uint) : Object
      {
         var item:Object = this._inventory.getInventoryItem(itemId);
         if(!item)
         {
            return null;
         }
         trace("UW::create_item",item.iconId);
         var iconContainer:IconContainer = item.category == ItemCategory.DECALS ? IconContainer.getInstance("decals") : (item.category == ItemCategory.SCOPES ? IconContainer.getInstance("consumables") : this._iconContainerBig);
         var icon:Icon = iconContainer.getIcon(item.iconId);
         if(item.category == ItemCategory.SCOPES)
         {
            icon.scaleX = 0.55;
            icon.scaleY = 0.55;
         }
         var dragIcon:Icon = (item.dragIcon as Icon).clone();
         return {
            "itemId":item.itemId,
            "name":item.name,
            "icon":icon,
            "dragIcon":dragIcon,
            "attachedId":item.slotTarget,
            "category":item.category
         };
      }
      
      protected function configDecalSlot(slot:UpgradeSlot) : void
      {
         slot.contentCanvas.scaleX = slot.contentCanvas.scaleY = 1.1;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.btnCancel = this.container.btnCancel;
         this.decalSlot2 = this.container.decalSlot2;
         this.decalSlot1 = this.container.decalSlot1;
         this.decalSlot = this.container.decalSlot;
         this.camoSlot = this.container.camoSlot;
         LocalizationData.addTextField(this.container.textField,"btn_upgrade_label");
         this.infoContainer.visible = false;
         this.btnCancel.enabled = true;
         this.decalSlot.index = CosmeticSlot.DECAL_1;
         this.decalSlot1.index = CosmeticSlot.DECAL_2;
         this.decalSlot2.index = CosmeticSlot.DECAL_3;
         this.camoSlot.index = CosmeticSlot.CAMOUFLAGE;
         this.configDecalSlot(this.decalSlot);
         this.configDecalSlot(this.decalSlot1);
         this.configDecalSlot(this.decalSlot2);
         this.dropSlots.visible = false;
         this._iconContainerBig = IconContainer.getInstance("camo_big");
         LocalizationData.addCallback(this.onLocal,"accept");
         this.camoSlot.setTransitionSize(95,105);
         this.decalSlot.setTransitionSize(95,105);
         this.decalSlot1.setTransitionSize(95,105);
         this.decalSlot2.setTransitionSize(95,105);
         this._slots.push(this.decalSlot,this.decalSlot1,this.decalSlot2,this.camoSlot);
         addEventListener(MouseEvent.CLICK,this.onMouseItemClick);
         var i:uint = 0;
         while(i < this._slots.length)
         {
            this._slots[i].addEventListener(DragEvent.DROP,this.onItemDropped);
            this._slots[i].addEventListener(UpgradeSlot.UPDATED,this.onItemUpdated);
            this._slots[i].addEventListener(ButtonEvent.CLICK,this.onItemClicked);
            this._slots[i].addEventListener(MouseEvent.CLICK,this.onItemMouseClicked);
            this._slots[i].addEventListener(MouseEvent.MOUSE_OVER,this.onItemOver);
            this._slots[i].addEventListener(MouseEvent.MOUSE_OUT,this.onItemOut);
            i = ++i;
         }
         if(Boolean(this.decalControl.cancelDecalPosition))
         {
            this.cancelDecalPosition = this.decalControl.cancelDecalPosition;
            this.cancelDecalPosition.visible = false;
            this.cancelDecalPosition.addEventListener(ButtonEvent.CLICK,this.onCancelDecalPosition);
            LocalizationData.addButton(this.cancelDecalPosition,"btn_cancel_decal_edit");
         }
         this.acceptDecalPosition = this.decalControl.acceptDecalPosition;
         trace("configUU",parent,this.acceptDecalPosition);
         if(Boolean(parent))
         {
            parent.addChild(this.decalControl);
            parent.addChild(this.infoContainer);
            this.infoContainer.mouseEnabled = false;
         }
         this.decalControl.visible = false;
         this.infoContainer.visible = false;
         LocalizationData.addTextField(this.decalControl.decalLabel,"decal_editor_label");
         LocalizationData.addTextField(this.decalControl.decalDescr,"decal_editor_description");
         LocalizationData.addButton(this.acceptDecalPosition,"btn_accept_decal_edit");
         this.acceptDecalPosition.visible = false;
         instance = this;
      }
      
      private function onItemMouseClicked(event:MouseEvent) : void
      {
         var category:int = 0;
         trace("onItemMouseClicked",event.target,event.currentTarget);
         var slot:UpgradeSlot = event.currentTarget as UpgradeSlot;
         if(Boolean(slot) && !slot.data)
         {
            category = hostDictId >= 0 ? ItemCategory.SCOPES : int(this._slotAgreements[slot.index]);
            this._inventory.selectCategory([category]);
         }
      }
      
      private function setScaleMode() : void
      {
      }
      
      private function onCancelDecalPosition(event:ButtonEvent) : void
      {
         if(!this.editData)
         {
            return;
         }
         trace("cancel_decal_edit",this._data.itemId,this.editData.itemId,event);
         ExternalInterface.call("cancel_decal_edit",this._data.itemId,this.editData.itemId);
         editData = null;
         this.lockControls(false);
      }
      
      private function onAcceptDecalPosition(event:ButtonEvent) : void
      {
         if(!this.editData)
         {
            return;
         }
         trace("accept_decal_edit",this._data.itemId,this.editData.itemId);
         ExternalInterface.call("accept_decal_edit",this._data.itemId,this.editData.itemId);
         editData = null;
         this.lockControls(false);
      }
      
      protected function lockControls(locked:Boolean, activeSlot:Object = null) : void
      {
         this.btnAccept.enabled = !locked;
         this.btnCancel.enabled = !locked;
         this.decalControl.visible = locked && visible;
         this.btnCancel.visible = !this.decalControl.visible;
         trace("infoDescr::lock");
         this.infoContainer.visible = !locked && visible;
         mouseChildren = !locked;
         mouseEnabled = !locked;
         var i:uint = 0;
         while(i < this._slots.length)
         {
            if(locked)
            {
               if(this._slots[i].btnChange)
               {
                  this._slots[i].btnChange.visible = false;
               }
               if(this._slots[i].btnDrop)
               {
                  this._slots[i].btnDrop.visible = false;
               }
               trace("lock::",this._slots[i],activeSlot);
               if(this._slots[i] != activeSlot)
               {
                  this._slots[i].enabled = false;
                  this._slots[i].alpha = 0.6;
               }
               else
               {
                  this._slots[i].alpha = 1;
                  if(this._slots[i].selectedBorder)
                  {
                     this._slots[i].selectedBorder.visible = true;
                  }
                  if(this._slots[i].border && this._slots[i].border.inner)
                  {
                     this._slots[i].border.inner.visible = true;
                  }
               }
            }
            else
            {
               if(this._slots[i].border && this._slots[i].border.inner)
               {
                  this._slots[i].border.inner.visible = false;
               }
               if(this._slots[i].btnChange && this._slots[i].data)
               {
                  this._slots[i].btnChange.visible = true;
               }
               if(this._slots[i].btnDrop && this._slots[i].data)
               {
                  this._slots[i].btnDrop.visible = true;
               }
               this._slots[i].enabled = true;
               this._slots[i].alpha = 1;
               if(this._slots[i].selectedBorder)
               {
                  this._slots[i].selectedBorder.visible = false;
               }
            }
            i++;
         }
      }
      
      private function onItemClicked(event:ButtonEvent) : void
      {
         stackTrace("onItemClciked");
         var slotData:Object = event.currentTarget.data;
         trace("onItemClicked:;",slotData ? slotData.category : null,event.target,event.currentTarget);
         if(slotData && slotData.category == ItemCategory.DECALS)
         {
            if(event.target.name == "btnDrop")
            {
               return;
            }
            if(event.target.name == "btnChange" && !this.decalControl.visible)
            {
               editData = slotData;
               trace("enter_decal_editor",this._data.itemId,slotData.itemId);
               this.lockControls(true,event.currentTarget);
               ExternalInterface.call("start_decal_edit",this._data.itemId,slotData.itemId);
            }
         }
      }
      
      protected function onLocal(value:String) : void
      {
         this.decalSlot.label = LocalizationData.getName("decal_slot");
         this.decalSlot1.label = LocalizationData.getName("decal_slot");
         this.decalSlot2.label = LocalizationData.getName("decal_slot");
         this.camoSlot.label = LocalizationData.getName("camo_slot");
         this.infoContainer.infoDescr.text = LocalizationData.getName("modification_screen_hint") + "\n" + LocalizationData.getName("modification_screen_edit_decal_hint");
         this.infoContainer.background.height = this.infoContainer.infoDescr.textHeight + 24;
      }
      
      private function onMouseItemClick(event:MouseEventEx) : void
      {
         if(event.buttonIdx == MouseEventEx.LEFT_BUTTON)
         {
            return;
         }
         var slot:DragSlot = event.target as DragSlot;
         trace("onMouseItemClick:::",event.target,event.currentTarget,slot);
         if(!slot || slot.enabled == false || slot.dragEnabled == false || slot.data == null)
         {
            return;
         }
         _selectedItem = slot;
      }
      
      private function dropElement() : void
      {
         this._selectedItem.data = null;
      }
      
      private function onItemOut(event:Event) : void
      {
      }
      
      private function onItemOver(event:Event) : void
      {
         var slot:UpgradeSlot = event.target as UpgradeSlot;
         if(slot && slot.data && slot.data.modifiers)
         {
         }
      }
      
      private function onItemUpdated(event:Event) : void
      {
         _upgrades = [];
         _hasChanges = true;
         trace("UW:onItemUpdated",event.target,event.currentTarget);
         var slot:UpgradeSlot = event.target as UpgradeSlot;
         var lastData:Object = slot.getLastData();
         trace("updateData::",slot.data,lastData);
         if(lastData && !slot.data)
         {
            trace("drop_upgrade_item::updated",this._data.itemId,lastData.itemId);
            ExternalInterface.call("drop_upgrade_item",this._data.itemId,uint(lastData.itemId));
         }
      }
      
      private function onItemDropped(event:DragEvent) : void
      {
         var i:uint;
         var item:Object;
         var slot:UpgradeSlot;
         trace("onItemDropped::",event.dragData);
         if(event.dragData)
         {
            i = 0;
            while(i < this._slots.length)
            {
               slot = this._slots[i];
               trace("slotId::",event.dragData.itemId,slot.data ? slot.data.itemId : "ff");
               if(slot.data && slot.data.itemId == event.dragData.itemId)
               {
                  slot.dropSlot();
               }
               i++;
            }
            slot = event.target as UpgradeSlot;
            trace("updateData::");
            item = this.create_item(event.dragData.itemId);
            slot.setData(item);
            trace("relocate_upgrade_item",this._data.itemId,uint(slot.data.itemId),uint(slot.index));
            ExternalInterface.call("relocate_upgrade_item",this._data.itemId,uint(slot.data.itemId),uint(slot.index));
         }
      }
      
      private function completeRelocate(result:Object) : void
      {
         var dragEvent:DragEvent = result.dragEvent;
         var slot:UpgradeSlot = dragEvent.target as UpgradeSlot;
         trace("updateData::");
         slot.setData(this.create_item(dragEvent.dragData.itemId));
      }
      
      private function acceptChanges(event:ButtonEvent = null) : void
      {
         _hasChanges = false;
         this.closePopup();
      }
      
      private function cancelChanges(event:ButtonEvent = null) : void
      {
         _hasChanges = false;
         this.closePopup();
         var i:uint = 0;
         var slot:UpgradeSlot;
         var slotData:Object;
         var startData:Object;
         while(i < this._slots.length)
         {
            slot = this._slots[i];
            slotData = slot.data;
            startData = slot.startData;
            trace("cancelChanges::",slotData,startData);
            if(startData)
            {
               if(slotData)
               {
                  trace("slotData.itemId",slotData.itemId,startData.itemId);
                  if(slotData.itemId != startData.itemId)
                  {
                     trace("cancel:drop_upgrade_item",this._data.itemId,uint(slotData.itemId));
                     ExternalInterface.call("drop_upgrade_item",this._data.itemId,uint(slotData.itemId));
                     trace("cancel:relocate_upgrade_item",this._data.itemId,uint(startData.itemId));
                     ExternalInterface.call("relocate_upgrade_item",this._data.itemId,uint(startData.itemId),uint(slot.index));
                  }
               }
               else
               {
                  trace("cancel:drop_upgrade_item",this._data.itemId,uint(slotData.itemId));
                  ExternalInterface.call("drop_upgrade_item",this._data.itemId,uint(slotData.itemId));
               }
            }
            else if(slotData)
            {
               trace("cancel:drop_upgrade_item",this._data.itemId,uint(slotData.itemId));
               ExternalInterface.call("drop_upgrade_item",this._data.itemId,uint(slotData.itemId));
            }
            i++;
         }
      }
   }
}

