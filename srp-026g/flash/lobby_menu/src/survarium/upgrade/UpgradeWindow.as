package survarium.upgrade
{
   import flash.display.DisplayObject;
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

      // A weapon's window has two pages, its attachments (the scope) and its appearance (the decals and
      // the camouflage), turned by the arrows either side of the title.
      public static const PAGE_ATTACHMENTS:int = 0;

      public static const PAGE_APPEARANCE:int = 1;

      // The title bar (the movie's): 180 wide from x 5. The arrows (PageArrow, 21 wide) 2 in from its
      // ends, from y 6.5 (the bar is 5 to 32 above its line); the title 124 wide between them (33 to 157).
      protected static const TITLE_WIDTH:Number = 176;

      protected static const TITLE_WIDTH_BETWEEN_ARROWS:Number = 124;

      protected static const PAGE_PREV_X:Number = 7;

      protected static const PAGE_NEXT_X:Number = 162;

      protected static const PAGE_ARROW_Y:Number = 6.5;

      // the line above armour's camouflage (the bar's lines are at 35 and 525.95), a thin shape
      protected static const SEPARATOR_Y:Number = 352;

      protected static const SEPARATOR_MAX_HEIGHT:Number = 5;
      
      // A weapon's decals: two, as armour's (the server's WEAPON_DECAL_SLOTS). A weapon's upgrades
      // come packed into at most three records, the weapon kit (docs/notes/weapon-kit.md, exe patch
      // 68): kitUpgrades gives the window their slots.
      protected static const WEAPON_DECALS:int = 2;

      // each slot's place as the movie has it (armour's window): [x, y, scaleX, scaleY] by slot
      protected var _slotPlaces:Dictionary;

      protected var _page:int = PAGE_ATTACHMENTS;

      protected var _hasAttachments:Boolean = false;

      protected var _hasAppearance:Boolean = false;

      protected var _pagePrev:PageArrow;

      protected var _pageNext:PageArrow;

      protected var _separator:DisplayObject;

      // set while onItemDropped empties the slot an item is moved from: no drop is sent for it
      private var _moving:Boolean = false;

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

      // A weapon the window opens for: one that takes scopes, a camouflage or decals.
      public static function isUpgradeableWeapon(item:Object) : Boolean
      {
         return isWeaponCategory(int(item.category)) && (hasScopes(int(item.dictId)) || Boolean(item.camoAvailable) || Boolean(item.decalAvailable));
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

      // A double click on an owned item of the inventory while the window is open
      // (InventoryList.onItemDoubleClick): the item goes where a drop puts it (put), into the first
      // slot shown that takes it and is empty, else the first that takes it (a decal over the first
      // decal when both are full). False, and nothing done, when no slot shown takes it, the window
      // already holds it or a decal's place is being edited (the slots locked: lockControls).
      public static function attach(item:Object) : Boolean
      {
         var slot:UpgradeSlot = null;
         if(!instance || !instance.visible || !instance._data || !item || instance.decalControl.visible)
         {
            return false;
         }
         slot = instance.slotFor(item);
         if(slot == null)
         {
            return false;
         }
         instance.put(slot,uint(item.itemId));
         return true;
      }

      // A weapon's upgrades by its logical slots (decals 0 and 1, camouflage 2, scope 3, muzzle 4,
      // grip 5): the records as they come (the decals), the weapon kit's record (exe patch 68 gives
      // its object kit_own, the kit's own slot, and kit_camo, kit_muzzle, kit_grip, the dict ids it
      // names, 0 none; it reads and checks the record) as its own item in its own slot, and each dict
      // id it names as the inventory's item of that id. docs/notes/weapon-kit.md.
      public static function kitUpgrades(upgrades:Array) : Array
      {
         var out:Array = [];
         var i:int = 0;
         var upgrade:Object = null;
         var named:Array = null;
         var k:int = 0;
         var item:Object = null;
         if(!upgrades)
         {
            return out;
         }
         while(i < upgrades.length)
         {
            upgrade = upgrades[i];
            i++;
            if(!upgrade || upgrade.kit_own == undefined)
            {
               out.push(upgrade);
               continue;
            }
            if(uint(upgrade.id) != 0)
            {
               out.push({
                  "id":upgrade.id,
                  "param":int(upgrade.kit_own)
               });
            }
            named = [[CosmeticSlot.DECAL_3,int(upgrade.kit_camo)],[CosmeticSlot.MUZZLE,int(upgrade.kit_muzzle)],[CosmeticSlot.GRIP,int(upgrade.kit_grip)]];
            k = 0;
            while(k < named.length)
            {
               item = named[k][1] != 0 ? inventoryItemOf(named[k][1]) : null;
               if(Boolean(item))
               {
                  out.push({
                     "id":item.itemId,
                     "param":named[k][0]
                  });
               }
               k++;
            }
         }
         return out;
      }

      // The inventory's item of a dictionary id (a kit names its camouflage, muzzle device and grip so).
      public static function inventoryItemOf(dictId:int) : Object
      {
         if(!instance || !instance._inventory)
         {
            return null;
         }
         return instance._inventory.getInventoryItemOf(dictId);
      }

      public static function inventoryItem(itemId:uint) : Object
      {
         if(!instance || !instance._inventory)
         {
            return null;
         }
         return instance._inventory.getInventoryItem(itemId);
      }
      
      // The chosen kit's weapons (slots 1 and 2) by dictionary id, for a sight's card (weapon_scopes.md).
      public static function kitWeapons() : Array
      {
         return Boolean(instance) && Boolean(instance._inventory) ? instance._inventory.kitWeaponDicts() : [];
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
         var upgrades:Array = null;
         if(Boolean(this._data))
         {
            trace("UW:upgrade_item_selected::",this._data.itemId,"up",this._data.upgrades.length,resetSlots,"decalSlots::",this._data.decalAvailable,"camoSlots::",this._data.camoAvailable);
            this.decalSlot.visible = this._data.decalAvailable > 0;
            this.decalSlot1.visible = this._data.decalAvailable > 1;
            this.decalSlot2.visible = this._data.decalAvailable > 2;
            this.camoSlot.visible = this._data.camoAvailable > 0;
            if(hostDictId >= 0)
            {
               // a weapon: its scope in camoSlot (CosmeticSlot.CAMOUFLAGE), its camouflage in decalSlot2
               // (DECAL_3), its one decal in decalSlot (DECAL_1). Every slot it has is shown while the
               // slots are filled below; then showPage leaves the page's own.
               this.decalSlot.visible = this.weaponDecals() > 0;
               this.decalSlot1.visible = this.weaponDecals() > 1;
               this.decalSlot2.visible = this._data.camoAvailable > 0;
               this.camoSlot.visible = hasScopes(hostDictId);
               this._hasAttachments = this.camoSlot.visible;
               this._hasAppearance = this.decalSlot.visible || this.decalSlot2.visible;
               // a new window opens on the attachments when the weapon takes scopes; the account's
               // update keeps the page
               if(resetSlots || !(this._page == PAGE_ATTACHMENTS ? this._hasAttachments : this._hasAppearance))
               {
                  this._page = this._hasAttachments ? PAGE_ATTACHMENTS : PAGE_APPEARANCE;
               }
               this.placeWeaponSlots();
               this.camoSlot.label = Boolean(stockScopes[hostDictId]) ? LocalizationData.getName("default_scope") : LocalizationData.getName("st_srp_scope_slot");
               this.decalSlot2.label = LocalizationData.getName("camo_slot");
               this.decalSlot2.contentCanvas.scaleX = this.decalSlot2.contentCanvas.scaleY = 1;
            }
            else if(Boolean(this._slotPlaces))
            {
               this.restoreSlots();
               if(Boolean(this._separator))
               {
                  this._separator.visible = true;
               }
               this.configDecalSlot(this.decalSlot2);
               this.showTitle(LocalizationData.getName("btn_upgrade_label"),false);
               this.onLocal(null);
               this.infoContainer.y = MainScene.stageHeight - this.infoContainer.height - 100;
            }
            if(resetSlots || this._data.upgrades.length == 0)
            {
               this.cleanSlots();
            }
            if(Boolean(this._data.upgrades) && resetSlots)
            {
               upgrades = hostDictId >= 0 ? kitUpgrades(this._data.upgrades) : this._data.upgrades;
               upgradeCount = int(upgrades.length);
               i = 0;
               while(i < upgradeCount)
               {
                  slotCount = int(this._slots.length);
                  j = 0;
                  while(j < slotCount)
                  {
                     slot = this._slots[j];
                     trace("UW::slotId::",slot.visible,slot.index,upgrades[i].id,upgrades[i].param);
                     if(slot.visible && slot.index == upgrades[i].param || slot.index == CosmeticSlot.CAMOUFLAGE && upgrades[i].param == undefined)
                     {
                        if(!slot.data || slot.data.itemId != upgrades[i].id)
                        {
                           slotData = this.create_item(upgrades[i].id);
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
            if(hostDictId >= 0)
            {
               this.showPage(this._page,resetSlots);
            }
         }
         else
         {
            this.decalSlot2.visible = this.decalSlot1.visible = this.decalSlot.visible = this.camoSlot.visible = false;
            this.decalSlot2.data = this.decalSlot1.data = this.decalSlot.data = this.camoSlot.data = null;
         }
      }

      // A weapon's slots in armour's places, as its appearance window has them: on the appearance page
      // its decal at the top, where armour's second decal is, and its camouflage at the bottom, under
      // the line, where armour's camouflage is; on the attachments page the scope at the top.
      protected function placeWeaponSlots() : void
      {
         var i:uint = 0;
         var slot:UpgradeSlot = null;
         if(!this._slotPlaces)
         {
            this._slotPlaces = new Dictionary();
            while(i < this._slots.length)
            {
               slot = this._slots[i];
               this._slotPlaces[slot] = [slot.x,slot.y,slot.scaleX,slot.scaleY];
               i++;
            }
         }
         this.placeSlot(this.decalSlot,this._slotPlaces[this.decalSlot1]);
         this.placeSlot(this.decalSlot1,this._slotPlaces[this.decalSlot]);
         this.placeSlot(this.decalSlot2,this._slotPlaces[this.camoSlot]);
         this.placeSlot(this.camoSlot,this._slotPlaces[this.decalSlot1]);
      }

      protected function placeSlot(slot:UpgradeSlot, place:Array) : void
      {
         slot.x = place[0];
         slot.y = place[1];
         slot.scaleX = place[2];
         slot.scaleY = place[3];
      }

      // Armour's window: every slot where the movie has it.
      protected function restoreSlots() : void
      {
         var i:uint = 0;
         var slot:UpgradeSlot = null;
         while(i < this._slots.length)
         {
            slot = this._slots[i];
            this.placeSlot(slot,this._slotPlaces[slot]);
            i++;
         }
      }

      // A weapon's page: its slots, the title between the arrows (none when the weapon has only one
      // page), the hint, and, when `selectFilter`, the inventory's filter on what goes in them, as the
      // window's opening chooses it (scopes; camouflages and decals, armour's appearance filter).
      protected function showPage(page:int, selectFilter:Boolean) : void
      {
         var attachments:Boolean = page == PAGE_ATTACHMENTS;
         this._page = page;
         this.camoSlot.visible = attachments && this._hasAttachments;
         this.decalSlot.visible = !attachments && this.weaponDecals() > 0;
         this.decalSlot1.visible = !attachments && this.weaponDecals() > 1;
         this.decalSlot2.visible = !attachments && this._data.camoAvailable > 0;
         if(Boolean(this._separator))
         {
            this._separator.visible = !attachments;
         }
         this.showTitle(LocalizationData.getName(attachments ? "scope_window_title" : "btn_upgrade_label"),this._hasAttachments && this._hasAppearance);
         this.infoContainer.infoDescr.text = attachments ? LocalizationData.getName("scope_screen_hint") : LocalizationData.getName("modification_screen_hint") + (this._data.decalAvailable > 0 ? "\n" + LocalizationData.getName("modification_screen_edit_decal_hint") : "");
         this.infoContainer.background.height = this.infoContainer.infoDescr.textHeight + 24;
         this.infoContainer.y = MainScene.stageHeight - this.infoContainer.height - 100;
         if(selectFilter && Boolean(this._inventory))
         {
            this._inventory.selectCategory(attachments ? [ItemCategory.SCOPES] : [ItemCategory.CAMOUFLAGES,ItemCategory.DECALS],false,true);
         }
      }

      // The decal slots a weapon's window shows: the item's, at most WEAPON_DECALS.
      protected function weaponDecals() : int
      {
         return Math.min(int(this._data.decalAvailable),WEAPON_DECALS);
      }
      
      // The window's title; with the page arrows, it keeps between them.
      protected function showTitle(title:String, withArrows:Boolean) : void
      {
         fitText(this.container.textField,title,withArrows ? TITLE_WIDTH_BETWEEN_ARROWS : TITLE_WIDTH);
         if(Boolean(this._pagePrev))
         {
            this._pagePrev.visible = this._pageNext.visible = withArrows;
         }
      }

      private function onPageArrow(event:ButtonEvent) : void
      {
         if(hostDictId < 0 || !this._data || !(this._hasAttachments && this._hasAppearance))
         {
            return;
         }
         this.showPage(this._page == PAGE_ATTACHMENTS ? PAGE_APPEARANCE : PAGE_ATTACHMENTS,true);
      }

      protected function addPageArrow(x:Number, pointRight:Boolean) : PageArrow
      {
         var arrow:PageArrow = new PageArrow();
         arrow.pointRight = pointRight;
         arrow.x = x;
         arrow.y = PAGE_ARROW_Y;
         arrow.visible = false;
         arrow.addEventListener(ButtonEvent.CLICK,this.onPageArrow);
         this.container.addChild(arrow);
         return arrow;
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
      
      // A drag's start lights up the slots its item goes into (a slot takes a drop only lit up:
      // UpgradeSlot.handleDropEvent).
      public function selectAgeements(category:int, dictId:int = 0) : void
      {
         trace("UW::selectAgeements",category,dictId,this._slots);
         var i:int = 0;
         while(i < this._slots.length)
         {
            this._slots[i].highlighted = this.takes(this._slots[i],category,dictId);
            i++;
         }
      }

      // Whether a slot, as the window shows it, takes an item of this category and dict id: a
      // weapon's decals, camouflage (decalSlot2) and the scopes that fit it (camoSlot); armour's by
      // the slots' categories (_slotAgreements, the game's restrictions added).
      protected function takes(slot:UpgradeSlot, category:int, dictId:int) : Boolean
      {
         if(!slot.visible || !slot.enabled)
         {
            return false;
         }
         if(hostDictId >= 0)
         {
            return category == this.weaponSlotCategory(slot) && (category != ItemCategory.SCOPES || fits(hostDictId,dictId));
         }
         return Boolean(this._slotAgreements[slot.index]) && (this._slotAgreements[slot.index] as Array).indexOf(category) != -1;
      }

      // Where a double click puts an item (attach): the first slot shown that takes it and is
      // empty, else the first that takes it; none when a slot of the window holds it already.
      protected function slotFor(item:Object) : UpgradeSlot
      {
         var i:int = 0;
         var slot:UpgradeSlot = null;
         var first:UpgradeSlot = null;
         if(this.holds(null,uint(item.itemId)))
         {
            return null;
         }
         while(i < this._slots.length)
         {
            slot = this._slots[i];
            if(this.takes(slot,int(item.category),int(item.dictId)))
            {
               if(!slot.data)
               {
                  return slot;
               }
               if(first == null)
               {
                  first = slot;
               }
            }
            i++;
         }
         return first;
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
         this._pagePrev = this.addPageArrow(PAGE_PREV_X,false);
         this._pageNext = this.addPageArrow(PAGE_NEXT_X,true);
         var child:int = 0;
         var shape:DisplayObject;
         while(child < this.container.numChildren)
         {
            shape = this.container.getChildAt(child);
            if(!(shape is UIComponent) && Math.abs(shape.y - SEPARATOR_Y) < 0.5 && shape.height < SEPARATOR_MAX_HEIGHT)
            {
               this._separator = shape;
            }
            child++;
         }
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
            category = hostDictId >= 0 ? this.weaponSlotCategory(slot) : int(this._slotAgreements[slot.index]);
            this._inventory.selectCategory([category]);
         }
      }
      
      // What goes in a weapon's slot: its camouflage, its scope or a decal.
      protected function weaponSlotCategory(slot:UpgradeSlot) : int
      {
         return slot == this.decalSlot2 ? ItemCategory.CAMOUFLAGES : (slot == this.camoSlot ? ItemCategory.SCOPES : ItemCategory.DECALS);
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
         this.decalSlot2.label = LocalizationData.getName(hostDictId >= 0 ? "camo_slot" : "decal_slot");
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
         if(lastData && !slot.data && !this._moving && !this.holds(slot,uint(lastData.itemId)))
         {
            trace("drop_upgrade_item::updated",this._data.itemId,lastData.itemId);
            ExternalInterface.call("drop_upgrade_item",this._data.itemId,uint(lastData.itemId));
         }
      }

      // Another slot of the window holds the item: a drag from one slot to another, whose drop
      // (relocate_upgrade_item, the move) comes before the slot it left is emptied (DragManager: the
      // target's drop, then the source's drag end). Dropping it then would take it off the item.
      protected function holds(except:UpgradeSlot, itemId:uint) : Boolean
      {
         var i:uint = 0;
         while(i < this._slots.length)
         {
            if(this._slots[i] != except && Boolean(this._slots[i].data) && uint(this._slots[i].data.itemId) == itemId)
            {
               return true;
            }
            i++;
         }
         return false;
      }

      private function onItemDropped(event:DragEvent) : void
      {
         trace("onItemDropped::",event.dragData);
         if(event.dragData)
         {
            this.put(event.target as UpgradeSlot,uint(event.dragData.itemId));
         }
      }

      // An owned item into a slot, as a drop (onItemDropped) or a double click (attach) puts it:
      // shown in the slot at once and sent to the game (relocate_upgrade_item); what the slot held
      // comes off the item there.
      protected function put(slot:UpgradeSlot, itemId:uint) : void
      {
         var i:uint = 0;
         var other:UpgradeSlot = null;
         var item:Object = null;
         while(i < this._slots.length)
         {
            other = this._slots[i];
            trace("slotId::",itemId,other.data ? other.data.itemId : "ff");
            if(other.data && other.data.itemId == itemId)
            {
               // a move from another slot of this window: the relocate below moves it on the
               // server (keeping a decal's place), so emptying the slot it left sends no drop
               this._moving = true;
               other.dropSlot();
               this._moving = false;
            }
            i++;
         }
         trace("updateData::");
         item = this.create_item(itemId);
         // an item not owned (none should be listed): nothing to put, and no error that would
         // leave the drag on the cursor
         if(item == null)
         {
            return;
         }
         slot.setData(item);
         trace("relocate_upgrade_item",this._data.itemId,uint(slot.data.itemId),uint(slot.index));
         ExternalInterface.call("relocate_upgrade_item",this._data.itemId,uint(slot.data.itemId),uint(slot.index));
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

