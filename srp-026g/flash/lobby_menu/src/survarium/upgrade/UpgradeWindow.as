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
         3:[25],
         0:[26],
         1:[26],
         2:[26]
      };
      
      protected var _upgrades:Array;
      
      protected var _data:ItemData;
      
      protected var _slots:Array = new Array();
      
      protected var _inventory:InventoryList;
      
      protected var _iconContainerBig:IconContainer;
      
      protected var _selectedItem:DragSlot;
      
      protected var _hasChanges:Boolean = false;
      
      protected var editData:Object = null;
      
      // each slot's x, y and scale as the movie places them (armour's window)
      protected var _slotPlaces:Array;

      public function UpgradeWindow()
      {
         super();
      }
      
      public static function addCompatibility(param1:int, param2:int) : Boolean
      {
         var _loc3_:Array = null;
         if(param2 >= 2101 && param2 <= 2199)
         {
            _loc3_ = scopePairs[param1] as Array;
            if(!_loc3_)
            {
               _loc3_ = new Array();
               scopePairs[param1] = _loc3_;
            }
            if(_loc3_.indexOf(param2) == -1)
            {
               _loc3_.push(param2);
            }
            return true;
         }
         if(param2 == 69 || param2 == 304 || param2 == 362 || param2 == 2206 || param2 == 2207)
         {
            stockScopes[param1] = param2;
            return true;
         }
         return false;
      }
      
      public static function isWeaponCategory(param1:int) : Boolean
      {
         return param1 == 8 || param1 == 13 || param1 == 14 || param1 == 15 || param1 == 16 || param1 == 17;
      }
      
      public static function hasScopes(param1:int) : Boolean
      {
         return Boolean(scopePairs[param1]);
      }

      public static function isUpgradeableWeapon(item:Object) : Boolean
      {
         return isWeaponCategory(int(item.category)) && (hasScopes(int(item.dictId)) || Boolean(item.camoAvailable) || Boolean(item.decalAvailable));
      }
      
      public static function fits(param1:int, param2:int) : Boolean
      {
         var _loc3_:Array = scopePairs[param1] as Array;
         return Boolean(_loc3_) && _loc3_.indexOf(param2) != -1;
      }
      
      public static function weaponsFor(param1:int) : Array
      {
         var _loc3_:String = null;
         var _loc2_:Array = new Array();
         for(_loc3_ in scopePairs)
         {
            if((scopePairs[_loc3_] as Array).indexOf(param1) != -1)
            {
               _loc2_.push(int(_loc3_));
            }
         }
         _loc2_.sort(Array.NUMERIC);
         return _loc2_;
      }
      
      public static function greyed(param1:Object) : Boolean
      {
         if(!instance || !instance.visible || hostDictId < 0 || !param1 || param1.category != 22)
         {
            return false;
         }
         return !fits(hostDictId,int(param1.dictId));
      }
      
      public static function inventoryItem(param1:uint) : Object
      {
         if(!instance || !instance._inventory)
         {
            return null;
         }
         return instance._inventory.getInventoryItem(param1);
      }
      
      public static function wearers(param1:Object) : Array
      {
         var _loc2_:Object = null;
         if(!param1 || !param1.slotTarget)
         {
            return null;
         }
         _loc2_ = inventoryItem(uint(param1.slotTarget));
         return Boolean(_loc2_) ? [_loc2_.name] : null;
      }
      
      public static function attachedScope(param1:Object) : Object
      {
         var _loc4_:Object = null;
         var _loc5_:Object = null;
         if(!param1 || !param1.upgrades)
         {
            return null;
         }
         var _loc2_:Array = param1.upgrades as Array;
         if(!_loc2_)
         {
            return null;
         }
         var _loc3_:int = 0;
         while(_loc3_ < _loc2_.length)
         {
            _loc4_ = _loc2_[_loc3_];
            if(Boolean(_loc4_) && _loc4_.param == 3)
            {
               _loc5_ = inventoryItem(uint(_loc4_.id));
               if(Boolean(_loc5_) && _loc5_.category == 22)
               {
                  return _loc5_;
               }
            }
            _loc3_++;
         }
         return null;
      }
      
      public static function fitText(param1:TextField, param2:String, param3:Number) : void
      {
         var _loc4_:TextFormat = param1.defaultTextFormat;
         if(!(param1 in textSizes))
         {
            textSizes[param1] = int(_loc4_.size);
         }
         var _loc5_:int = int(textSizes[param1]);
         _loc4_.size = _loc5_;
         param1.defaultTextFormat = _loc4_;
         param1.text = param2;
         while((param1.textWidth > param3 || param1.numLines > 1) && _loc5_ > 9)
         {
            _loc5_--;
            _loc4_.size = _loc5_;
            param1.defaultTextFormat = _loc4_;
            param1.text = param2;
         }
      }
      
      public static function getPair(param1:int, param2:int) : Boolean
      {
         var _loc3_:Boolean;
         if(_itemsAgreements[param1] as Array && (_itemsAgreements[param1] as Array).indexOf(param2) != -1)
         {
            return true;
         }
         if(_itemsAgreements[param2] as Array && (_itemsAgreements[param2] as Array).indexOf(param1) != -1)
         {
            return true;
         }
         return false;
      }
      
      override public function set x(param1:Number) : void
      {
         super.x = int(param1 * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(param1:Number) : void
      {
         super.y = int(param1 * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      public function setInventory(param1:InventoryList) : void
      {
         _inventory = param1;
         btnCancel = this.container.btnCancel;
         btnAccept = this.btnCancel;
         this.btnAccept.addEventListener(ButtonEvent.CLICK,this.acceptChanges);
         LocalizationData.addButton(this.btnAccept,"confirm");
      }
      
      public function onDrag(param1:DragEvent) : void
      {
         trace("UW::onDrag",param1);
         if(param1 == null || param1.dragData == null)
         {
            return;
         }
         if(param1.type == "dragStart")
         {
            this.selectAgeements(param1.dragData.category,param1.dragData.dictId);
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
      
      public function setItemData(param1:Object) : void
      {
      }
      
      public function getItemId() : uint
      {
         return this._data ? this._data.itemId : 0;
      }
      
      public function setData(param1:ItemData, param2:Boolean = true) : void
      {
         this._data = param1;
         this._hasChanges = false;
         this.container.x = parent["inventory_list"].x - x - 190;
         this.container.y = parent["inventory_list"].y - y + 40;
         this.decalControl.x = (MainScene.stageWidth - this.decalControl.width - this.container.x + 320) / 2;
         this.decalControl.y = 160;
         this.infoContainer.x = (MainScene.stageWidth - this.infoContainer.width - this.container.x + 320) / 2;
         this.infoContainer.y = MainScene.stageHeight - this.infoContainer.height - 100;
         this.infoContainer.visible = true;
         hostDictId = Boolean(param1) && isWeaponCategory(int(param1.category)) ? int(param1.dictId) : -1;
         this.updateData(param2);
      }
      
      private function updateData(param1:Boolean = true) : void
      {
         var _loc2_:int = 0;
         var _loc3_:int = 0;
         var _loc4_:int = 0;
         var _loc5_:uint = 0;
         var _loc6_:UpgradeSlot = null;
         var _loc7_:Object = null;
         if(Boolean(this._data))
         {
            trace("UW:upgrade_item_selected::",this._data.itemId,"up",this._data.upgrades.length,param1,"decalSlots::",this._data.decalAvailable,"camoSlots::",this._data.camoAvailable);
            this.decalSlot.visible = this._data.decalAvailable > 0;
            this.decalSlot1.visible = this._data.decalAvailable > 1;
            this.decalSlot2.visible = this._data.decalAvailable > 2;
            this.camoSlot.visible = this._data.camoAvailable > 0;
            if(hostDictId >= 0)
            {
               // a weapon: the scope in slot 3, its camouflage in slot 2 (decalSlot2), its two decals in
               // slots 0 and 1, in that order from the top
               this.decalSlot.visible = this._data.decalAvailable > 0;
               this.decalSlot1.visible = this._data.decalAvailable > 1;
               this.decalSlot2.visible = this._data.camoAvailable > 0;
               this.camoSlot.visible = hasScopes(hostDictId);
               this.placeWeaponSlots();
               this.camoSlot.label = Boolean(stockScopes[hostDictId]) ? LocalizationData.getName("default_scope") : LocalizationData.getName("scope_slot");
               this.decalSlot2.label = LocalizationData.getName("camo_slot");
               this.decalSlot2.contentCanvas.scaleX = this.decalSlot2.contentCanvas.scaleY = 1;
               this.container.textField.text = LocalizationData.getName("scope_window_title");
               this.infoContainer.infoDescr.text = LocalizationData.getName("scope_screen_hint") + (this._data.decalAvailable > 0 ? "\n" + LocalizationData.getName("modification_screen_edit_decal_hint") : "");
               this.infoContainer.background.height = this.infoContainer.infoDescr.textHeight + 24;
               this.infoContainer.y = MainScene.stageHeight - this.infoContainer.height - 100;
            }
            else if(Boolean(this._slotPlaces))
            {
               this.restoreSlots();
               this.configDecalSlot(this.decalSlot2);
               this.container.textField.text = LocalizationData.getName("btn_upgrade_label");
               this.onLocal(null);
               this.infoContainer.y = MainScene.stageHeight - this.infoContainer.height - 100;
            }
            if(param1 || this._data.upgrades.length == 0)
            {
               this.cleanSlots();
            }
            if(Boolean(this._data.upgrades) && param1)
            {
               _loc2_ = int(this._data.upgrades.length);
               _loc3_ = 0;
               while(_loc3_ < _loc2_)
               {
                  _loc4_ = int(this._slots.length);
                  _loc5_ = 0;
                  while(_loc5_ < _loc4_)
                  {
                     _loc6_ = this._slots[_loc5_];
                     trace("UW::slotId::",_loc6_.visible,_loc6_.index,this._data.upgrades[_loc3_].id,this._data.upgrades[_loc3_].param);
                     if(_loc6_.visible && _loc6_.index == this._data.upgrades[_loc3_].param || _loc6_.index == 3 && this._data.upgrades[_loc3_].param == undefined)
                     {
                        if(!_loc6_.data || _loc6_.data.itemId != this._data.upgrades[_loc3_].id)
                        {
                           _loc7_ = this.create_item(this._data.upgrades[_loc3_].id);
                           _loc6_.data = _loc7_;
                           if(param1)
                           {
                              _loc6_.startData = _loc7_;
                           }
                        }
                        break;
                     }
                     _loc5_ = ++_loc5_;
                  }
                  _loc3_++;
               }
            }
         }
         else
         {
            this.decalSlot2.visible = this.decalSlot1.visible = this.decalSlot.visible = this.camoSlot.visible = false;
            this.decalSlot2.data = this.decalSlot1.data = this.decalSlot.data = this.camoSlot.data = null;
         }
      }
      
      // A weapon's slots in one column from the top, the scope, the camouflage and the decals, as many as it
      // takes: the armour window's places for three, the column 0.9 as tall for four.
      protected function placeWeaponSlots() : void
      {
         var _loc1_:uint = 0;
         var _loc2_:Array = [this.camoSlot,this.decalSlot2,this.decalSlot,this.decalSlot1];
         var _loc3_:Array = new Array();
         var _loc4_:Number = NaN;
         var _loc5_:UpgradeSlot = null;
         if(!this._slotPlaces)
         {
            this._slotPlaces = new Array();
            while(_loc1_ < this._slots.length)
            {
               _loc5_ = this._slots[_loc1_];
               this._slotPlaces.push([_loc5_.x,_loc5_.y,_loc5_.scaleX,_loc5_.scaleY]);
               _loc1_++;
            }
         }
         _loc1_ = 0;
         while(_loc1_ < _loc2_.length)
         {
            if(UpgradeSlot(_loc2_[_loc1_]).visible)
            {
               _loc3_.push(_loc2_[_loc1_]);
            }
            _loc1_++;
         }
         _loc4_ = _loc3_.length > 3 ? 0.9 : 1;
         _loc1_ = 0;
         while(_loc1_ < _loc3_.length)
         {
            _loc5_ = _loc3_[_loc1_];
            _loc5_.scaleX = _loc5_.scaleY = _loc4_;
            // a slot is 139 x 130 around (35.55, 44.25) of itself; the column's middle at x 96.55, its top at 28
            _loc5_.x = 96.55 - 35.55 * _loc4_;
            _loc5_.y = 28 + 21 * _loc4_ + _loc1_ * (_loc3_.length > 3 ? 123 : 142);
            _loc1_++;
         }
      }

      protected function restoreSlots() : void
      {
         var _loc1_:uint = 0;
         var _loc2_:UpgradeSlot = null;
         while(_loc1_ < this._slots.length)
         {
            _loc2_ = this._slots[_loc1_];
            _loc2_.x = this._slotPlaces[_loc1_][0];
            _loc2_.y = this._slotPlaces[_loc1_][1];
            _loc2_.scaleX = this._slotPlaces[_loc1_][2];
            _loc2_.scaleY = this._slotPlaces[_loc1_][3];
            _loc1_++;
         }
      }

      protected function cleanSlots() : void
      {
         var _loc1_:int = (this._slots.length);
         var _loc2_:uint = 0;
         var _loc3_:UpgradeSlot;
         while(_loc2_ < _loc1_)
         {
            _loc3_ = this._slots[_loc2_];
            _loc3_.data = null;
            _loc3_.startData = null;
            _loc2_++;
         }
      }
      
      public function setItemsCompatibility(param1:Array) : void
      {
         var _loc2_:int = 0;
         var _loc3_:Object;
         while(_loc2_ < param1.length)
         {
            _loc3_ = param1[_loc2_];
            if(!_itemsAgreements[_loc3_.first_item_dict_id])
            {
               _itemsAgreements[_loc3_.first_item_dict_id] = new Array();
            }
            _itemsAgreements[_loc3_.first_item_dict_id].push(_loc3_.second_item_dict_id);
            _loc2_++;
         }
      }
      
      public function setSlotsRestrictions(param1:Array) : void
      {
         var _loc2_:int = 0;
         var _loc3_:Object;
         while(_loc2_ < param1.length)
         {
            _loc3_ = param1[_loc2_];
            if(!this._slotAgreements[_loc3_.slot_id])
            {
               this._slotAgreements[_loc3_.slot_id] = new Array();
            }
            this._slotAgreements[_loc3_.slot_id].push(_loc3_.category_id);
            _loc2_++;
         }
      }
      
      public function selectAgeements(param1:int, param2:int = 0) : void
      {
         trace("UW::selectAgeements",param1,param2,this._slots);
         if(hostDictId >= 0)
         {
            this.decalSlot.highlighted = param1 == 26 && this.decalSlot.visible && Boolean(this.decalSlot.enabled);
            this.decalSlot1.highlighted = param1 == 26 && this.decalSlot1.visible && Boolean(this.decalSlot1.enabled);
            this.decalSlot2.highlighted = param1 == 25 && this.decalSlot2.visible && Boolean(this.decalSlot2.enabled);
            this.camoSlot.highlighted = param1 == 22 && fits(hostDictId,param2) && Boolean(this.camoSlot.enabled);
            return;
         }
         var _loc3_:int = 0;
         while(_loc3_ < this._slots.length)
         {
            trace("iterate::",this._slots[_loc3_].index,this._slotAgreements[this._slots[_loc3_].index],this._slots[_loc3_].enabled);
            if(Boolean(this._slotAgreements[this._slots[_loc3_].index]) && Boolean((this._slotAgreements[this._slots[_loc3_].index] as Array).indexOf(param1) != -1) && Boolean(this._slots[_loc3_].enabled))
            {
               trace("highlighted true");
               this._slots[_loc3_].highlighted = true;
            }
            else
            {
               trace("highlighted false");
               this._slots[_loc3_].highlighted = false;
            }
            _loc3_++;
         }
      }
      
      public function deselectAgeements() : void
      {
         trace("UW::deselectAgeements");
         var _loc1_:int = 0;
         while(_loc1_ < this._slots.length)
         {
            if(this._slots[_loc1_].enabled || this._slots[_loc1_].highlighted == true)
            {
               trace("highlighted false");
               this._slots[_loc1_].highlighted = false;
            }
            _loc1_++;
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
      
      public function acceptChangesItem(param1:Object = null) : void
      {
         this.acceptChanges();
      }
      
      protected function create_item(param1:uint) : Object
      {
         var _loc2_:Object = this._inventory.getInventoryItem(param1);
         if(!_loc2_)
         {
            return null;
         }
         trace("UW::create_item",_loc2_.iconId);
         var _loc3_:IconContainer = _loc2_.category == 26 ? IconContainer.getInstance("decals") : (_loc2_.category == 22 ? IconContainer.getInstance("consumables") : this._iconContainerBig);
         var _loc4_:Icon = _loc3_.getIcon(_loc2_.iconId);
         if(_loc2_.category == 22)
         {
            _loc4_.scaleX = 0.55;
            _loc4_.scaleY = 0.55;
         }
         var _loc5_:Icon = (_loc2_.dragIcon as Icon).clone();
         return {
            "itemId":_loc2_.itemId,
            "name":_loc2_.name,
            "icon":_loc4_,
            "dragIcon":_loc5_,
            "attachedId":_loc2_.slotTarget,
            "category":_loc2_.category
         };
      }
      
      protected function configDecalSlot(param1:UpgradeSlot) : void
      {
         param1.contentCanvas.scaleX = param1.contentCanvas.scaleY = 1.1;
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
         this.decalSlot.index = 0;
         this.decalSlot1.index = 1;
         this.decalSlot2.index = 2;
         this.camoSlot.index = 3;
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
         var _loc1_:uint = 0;
         while(_loc1_ < this._slots.length)
         {
            this._slots[_loc1_].addEventListener(DragEvent.DROP,this.onItemDropped);
            this._slots[_loc1_].addEventListener(UpgradeSlot.UPDATED,this.onItemUpdated);
            this._slots[_loc1_].addEventListener(ButtonEvent.CLICK,this.onItemClicked);
            this._slots[_loc1_].addEventListener(MouseEvent.CLICK,this.onItemMouseClicked);
            this._slots[_loc1_].addEventListener(MouseEvent.MOUSE_OVER,this.onItemOver);
            this._slots[_loc1_].addEventListener(MouseEvent.MOUSE_OUT,this.onItemOut);
            _loc1_ = ++_loc1_;
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
      
      private function onItemMouseClicked(param1:MouseEvent) : void
      {
         var _loc3_:int = 0;
         trace("onItemMouseClicked",param1.target,param1.currentTarget);
         var _loc2_:UpgradeSlot = param1.currentTarget as UpgradeSlot;
         if(Boolean(_loc2_) && !_loc2_.data)
         {
            _loc3_ = hostDictId >= 0 ? (_loc2_ == this.decalSlot2 ? 25 : (_loc2_ == this.camoSlot ? 22 : 26)) : int(this._slotAgreements[_loc2_.index]);
            this._inventory.selectCategory([_loc3_]);
         }
      }
      
      private function setScaleMode() : void
      {
      }
      
      private function onCancelDecalPosition(param1:ButtonEvent) : void
      {
         if(!this.editData)
         {
            return;
         }
         trace("cancel_decal_edit",this._data.itemId,this.editData.itemId,param1);
         ExternalInterface.call("cancel_decal_edit",this._data.itemId,this.editData.itemId);
         editData = null;
         this.lockControls(false);
      }
      
      private function onAcceptDecalPosition(param1:ButtonEvent) : void
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
      
      protected function lockControls(param1:Boolean, param2:Object = null) : void
      {
         this.btnAccept.enabled = !param1;
         this.btnCancel.enabled = !param1;
         this.decalControl.visible = param1 && visible;
         this.btnCancel.visible = !this.decalControl.visible;
         trace("infoDescr::lock");
         this.infoContainer.visible = !param1 && visible;
         mouseChildren = !param1;
         mouseEnabled = !param1;
         var _loc3_:uint = 0;
         while(_loc3_ < this._slots.length)
         {
            if(param1)
            {
               if(this._slots[_loc3_].btnChange)
               {
                  this._slots[_loc3_].btnChange.visible = false;
               }
               if(this._slots[_loc3_].btnDrop)
               {
                  this._slots[_loc3_].btnDrop.visible = false;
               }
               trace("lock::",this._slots[_loc3_],param2);
               if(this._slots[_loc3_] != param2)
               {
                  this._slots[_loc3_].enabled = false;
                  this._slots[_loc3_].alpha = 0.6;
               }
               else
               {
                  this._slots[_loc3_].alpha = 1;
                  if(this._slots[_loc3_].selectedBorder)
                  {
                     this._slots[_loc3_].selectedBorder.visible = true;
                  }
                  if(this._slots[_loc3_].border && this._slots[_loc3_].border.inner)
                  {
                     this._slots[_loc3_].border.inner.visible = true;
                  }
               }
            }
            else
            {
               if(this._slots[_loc3_].border && this._slots[_loc3_].border.inner)
               {
                  this._slots[_loc3_].border.inner.visible = false;
               }
               if(this._slots[_loc3_].btnChange && this._slots[_loc3_].data)
               {
                  this._slots[_loc3_].btnChange.visible = true;
               }
               if(this._slots[_loc3_].btnDrop && this._slots[_loc3_].data)
               {
                  this._slots[_loc3_].btnDrop.visible = true;
               }
               this._slots[_loc3_].enabled = true;
               this._slots[_loc3_].alpha = 1;
               if(this._slots[_loc3_].selectedBorder)
               {
                  this._slots[_loc3_].selectedBorder.visible = false;
               }
            }
            _loc3_++;
         }
      }
      
      private function onItemClicked(param1:ButtonEvent) : void
      {
         stackTrace("onItemClciked");
         var _loc2_:Object = param1.currentTarget.data;
         trace("onItemClicked:;",_loc2_ ? _loc2_.category : null,param1.target,param1.currentTarget);
         if(_loc2_ && _loc2_.category == 26)
         {
            if(param1.target.name == "btnDrop")
            {
               return;
            }
            if(param1.target.name == "btnChange" && !this.decalControl.visible)
            {
               editData = _loc2_;
               trace("enter_decal_editor",this._data.itemId,_loc2_.itemId);
               this.lockControls(true,param1.currentTarget);
               ExternalInterface.call("start_decal_edit",this._data.itemId,_loc2_.itemId);
            }
         }
      }
      
      protected function onLocal(param1:String) : void
      {
         this.decalSlot.label = LocalizationData.getName("decal_slot");
         this.decalSlot1.label = LocalizationData.getName("decal_slot");
         this.decalSlot2.label = LocalizationData.getName(hostDictId >= 0 ? "camo_slot" : "decal_slot");
         this.camoSlot.label = LocalizationData.getName("camo_slot");
         this.infoContainer.infoDescr.text = LocalizationData.getName("modification_screen_hint") + "\n" + LocalizationData.getName("modification_screen_edit_decal_hint");
         this.infoContainer.background.height = this.infoContainer.infoDescr.textHeight + 24;
      }
      
      private function onMouseItemClick(param1:MouseEventEx) : void
      {
         if(param1.buttonIdx == 0)
         {
            return;
         }
         var _loc2_:DragSlot = param1.target as DragSlot;
         trace("onMouseItemClick:::",param1.target,param1.currentTarget,_loc2_);
         if(!_loc2_ || _loc2_.enabled == false || _loc2_.dragEnabled == false || _loc2_.data == null)
         {
            return;
         }
         _selectedItem = _loc2_;
      }
      
      private function dropElement() : void
      {
         this._selectedItem.data = null;
      }
      
      private function onItemOut(param1:Event) : void
      {
      }
      
      private function onItemOver(param1:Event) : void
      {
         var _loc2_:UpgradeSlot = param1.target as UpgradeSlot;
         if(_loc2_ && _loc2_.data && _loc2_.data.modifiers)
         {
         }
      }
      
      private function onItemUpdated(param1:Event) : void
      {
         _upgrades = [];
         _hasChanges = true;
         trace("UW:onItemUpdated",param1.target,param1.currentTarget);
         var _loc2_:UpgradeSlot = param1.target as UpgradeSlot;
         var _loc3_:Object = _loc2_.getLastData();
         trace("updateData::",_loc2_.data,_loc3_);
         if(_loc3_ && !_loc2_.data)
         {
            trace("drop_upgrade_item::updated",this._data.itemId,_loc3_.itemId);
            ExternalInterface.call("drop_upgrade_item",this._data.itemId,uint(_loc3_.itemId));
         }
      }
      
      private function onItemDropped(param1:DragEvent) : void
      {
         var _loc2_:uint;
         var _loc3_:Object;
         var _loc4_:UpgradeSlot;
         trace("onItemDropped::",param1.dragData);
         if(param1.dragData)
         {
            _loc2_ = 0;
            while(_loc2_ < this._slots.length)
            {
               _loc4_ = this._slots[_loc2_];
               trace("slotId::",param1.dragData.itemId,_loc4_.data ? _loc4_.data.itemId : "ff");
               if(_loc4_.data && _loc4_.data.itemId == param1.dragData.itemId)
               {
                  _loc4_.dropSlot();
               }
               _loc2_++;
            }
            _loc4_ = param1.target as UpgradeSlot;
            trace("updateData::");
            _loc3_ = this.create_item(param1.dragData.itemId);
            _loc4_.setData(_loc3_);
            trace("relocate_upgrade_item",this._data.itemId,uint(_loc4_.data.itemId),uint(_loc4_.index));
            ExternalInterface.call("relocate_upgrade_item",this._data.itemId,uint(_loc4_.data.itemId),uint(_loc4_.index));
         }
      }
      
      private function completeRelocate(param1:Object) : void
      {
         var _loc2_:DragEvent = param1.dragEvent;
         var _loc3_:UpgradeSlot = _loc2_.target as UpgradeSlot;
         trace("updateData::");
         _loc3_.setData(this.create_item(_loc2_.dragData.itemId));
      }
      
      private function acceptChanges(param1:ButtonEvent = null) : void
      {
         _hasChanges = false;
         this.closePopup();
      }
      
      private function cancelChanges(param1:ButtonEvent = null) : void
      {
         _hasChanges = false;
         this.closePopup();
         var _loc2_:uint = 0;
         var _loc3_:UpgradeSlot;
         var _loc4_:Object;
         var _loc5_:Object;
         while(_loc2_ < this._slots.length)
         {
            _loc3_ = this._slots[_loc2_];
            _loc4_ = _loc3_.data;
            _loc5_ = _loc3_.startData;
            trace("cancelChanges::",_loc4_,_loc5_);
            if(_loc5_)
            {
               if(_loc4_)
               {
                  trace("slotData.itemId",_loc4_.itemId,_loc5_.itemId);
                  if(_loc4_.itemId != _loc5_.itemId)
                  {
                     trace("cancel:drop_upgrade_item",this._data.itemId,uint(_loc4_.itemId));
                     ExternalInterface.call("drop_upgrade_item",this._data.itemId,uint(_loc4_.itemId));
                     trace("cancel:relocate_upgrade_item",this._data.itemId,uint(_loc5_.itemId));
                     ExternalInterface.call("relocate_upgrade_item",this._data.itemId,uint(_loc5_.itemId),uint(_loc3_.index));
                  }
               }
               else
               {
                  trace("cancel:drop_upgrade_item",this._data.itemId,uint(_loc4_.itemId));
                  ExternalInterface.call("drop_upgrade_item",this._data.itemId,uint(_loc4_.itemId));
               }
            }
            else if(_loc4_)
            {
               trace("cancel:drop_upgrade_item",this._data.itemId,uint(_loc4_.itemId));
               ExternalInterface.call("drop_upgrade_item",this._data.itemId,uint(_loc4_.itemId));
            }
            _loc2_++;
         }
      }
   }
}

