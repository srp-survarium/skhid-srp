package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.utils.Dictionary;
   import flash.utils.Timer;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.DragSlot;
   import scaleform.clik.controls.ScrollingList;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.gfx.MouseEventEx;
   import scaleform.gfx.TextFieldEx;
   import survarium.Util;
   import survarium.constants.Events;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.data.PropertiesDescriptor;
   import survarium.interfaces.IAmmoShare;
   import survarium.interfaces.IInventoryItemFactory;
   import survarium.interfaces.IInventoryItemRenderer;
   import survarium.interfaces.ILocalPaperDoll;
   import survarium.interfaces.ILocalPlayerProfile;
   import survarium.interfaces.IPaperDollSlot;
   import survarium.item.ItemFactory;
   import survarium.messages.MessageCraftItem;
   import survarium.upgrade.UpgradeWindow;
   import survarium.constants.Faction;
   import survarium.constants.InventoryTab;
   import survarium.constants.ItemCategory;
   import survarium.constants.ItemCondition;
   import survarium.constants.ItemDefaults;
   import survarium.constants.ItemPlace;
   import survarium.constants.ItemPrice;
   import survarium.constants.ModificationState;
   import survarium.constants.ModifierType;
   import survarium.constants.PropertyDirection;
   import survarium.constants.QuickSlotSize;
   import survarium.constants.ShopFilter;
   import survarium.constants.SuperCategory;
   import survarium.constants.VoucherType;
   import survarium.constants.WeaponHands;
   
   public class InventoryList extends FiltersWindow implements IInventoryItemFactory
   {
      
      public static const MAX_FREE_ITEMS_COUNT:uint = 1000;
      
      public static const HIDE_ITEM_HINT:String = "hideItemHint";
      
      public static var AllOW_COMPARE:Boolean = true;
      
      private var _inventoryItems:Array;
      
      private var _shopItems:Array;
      
      private var _inventoryItemsDict:Dictionary;
      
      private var _dragListener:Function;
      
      private var _newItemsInfo:Array = new Array();
      
      private var _oldItems:Array = new Array();
      
      private var _activated:Boolean = false;
      
      private var _inventoryLocked:Boolean = false;
      
      private var _shopLocked:Boolean = false;
      
      private var _cardItem:Object = null;
      
      private var _ammoItems:Array;
      
      private var _propsDescriptor:PropertiesDescriptor;
      
      private var _ignoreNextAttach:int = 0;
      
      private var _useItemId:uint;
      
      private var _activeItems:int = InventoryTab.INVENTORY;
      
      private var _allwayShowControlPanel:Boolean = false;
      
      private var _hoverIndex:int = -1;
      
      public var updateItemCard:Boolean = true;
      
      public var ignoreSelection:Boolean = false;
      
      public var tx_weight_label:TextField;
      
      public var tx_weight:TextField;
      
      public var tx_total_label:TextField;
      
      public var tx_total:TextField;
      
      public var shopSubsystem:UIWindow = new UIWindow();
      
      public var bundleList:MovieClip;
      
      public var item_compare:ItemCompare;
      
      public var textField:TextField;
      
      public var checkbox_all_traders:Button;
      
      public var _item_control:*;
      
      public var shop_lights:MovieClip;
      
      public var checkboxCompareItems:Button;
      
      public var sortTab:ButtonBarDynam;
      
      public var shopTab:ButtonBarDynam = new ButtonBarDynam();
      
      public var itemControlPanel:ItemControlPanel;
      
      public var isShop:Boolean = false;
      
      protected var _bundleItems:Array = [];
      
      private var _filterItems:Array;
      
      protected var _filterDictId:int = -1;
      
      protected var _filterselectedId:int = -1;
      
      public function InventoryList()
      {
         super();
         itemsTileList.addEventListener(ListEvent.ITEM_ROLL_OVER,this.newItemOver);
         itemsTileList.addEventListener(ListEvent.ITEM_ROLL_OUT,this.newItemOut);
         itemsTileList.addEventListener(ListEvent.ITEM_CLICK,this.onItemPress);
         sellersTabPage.addEventListener(MouseEvent.MOUSE_UP,this.checkReskin);
         optionControl.shopTab = this.shopTab;
         optionControl.setCheckBox(this.checkbox_all_traders);
         optionControl.setSortTab(this.sortTab);
         this._inventoryItemsDict = new Dictionary(true);
         label.mouseEnabled = false;
         LocalizationData.addTextField(label,"inventory_label");
      }
      
      public static function logModifiers(value:*) : void
      {
         trace("logModifiers");
         if(!value)
         {
            return;
         }
         var _propsDescriptor:* = PropertiesDescriptor.getPropertiesDescriptor("properties");
         var modifiers:Array = value;
         if(!modifiers)
         {
            return;
         }
         for(var i:uint = 0; i < modifiers.length; i++)
         {
            var modificatorProps:Object;
            var modifierProp:Object;
            var propDescription:Object;
            modificatorProps = modifiers[i].props;
            trace("modifier::",modifiers[i].id,modifiers[i].name);
            for each(modifierProp in modificatorProps)
            {
               propDescription = _propsDescriptor.getPropertyDescription(modifierProp.prop_id);
               trace("addPropValue::",modifierProp.prop_id,modifierProp.prop_value,propDescription.prop_name);
            }
         }
      }
      
      override public function activate(value:Boolean, byUser:Boolean = true) : void
      {
         var subsystemName:String;
         trace("activate",name,value,byUser,"vis",visible);
         if(byUser)
         {
            subsystemName = this.isShop ? this.shopSubsystem.subsystem_name : subsystem_name;
            if(!visible && value)
            {
               trace("subsystem_activated",subsystemName);
               ExternalInterface.call("subsystem_activated",subsystemName);
            }
            else if(visible && !value)
            {
               trace("subsystem_deactivated",subsystemName);
               ExternalInterface.call("subsystem_deactivated",subsystemName);
            }
         }
         visible = value;
      }
      
      public function toggleShopMode(value:Boolean) : void
      {
         isShop = value;
         label.text = LocalizationData.getName(this.isShop ? "shop" : "inventory_label");
         this.shopTab.selectedIndex = this.isShop ? ShopFilter.SHOP : ShopFilter.INVENTORY;
         this.itemControlPanel.toggleShop(value);
      }
      
      override public function get width() : Number
      {
         return background.width;
      }
      
      override public function get height() : Number
      {
         return background.height;
      }
      
      override public function set width(value:Number) : void
      {
         super.width = value;
      }
      
      public function set item_control(value:*) : void
      {
         _item_control = value;
         this._item_control.addEventListener(Event.RESIZE,this.onCardResize);
         this._item_control.addEventListener(Events.SELL,this.onCardSell);
         this._item_control.addEventListener(Events.BUY,this.onCardBuy);
         this.itemControlPanel.setItemControl(this._item_control);
      }
      
      override public function set y(value:Number) : void
      {
         super.y = value;
         this.onCardResize();
      }
      
      public function set enableControls(value:Boolean) : void
      {
         itemsTileList.scrollBar.enabled = value;
      }
      
      override public function set visible(value:Boolean) : void
      {
         if(visible && value == false)
         {
            dispatchEvent(new Event(Event.CLOSE));
         }
         var pp:MovieClip = parent["player_profile"];
         trace("II:visible",pp);
         super.visible = value;
         if(this.isShop)
         {
            pp.visible = false;
         }
         else
         {
            pp.visible = value;
            this._item_control.visible = value;
            if(!this._activated)
            {
               _activated = true;
            }
            if(value == false && (pp.ammoContainer.btn_apply.enabled))
            {
            }
         }
      }
      
      public function hasItem(value:int) : Boolean
      {
         var l:int = (this._inventoryItems.length);
         for(var i:uint = 0; i < l; i++)
         {
            if(this._inventoryItems[i].dictId == value)
            {
               return true;
            }
         }
         return false;
      }
      
      public function selectItemDict(value:int, seller:int = Faction.ANY) : void
      {
         var l:int = (itemsTileList.dataProvider.length);
         for(var i:uint = 0; i < l; i++)
         {
            var itemData:ItemData;
            itemData = itemsTileList.dataProvider[i];
            if(itemData.dictId == value && (itemData.factionId == seller || seller == Faction.ANY))
            {
               itemsTileList.selectedIndex = i;
               return;
            }
         }
      }
      
      protected function onCardSell(e:Event) : void
      {
         var cardData:Object = this._item_control.getItemData();
         trace("onCardSell",cardData.id);
         var descrData:Object = _itemDescriptor.getItemDescription(cardData.dictId);
         _cardItem = {
            id:cardData.id,
            action:"sell",
            name:descrData.name
         };
      }
      
      protected function onCardBuy(e:Event) : void
      {
         var cardData:Object = this._item_control.getItemData();
         trace("onCardBuy",cardData.dictId);
         var itemsAtStart:int = 0;
         for(var i:uint = 0; i < this._inventoryItems.length; i++)
         {
            if(this._inventoryItems[i].dictId == cardData.dictId)
            {
               itemsAtStart++;
            }
         }
         _cardItem = {
            id:this._item_control.getItemData().dictId,
            action:"buy",
            itemsAtStart:itemsAtStart
         };
      }
      
      protected function onCardResize(e:Event = null) : void
      {
         if((this._item_control))
         {
         }
      }
      
      protected function size() : void
      {
         trace("szd",this.width,this.height,x,y,getBounds(parent));
         for(var i:int = 0; i < numChildren; i++)
         {
            var chd:*;
            chd = this.getChildAt(i);
            if((chd))
            {
               trace("chd",chd,chd.name,chd.x,chd.y,chd.width,chd.height,"|",chd.x + chd.width,chd.y + chd.height,chd.getBounds(this));
            }
         }
      }
      
      private function deleteDoubleItems(value:Array, seller:int, dict:Dictionary) : void
      {
         var tmpObject:Object = new Object();
         var doubleObject:Object = new Object();
         var tempItems:Array = new Array();
         for(var i:uint = 0; i < value.length; i++)
         {
            itemData = value[i];
            if(tmpObject[itemData.dictId] == true)
            {
               doubleObject[itemData.dictId] = true;
            }
            else
            {
               tmpObject[itemData.dictId] = true;
            }
         }
         for(i = 0; i < value.length; i++)
         {
            if(doubleObject[value[i].dictId] == true)
            {
               tempItems.push(value[i]);
               value.splice(i,1);
               i--;
            }
            else
            {
               dict[value[i].dictId] = value[i];
            }
         }
         var handledObject:Object = new Object();
         var itemData:ItemData;
         for(i = 0; i < tempItems.length; i++)
         {
            if((tempItems[i]) && ((seller != Faction.ANY && tempItems[i].seller == seller || seller == Faction.ANY && tempItems[i].canBuy == true)) && !handledObject[tempItems[i].dictId])
            {
               handledObject[tempItems[i].dictId] = true;
               value.push(tempItems[i]);
               dict[tempItems[i].dictId] = tempItems[i];
               tempItems.splice(i,1);
               i--;
            }
         }
         for(i = 0; i < tempItems.length; i++)
         {
            if((tempItems[i]) && !handledObject[tempItems[i].dictId])
            {
               handledObject[tempItems[i].dictId] = true;
               value.push(tempItems[i]);
               dict[tempItems[i].dictId] = tempItems[i];
               tempItems.splice(i,1);
               i--;
            }
         }
      }
      
      public function toggleUpgradeMode() : void
      {
         _allwayShowControlPanel = !this._allwayShowControlPanel;
         this.itemControlPanel.toggleModifiersButtons();
         if(this._allwayShowControlPanel && !this.itemControlPanel.visible)
         {
            this.onFilterShowed();
         }
      }
      
      public function sellItem(itemId:uint) : void
      {
         var itemData:ItemData = this._inventoryItemsDict[itemId];
         this.sellObject(itemData);
      }
      
      public function ignoreNextAttach(value:int) : void
      {
         _ignoreNextAttach = value;
      }
      
      public function onPlayerMoneyUpdate(e:Event = null) : void
      {
         if(mainTabBar.selectedIndex == InventoryTab.SHOP)
         {
            itemsTileList.invalidate(InvalidationType.ALL);
         }
      }
      
      public function setupShopItems(value:Array) : void
      {
         trace("IL::setupShopItems",sellersTabPage.selectedIndex);
         _shopItems = value;
         if(visible)
         {
         }
         else
         {
            _activated = false;
         }
         for(var i:uint = 0; i < value.length; i++)
         {
            item = value[i];
            if(item.category == ItemCategory.VOUCHERS)
            {
               if(PlayerInfo.getVoucherCount(item.type) == 0)
               {
                  PlayerInfo.setVoucherCount(item.dictId,0,item.type,0);
               }
            }
            this.fillModifiersDescription(item);
         }
         var supper:Array = optionControl.getSelectedIndexes();
         var filterOptions:Object = optionControl.getFilterOptions();
         var seller:int = sellersTabPage.selectedIndex != -1 ? (sellersTabPage.dataProvider[sellersTabPage.selectedIndex].faction_id) : Faction.NONE;
         var item:ItemData;
         this.ApplyInventoryFilter(supper,inventoryTabPage.selectedIndex,seller,filterOptions);
      }
      
      public function lockControls(value:Boolean) : void
      {
         trace("lockControls::",value);
         _inventoryLocked = value;
         inventoryTabPage.mouseEnabled = !value;
         inventoryTabPage.mouseChildren = !value;
         btnClose.mouseEnabled = !value;
         this.checkbox_all_traders.mouseEnabled = !value;
         optionControl.mouseEnabled = !value;
         optionControl.mouseChildren = !value;
         this.itemControlPanel.mouseEnabled = !value;
         this.itemControlPanel.mouseChildren = !value;
         this.checkboxCompareItems.mouseEnabled = !value;
         this._item_control.checkButtonEnabled();
      }
      
      public function lock(elementId:uint, state:Boolean) : void
      {
         if(elementId == LobbyMenu.INVENTORY_ELEMNENT_ID)
         {
            _inventoryLocked = state;
         }
         else
         {
            _shopLocked = state;
         }
         this.handleLockedWindow();
      }
      
      public function getElelementLocked(elementId:uint) : Boolean
      {
         if(elementId == LobbyMenu.INVENTORY_ELEMNENT_ID)
         {
            return this._inventoryLocked;
         }
         return this._shopLocked;
      }
      
      private function handleLockedWindow() : void
      {
         if(this._shopLocked && this._activeItems == InventoryTab.SHOP || this._inventoryLocked && this._activeItems == InventoryTab.INVENTORY)
         {
            itemsTileList.enabled = false;
            itemsTileList.invalidateData();
            this._item_control.checkButtonEnabled();
         }
         else if(itemsTileList.enabled != true)
         {
            itemsTileList.enabled = true;
            itemsTileList.invalidateData();
            this._item_control.checkButtonEnabled();
         }
      }
      
      public function fillRewardItems(value:Array) : void
      {
      }
      
      public function createBaseItem(dictId:uint) : ItemData
      {
         var descrData:Object = _itemDescriptor.getItemDescription(dictId);
         var item_data:ItemData = new ItemData();
         var icons:IconContainer;
         var category:int = ItemFactory.getCategory(descrData.category);
         if(category == SuperCategory.EQUIPMENT)
         {
            icons = IconContainer.getInstance("equipment");
         }
         else if(category == SuperCategory.WEAPONS)
         {
            icons = IconContainer.getInstance("weapons");
         }
         else if(category == SuperCategory.UPGRADES)
         {
            if(descrData.category == ItemCategory.VOUCHERS || descrData.category == ItemCategory.GENERIC_MONEY || descrData.category == ItemCategory.PREMIUM_MONEY)
            {
               icons = IconContainer.getInstance("vouchers");
            }
            else if(descrData.category == ItemCategory.DECALS)
            {
               icons = IconContainer.getInstance("decals");
            }
            else
            {
               icons = IconContainer.getInstance("camo");
            }
         }
         else
         {
            icons = IconContainer.getInstance("consumables");
         }
         if(descrData.icon_row == undefined)
         {
            descrData.icon_row = descrData.icon % icons.rows + 1;
            descrData.icon_column = int(descrData.icon / icons.rows) + 1;
            trace("create_icon",descrData.icon,descrData.icon_row,descrData.icon_column);
         }
         var iconRow:int = descrData.icon_row - 1;
         var iconColumn:int = descrData.icon_column - 1;
         item_data.iconId = iconColumn * icons.rows + iconRow;
         trace("IL_icon",item_data.iconId,icons.columns,"|",iconRow,iconColumn);
         if(descrData.double_handed == undefined)
         {
            descrData.double_handed = true;
         }
         item_data.matchMakingLevel = descrData.match_making_level;
         item_data.additionalSlots = descrData.additional_slots;
         item_data.type = descrData.voucher_type;
         item_data.superCategory = category;
         item_data.category = descrData.category;
         item_data.isWeapon = category == SuperCategory.WEAPONS;
         item_data.isStack = descrData.is_stack;
         item_data.oneHanded = !descrData.double_handed;
         item_data.isPremium = descrData.is_premium;
         item_data.name = descrData.name;
         item_data.dictId = dictId;
         item_data.repairCostRatio = descrData.repair_cost_ratio;
         if(descrData.modifiers == undefined)
         {
            descrData.modifiers = [];
         }
         item_data.modifiers = descrData.modifiers;
         item_data.modifiersDefault = descrData.modifiers;
         item_data.rank = descrData.rarity_level;
         item_data.fromBundle = item_data.rank > 0;
         var defModifersCount:uint = item_data.modifiersDefault.length;
         for(var i:uint = 0; i < defModifersCount; i++)
         {
            item_data.modifiersDefault[i].isDefault = true;
         }
         item_data.properties = descrData.item_properties;
         if((descrData.descr))
         {
            item_data.itemDescription = descrData.descr;
         }
         item_data.cost = descrData.cost;
         item_data.itemLevel = descrData.level;
         if(descrData.camo_available == undefined)
         {
            descrData.camo_available = ItemDefaults.CAMO_AVAILABLE;
         }
         if(descrData.decal_available == undefined)
         {
            descrData.decal_available = ItemDefaults.DECAL_AVAILABLE;
         }
         item_data.sellers = descrData.sellers;
         item_data.icon = icons.getIcon(item_data.iconId);
         item_data.dragIcon = icons.getIcon(item_data.iconId);
         item_data.messIcon = icons.getIcon(item_data.iconId);
         item_data.camoAvailable = descrData.camo_available;
         item_data.decalAvailable = descrData.decal_available;
         if(descrData.upgrades_available == undefined)
         {
            descrData.upgrades_available = ItemDefaults.UPGRADES_AVAILABLE;
         }
         item_data.upgradesAvailable = descrData.upgrades_available;
         item_data.weight = descrData.weight;
         item_data.clipSize = descrData.clip_size;
         item_data.quickSlotItemSize = (descrData.fills_whole_pocket) ? QuickSlotSize.WHOLE : QuickSlotSize.HALF;
         item_data.clipsInBag = descrData.clips_in_bag;
         if(descrData.required_bags != undefined)
         {
            item_data.clipsInBag = 1 / descrData.required_bags;
         }
         item_data.usable = item_data.category == ItemCategory.VOUCHERS && (item_data.type == VoucherType.PREMIUM || item_data.type == VoucherType.RENAME);
         var str:String = "";
         var ammoItems:Array;
         var ammoStr:String;
         var ammoL:uint;
         var j:uint;
         var descrName:String;
         if(item_data.isWeapon)
         {
            if(!descrData.ammo_text)
            {
               ammoItems = ItemFactory.getItemCombableItemNames(item_data.dictId);
               if((ammoItems))
               {
                  ammoStr = "";
                  ammoL = ammoItems.length;
                  item_data.ammoText = "";
                  for(j = 0; j < ammoL; j++)
                  {
                     descrName = _itemDescriptor.getItemDescription(ammoItems[j]).name;
                     if((descrName))
                     {
                        str += "   " + descrName + "\n";
                        item_data.ammoText += (j == 0 ? "" : ", ") + descrName;
                     }
                  }
               }
               descrData.ammo_text = item_data.ammoText;
            }
            else
            {
               item_data.ammoText = descrData.ammo_text;
            }
         }
         return item_data;
      }
      
      public function createItem(dictId:uint, seller:* = Faction.ANY) : ItemData
      {
         var seller_icons:IconContainer = IconContainer.getInstance("factionItemIcons");
         var descrData:Object = _itemDescriptor.getItemDescription(dictId);
         var item_data:ItemData = this.createBaseItem(dictId);
         var seller_icon:Icon;
         var factionId:int = seller;
         if((descrData) && (descrData.sellers))
         {
            if((descrData.sellers as Array).length == 1)
            {
               factionId = (descrData.sellers[0].faction_id);
            }
            else
            {
               factionId = Faction.ANY;
            }
         }
         if(factionId != Faction.ANY)
         {
            seller_icon = seller_icons.getIcon(factionId);
         }
         else
         {
            seller_icon = null;
         }
         item_data.sellerIcon = seller_icon;
         item_data.factionId = factionId;
         item_data.price = GangShop.getInstance().getItemPrice(dictId);
         trace("IL::create_item::",item_data.name,item_data.category,item_data.usable,item_data.type,item_data.category == ItemCategory.VOUCHERS,descrData.descr);
         return item_data;
      }
      
      public function createItemData(value:Object) : ItemData
      {
         var item:ItemData = this.createItem(value.dictId,value.faction_id);
         item.value = value.condition_or_stack;
         this.updateItem(item,value);
         return item;
      }
      
      protected function updateItem(item:ItemData, value:Object) : void
      {
         if(value.double_handed != undefined)
         {
            item.oneHanded = !value.double_handed;
         }
         if(value.modifiers != undefined)
         {
            item.broken = this.getItemBroken(value.modifiers);
            item.modifiers = (value.modifiers as Array).concat(item.modifiersDefault);
         }
         if(value.rarity_level != undefined)
         {
            item.rank = value.rarity_level;
         }
         if((value.cost))
         {
            item.cost = value.cost;
         }
         item.camoItem = value.camo_item;
         item.decalItem = value.decal_item;
         this.updateItemTipText(item);
         this.fillModifiersDescription(item);
      }
      
      protected function updateItemTipText(item:ItemData) : void
      {
         var lvlTip:String = item.superCategory != SuperCategory.UPGRADES ? Util.styledChar(LocalizationData.getName("equipment_level") + " - " + item.itemLevel.toString(),"gray") : "";
         var tipStr:String = "" + ((item.rank) ? Util.styledChar(LocalizationData.getName("modified") + "\n","green") : "") + ((item.ammoText) ? LocalizationData.getName("ammo") + ":\n" + item.ammoText : "") + ((lvlTip) ? Util.styledChar(" \n","small") + lvlTip : "");
         item.tipText = tipStr;
      }
      
      public function fillModifierDescription(modifier:Object) : void
      {
         var propDescription:Object;
         _propsDescriptor = PropertiesDescriptor.getPropertiesDescriptor("properties");
         var modificatorProps:Object = modifier.props;
         var modifierProp:Object;
         var propValue:Number;
         var propPostFix:String;
         var propPrefix:String;
         var style:String;
         var propHintTx:String;
         trace("modifier::",modificatorProps);
         modifier.textValue = "";
         for each(modifierProp in modificatorProps)
         {
            propValue = Number(modifierProp.prop_value);
            if(propValue != Number(modifierProp.prop_value))
            {
               propValue = Util.stringToNumber(modifierProp.prop_value);
            }
            propDescription = this._propsDescriptor.getPropertyDescription(modifierProp.prop_id);
            if((propDescription))
            {
               propPostFix = propDescription.modifier_type == ModifierType.ADD ? propDescription.postfix : "%";
               propPrefix = modifierProp.prop_value >= 0 ? "+" : "";
               style = propDescription.direction == PropertyDirection.LOWER_IS_BETTER && modifierProp.prop_value < 0 || propDescription.direction == PropertyDirection.HIGHER_IS_BETTER && modifierProp.prop_value > 0 ? "green" : "red";
               propValue = propDescription.modifier_type == ModifierType.ADD ? modifierProp.prop_value : modifierProp.prop_value * 100;
               propHintTx = propDescription.prop_name + ": " + Util.styledChar(propPrefix + propValue + ((propPostFix) ? propPostFix : ""),style);
               modifier.textValue += propHintTx + "\n";
            }
         }
      }
      
      public function fillModifiersDescription(value:ItemData) : void
      {
         _propsDescriptor = PropertiesDescriptor.getPropertiesDescriptor("properties");
         var modifiers:Array = value.modifiers;
         trace("fillModifiersDescription::",value.name,value.itemId,modifiers);
         if(!modifiers)
         {
            return;
         }
         var hintText:String = "";
         var props:Object = new Object();
         var originName:String = value.name;
         var propDescription:Object;
         var propPostFix:String;
         var propPrefix:String;
         var style:String;
         for(var i:uint = 0; i < modifiers.length; i++)
         {
            var modificatorProps:Object;
            var modifierProp:Object;
            var propValue:Number;
            var propHintTx:String;
            var propHint:String;
            modificatorProps = modifiers[i].props;
            trace("modifier::",hintText,modificatorProps);
            modifiers[i].textValue = "";
            for each(modifierProp in modificatorProps)
            {
               if(props[modifierProp.prop_id] == undefined)
               {
                  trace("addPropValue::",modifierProp.prop_id,modifierProp.prop_value);
                  props[modifierProp.prop_id] = {
                     prop_id:modifierProp.prop_id,
                     prop_value:0,
                     modifier:modifiers[i]
                  };
               }
               propValue = Number(modifierProp.prop_value);
               if(propValue != Number(modifierProp.prop_value))
               {
                  propValue = Util.stringToNumber(modifierProp.prop_value);
               }
               props[modifierProp.prop_id].prop_value += propValue;
               propDescription = this._propsDescriptor.getPropertyDescription(modifierProp.prop_id);
               if((propDescription))
               {
                  propPostFix = propDescription.modifier_type == ModifierType.ADD ? propDescription.postfix : "%";
                  propPrefix = modifierProp.prop_value >= 0 ? "+" : "";
                  style = propDescription.direction == PropertyDirection.LOWER_IS_BETTER && modifierProp.prop_value < 0 || propDescription.direction == PropertyDirection.HIGHER_IS_BETTER && modifierProp.prop_value > 0 ? "green" : "red";
                  propValue = propDescription.modifier_type == ModifierType.ADD ? modifierProp.prop_value : modifierProp.prop_value * 100;
                  propHintTx = propDescription.prop_name + ": " + Util.styledChar(propPrefix + propValue + ((propPostFix) ? propPostFix : ""),style);
                  modifiers[i].textValue += propHintTx + "\n";
               }
            }
         }
         for each(modifierProp in props)
         {
            propDescription = this._propsDescriptor.getPropertyDescription(modifierProp.prop_id);
            if((propDescription))
            {
               if(propDescription.postfix == undefined)
               {
                  propDescription.postfix = "";
               }
               propPostFix = propDescription.modifier_type == ModifierType.ADD ? propDescription.postfix : "%";
               propPrefix = modifierProp.prop_value >= 0 ? "+" : "";
               if(propDescription.modifier_type == undefined)
               {
                  propDescription.modifier_type = ModifierType.INVALID;
                  propDescription.modifier_type = modifierProp.modifier_type;
               }
               propValue = propDescription.modifier_type == ModifierType.ADD ? modifierProp.prop_value : modifierProp.prop_value * 100;
               propValue = Math.round(propValue / 0.01) * 0.01;
               style = propDescription.direction == PropertyDirection.LOWER_IS_BETTER && modifierProp.prop_value < 0 || propDescription.direction == PropertyDirection.HIGHER_IS_BETTER && modifierProp.prop_value > 0 ? "green" : "red";
               propHint = propDescription.prop_name + ": " + Util.styledChar(propPrefix + propValue + propPostFix,style);
               if(!modifierProp.modifier.textValue)
               {
                  modifierProp.modifier.textValue = propHint;
               }
               hintText += propHint + "\n";
            }
         }
         value.modifiersDescription = hintText;
      }
      
      public function forEachAmmo(value:Function) : void
      {
         var items:uint = this._ammoItems.length;
         trace("IL:forEachAmmo::",items);
         for(var i:uint = 0; i < items; i++)
         {
            if((value(this._ammoItems[i])))
            {
               break;
            }
         }
      }
      
      public function addFakeItems(value:Array) : void
      {
         return;
         return;
      }
      
      private function getItemBroken(value:Array) : Boolean
      {
         if(!value)
         {
            return false;
         }
         var l:uint = value.length;
         for(var i:uint = 0; i < l; i++)
         {
            var modifier:Object;
            modifier = value[i];
            if((modifier.state))
            {
               return true;
            }
         }
         return false;
      }
      
      private function getItemRank(value:Array) : int
      {
         var rank:uint = 0;
         if(!value)
         {
            return rank;
         }
         var l:uint = value.length;
         for(var i:uint = 0; i < l; i++)
         {
            var modifier:Object;
            modifier = value[i];
            if(!modifier.state)
            {
               rank++;
            }
         }
         return rank;
      }
      
      public function updateInventoryItem(itemData:Object) : void
      {
         var item:ItemData = this.getInventoryItem(itemData.id);
         trace("updateInventoryItem::",item,this._ignoreNextAttach,(item) ? item.name : "no_item",itemData.id,itemData.upgrades,"cond::",itemData.condition_or_stack,"modifiers::",itemData.modifiers,"attached::",itemData.is_attached_to,itemData.equipped);
         if(!itemData)
         {
            throw Error("cannotFindItemToUpdate " + itemData + " with id " + ((itemData) ? itemData.id : "null"));
         }
         item.value = itemData.condition_or_stack;
         if(itemData.modifiers != undefined)
         {
            item.broken = this.getItemBroken(itemData.modifiers);
            item.modifiers = (itemData.modifiers as Array).concat(item.modifiersDefault);
         }
         if(itemData.rarity_level != undefined)
         {
            item.rank = itemData.rarity_level;
         }
         if(itemData.double_handed != undefined)
         {
            item.oneHanded = !itemData.double_handed;
         }
         if(item.isWeapon)
         {
            if(!item.ammoText)
            {
               str = "";
               ammoItems = ItemFactory.getItemCombableItemNames(item.dictId);
               if((ammoItems))
               {
                  ammoL = ammoItems.length;
                  for(j = 0; j < ammoL; j++)
                  {
                     descrName = _itemDescriptor.getItemDescription(ammoItems[j]).name;
                     if((descrName))
                     {
                        str += "   " + descrName + "\n";
                     }
                  }
               }
               item.ammoText = str;
            }
         }
         this.updateItemTipText(item);
         if(item.category == ItemCategory.VOUCHERS)
         {
            PlayerInfo.setVoucherCount(item.dictId,item.value,item.type,item.itemId);
         }
         if(itemData.is_attached_to != undefined)
         {
            if(this._ignoreNextAttach == item.itemId && itemData.is_attached_to == 0)
            {
               trace("ignoredAtZero");
               _ignoreNextAttach = 0;
            }
            else
            {
               item.equipped = itemData.is_attached_to;
               item.slotTarget = itemData.is_attached_to;
            }
         }
         item.repairTime = itemData.repair_time;
         item.rerollTime = itemData.reroll_time;
         if((item.repairTime))
         {
            item.enabled = false;
         }
         else
         {
            item.enabled = true;
         }
         if(itemData.equipped != undefined)
         {
            item.equipped = itemData.equipped;
         }
         if(itemData.upgrades == undefined)
         {
            itemData.upgrades = [{id:9003}];
         }
         item.upgrades = itemData.upgrades;
         if((item.upgrades))
         {
            l = (item.upgrades.length);
            for(i = 0; i < l; i++)
            {
               trace("updateItem::upgrades",item.upgrades[i].id);
            }
         }
         this.fillModifiersDescription(item);
         if(mainTabBar.selectedIndex == InventoryTab.INVENTORY)
         {
            supper = optionControl.getSelectedIndexes();
            filterOptions = optionControl.getFilterOptions();
            this.ApplyInventoryFilter(supper,inventoryTabPage.selectedIndex,sellersTabPage.dataProvider[sellersTabPage.selectedIndex].faction_id,filterOptions);
            dispatchEvent(new Event(Event.CHANGE));
         }
         var pp:ILocalPlayerProfile = parent["player_profile"];
         var str:String;
         var ammoItems:Array;
         var ammoL:uint;
         var j:uint;
         var descrName:String;
         var l:int;
         var i:int;
         var supper:Array;
         var filterOptions:Object;
         pp.refillItem(itemData.id);
         trace("itemUpdated");
      }
      
      public function getBundleData(value:int) : Object
      {
         var l:uint = this._bundleItems.length;
         for(var i:uint = 0; i < l; i++)
         {
            var itemData:Object;
            itemData = this._bundleItems[i];
            if(itemData.itemId == value)
            {
               return itemData;
            }
         }
         return null;
      }
      
      public function setupBundleData(p:Array) : void
      {
         _bundleItems = p;
         var l:uint = this._bundleItems.length;
         var iconsContainer:IconContainer = IconContainer.getInstance("bundles");
         var sellerIcons:IconContainer = IconContainer.getInstance("factionItemIcons");
         for(var i:uint = 0; i < l; i++)
         {
            itemData = this._bundleItems[i];
            item = new ItemData();
            if(itemData.icon_row == undefined)
            {
               itemData.icon_row = itemData.bundle_icon % iconsContainer.rows + 1;
               itemData.icon_column = int(itemData.bundle_icon / iconsContainer.rows) + 1;
               trace("fill_bundle_icon",itemData.bundle_icon,"->",itemData.icon_row,itemData.icon_column);
            }
            iconRow = itemData.icon_row - 1;
            iconColumn = itemData.icon_column - 1;
            item.iconId = iconColumn * iconsContainer.rows + iconRow;
            item.icon = iconsContainer.getIcon(item.iconId);
            trace("setupBundleData",iconRow,iconColumn,item.iconId,iconsContainer.rows,"|",itemData.icon_row,itemData.icon_column,itemData.bundle_icon);
            item.modifiers = [];
            item.isStack = false;
            item.dragIcon = item.icon.clone();
            item.messIcon = item.icon.clone();
            item.name = itemData.bundle_name;
            item.itemId = itemData.bundle_id;
            item.dictId = itemData.bundle_id;
            item.itemDescription = itemData.bundle_descr;
            item.properties = itemData.bundle_properties;
            item.isPremium = itemData.currency_type;
            item.cost = itemData.cost;
            item.price = itemData.cost;
            item.sellerIcon = sellerIcons.getIcon(0);
            item.factionId = Faction.NONE;
            item.canBuy = true;
            item.descriptor = ItemPlace.BUNDLE;
            item.category = ItemCategory.BUNDLE;
            item.fromBundle = true;
            if(!itemData.bundle_items)
            {
               itemData.bundle_items = [];
            }
            bundleItems = itemData.bundle_items;
            for(j = 0; j < bundleItems.length; j++)
            {
               newItem = this.createItem(bundleItems[j].dict_id);
               newItem.dragEnabled = false;
               newItem.descriptor = ItemPlace.NONE;
               newItem.canBuy = true;
               if(!newItem.modifiers)
               {
                  newItem.modifiers = [];
               }
               newItem.upgrades = [];
               newItem.sellerIcon = sellerIcons.getIcon(0);
               newItem.value = newItem.isStack ? (bundleItems[j].amount) : ItemCondition.FULL;
               this.fillModifiersDescription(newItem);
               bundleItems[j] = newItem;
            }
            item.content = bundleItems;
            this._bundleItems[i] = item;
         }
         var supper:Array = optionControl.getSelectedIndexes();
         var filterOptions:Object = optionControl.getFilterOptions();
         var seller:int = sellersTabPage.selectedIndex != -1 ? sellersTabPage.dataProvider[sellersTabPage.selectedIndex].faction_id : Faction.NONE;
         var itemData:Object;
         var item:ItemData;
         var iconRow:int;
         var iconColumn:int;
         var bundleItems:Array;
         var j:uint;
         var newItem:ItemData;
         this.ApplyInventoryFilter(supper,inventoryTabPage.selectedIndex,seller,filterOptions);
      }
      
      [Event(name="change",type="flash.events.Event")]
      public function setupInventoryData(p:Array) : void
      {
         trace("setupInventoryData",p.length);
         _inventoryItems = p;
         PlayerInfo.setVoucherCount(PlayerInfo.getVoucherDictId(VoucherType.RENAME),0,VoucherType.RENAME,0);
         PlayerInfo.setVoucherCount(PlayerInfo.getVoucherDictId(VoucherType.DROP_CARD),0,VoucherType.DROP_CARD,0);
         PlayerInfo.setVoucherCount(PlayerInfo.getVoucherDictId(VoucherType.REPAIR),0,VoucherType.REPAIR,0);
         this.addFakeItems(this._inventoryItems);
         if((this._ammoItems))
         {
            this._ammoItems.length = 0;
         }
         _ammoItems = new Array();
         var tempDictionary:Dictionary = new Dictionary(true);
         var foundSold:Boolean = false;
         var l:uint = this._inventoryItems.length;
         for(var i:uint = 0; i < l; i++)
         {
            itemData = this._inventoryItems[i];
            itemId = (itemData.id);
            created = this._inventoryItemsDict[itemId] != null;
            if(!created)
            {
               item = this.createItemData(itemData);
               item.itemId = itemId;
               this._inventoryItemsDict[itemId] = item;
            }
            else
            {
               item = this.getInventoryItem(itemId);
               this.updateItem(item,itemData);
            }
            item.descriptor = ItemPlace.INVENTORY;
            inRepair = item.repairTime != 0;
            item.repairTime = itemData.repair_time;
            item.rerollTime = itemData.reroll_time;
            item.modifierUpgraded = itemData.rerolled_modifier;
            item.value = itemData.condition_or_stack;
            item.equipped = itemData.is_attached_to;
            item.slotTarget = itemData.is_attached_to;
            trace("IV:addItem::",item.name,"rt::",item.repairTime,itemData.repair_time,"| icon:id",item.iconId,item.dictId,itemData.is_attached_to,item.itemId,item.rank,"category::",item.category,"vcat",ItemCategory.VOUCHERS,"cost::",item.cost,itemData.cost,item.icon,item.sellerIcon,item.messIcon);
            if((item.repairTime))
            {
               item.enabled = false;
               timer = QuestWindow.getTimerInstance();
               if(!created || created && !inRepair)
               {
                  item.repairTimeDelta = timer.currentCount;
               }
               trace("updatedRp::",itemData.repair_time,timer.currentCount,item.repairTimeDelta);
            }
            else
            {
               item.enabled = true;
            }
            if((item.rerollTime))
            {
               timer = QuestWindow.getTimerInstance();
               if(!created)
               {
                  item.timeDelta = timer.currentCount;
               }
            }
            if(item.category == ItemCategory.VOUCHERS)
            {
               PlayerInfo.setVoucherCount(item.dictId,item.value,item.type,item.itemId);
            }
            else if(item.isStack && item.superCategory == SuperCategory.AMMO)
            {
               this._ammoItems.push(item);
            }
            tempDictionary[itemId] = item;
            if(itemData.equipped != undefined)
            {
               item.equipped = itemData.equipped;
            }
            item.upgrades = itemData.upgrades;
            this._inventoryItems[i] = item;
            trace("ItemCreated::",item.itemId);
            if((this._cardItem))
            {
               trace("_cardItem.action",this._cardItem.action,this._cardItem.id," == ",this._inventoryItems[i].id);
               if(this._cardItem.action == "buy" && this._cardItem.id == this._inventoryItems[i].dictId)
               {
                  if(this._cardItem.itemsAtStart == 0)
                  {
                     parent["add_notification"]({label:LocalizationData.getName("bought") + " " + this._inventoryItems[i].name});
                     _cardItem = null;
                  }
                  else
                  {
                     this._cardItem.itemsAtStart -= 1;
                  }
               }
               else if(this._cardItem.action == "sell" && this._cardItem.id == this._inventoryItems[i].id)
               {
                  foundSold = true;
               }
            }
         }
         if((!foundSold) && (this._cardItem) && this._cardItem.action == "sell")
         {
            Message.showMessage("ok",LocalizationData.getName("sold") + " " + this._cardItem.name);
         }
         _cardItem = null;
         _inventoryItemsDict = tempDictionary;
         var pp:ILocalPlayerProfile = parent["player_profile"];
         pp.handleUnresolvedItems();
         pp.fillEquippedItems();
         var supper:Array = optionControl.getSelectedIndexes();
         var filterOptions:Object = optionControl.getFilterOptions();
         var itemData:Object;
         var item:ItemData;
         var itemId:int;
         var created:Boolean;
         var inRepair:Boolean;
         var timer:Timer;
         var victory_screen:*;
         this.ApplyInventoryFilter(supper,inventoryTabPage.selectedIndex,sellersTabPage.dataProvider[sellersTabPage.selectedIndex].faction_id,filterOptions);
         dispatchEvent(new Event(Event.CHANGE));
         if((parent))
         {
            victory_screen = parent["victory_screen"];
            if((victory_screen) && (victory_screen.visible))
            {
               victory_screen.updateVoucherCount();
            }
         }
      }
      
      override public function forEachItem(value:Function) : void
      {
         var items:uint = this._inventoryItems.length;
         for(var i:uint = 0; i < items; i++)
         {
            if((value(this._inventoryItems[i])))
            {
               break;
            }
         }
      }
      
      override public function fillSellers(sellers:Array) : void
      {
         trace("fillSellersFromGangShop::",sellers);
         var factions:Array = sellers.concat();
         for(var i:int = 0; i < factions.length; i++)
         {
            factions[i].icon = factions[i].faction_id + 2;
         }
         factions.length = Math.min(Faction.TRADING_FACTIONS,factions.length);
         factions.unshift({
            icon:2,
            faction_id:Faction.NONE,
            name:LocalizationData.getName("non_faction_items")
         });
         sellersTabPage.dataProvider = new DataProvider(factions);
         sellersTabPage.invalidate(InvalidationType.ALL);
         sellersTabPage.buttonWidth = 98;
         sellersTabPage.width = factions.length * sellersTabPage.buttonWidth + 30;
         if(sellersTabPage.selectedIndex != -1)
         {
            this.ApplyInventoryFilter(null,inventoryTabPage.selectedIndex,sellersTabPage.dataProvider[sellersTabPage.selectedIndex].faction_id);
         }
      }
      
      public function tryAutoFill(value:ItemData) : void
      {
         var paperDoll:ILocalPaperDoll = parent["player_profile"].profileItems;
         var pp:ILocalPlayerProfile = parent["player_profile"];
         paperDoll.selectAgeements(value.category,value.dictId,value.oneHanded,false,value.itemId);
         var trgSlt:int = paperDoll.getSlot(value);
         var _relocateArray:Array;
         var _obj:Object;
         var icon:Sprite;
         paperDoll.deselectAgeements();
         trace("tryAutoFill::",trgSlt);
         if(trgSlt != -1)
         {
            if(!value.isStack)
            {
               if(value.enabled)
               {
                  _relocateArray = [{
                     profile:pp.profileId,
                     id:value.itemId,
                     dict_id:value.dictId,
                     sourceSlot:ItemPlace.INVENTORY,
                     targetSlot:trgSlt,
                     count:1
                  }];
                  pp._relocateFunction(_relocateArray);
               }
            }
            else
            {
               _obj = new Object();
               _obj.count = value.value;
               _obj.profile = pp.profileId;
               _obj.id = value.itemId;
               _obj.dict_id = value.dictId;
               _obj.sourceSlot = ItemPlace.INVENTORY;
               _obj.targetSlot = trgSlt;
               _obj.name = value.name;
               _obj.onOK = pp._relocateFunction;
               _obj.is_stack = value.isStack;
               _obj.weight = value.weight;
               _obj.clip_size = value.clipSize;
               icon = value.messIcon;
               _obj.icon = icon;
               Message.showMessage("askr",_obj,true);
            }
         }
      }
      
      public function get_ammo_share() : IAmmoShare
      {
         return null;
      }
      
      public function registerRelocateItem(value:Object) : void
      {
         this._newItemsInfo.push(value);
         for(var i:uint = 0; i < this._inventoryItems.length; i++)
         {
            if(this._inventoryItems[i].dictId == value.uid)
            {
               this._oldItems.push(this._inventoryItems[i].id);
            }
         }
      }
      
      public function scrollToCategory(value:int) : void
      {
         var dataProvider:DataProvider = itemsTileList.dataProvider as DataProvider;
         var l:int = dataProvider.length;
         trace("scrollToCategory::",value,"in",l,"items.");
         for(var i:uint = 0; i < l; i++)
         {
            var itemData:Object;
            itemData = dataProvider[i];
            if((itemData) && itemData.category == value)
            {
               trace("newSL::",i,int(i / itemsTileList.columnCount) * 20);
               itemsTileList.scrollBar.position = int(i / itemsTileList.columnCount) * 20;
               return;
            }
         }
      }
      
      public function selectCategory(value:Array, deselect:Boolean = false, lock:Boolean = false) : void
      {
         trace("selectCategoty:: " + String(value));
         if(!value || this.ignoreSelection)
         {
            return;
         }
         if(!deselect)
         {
            mainTabBar.selectedIndex = InventoryTab.INVENTORY;
         }
         var providers:uint = optionControl.filterList.getDataProviderCount();
         var selectCount:uint = value.length;
         for(var j:uint = 0; j < providers; j++)
         {
            var dataProvider:DataProvider;
            var pLength:uint;
            var i:uint;
            var k:uint;
            var fin:Array;
            var g:uint;
            var h:uint;
            dataProvider = optionControl.filterList.getDataProvider(j);
            pLength = dataProvider.length;
            for(i = 0; i < pLength; i++)
            {
               for(k = 0; k < selectCount; k++)
               {
                  if(dataProvider[i].sucat == value[k])
                  {
                     trace("foundCategoryAtProvider",j);
                     if(deselect)
                     {
                        itemsTileList.selectedIndex = -1;
                     }
                     optionControl.checkbox_all_traders.selected = true;
                     inventoryTabPage.selectedIndex = j + 1;
                     optionControl.filterList.setDataProvider(j);
                     optionControl.filterList.deselectAll();
                     fin = [];
                     for(g = 0; g < pLength; g++)
                     {
                        for(h = 0; h < selectCount; h++)
                        {
                           if(dataProvider[g].sucat == value[h])
                           {
                              fin.push(g);
                           }
                        }
                     }
                     if(lock == false)
                     {
                        optionControl.filterList.selectPart(fin,j);
                     }
                     else
                     {
                        optionControl.filterList.focusPart(fin,j);
                     }
                     optionControl.show();
                     this.onFilterShowed();
                     return;
                  }
               }
            }
         }
      }
      
      public function getItemData(dictId:uint) : Object
      {
         var descrData:Object = _itemDescriptor.getItemDescription(dictId);
         return descrData;
      }
      
      public function getItemDataByDictId(dictId:uint) : ItemData
      {
         for(var i:uint = 0; i < this._inventoryItems.length; i++)
         {
            if(this._inventoryItems[i].dictId == dictId)
            {
               return this._inventoryItems[i];
            }
         }
         return null;
      }
      
      public function getInventoryItem(itemId:uint) : ItemData
      {
         if(this._inventoryItemsDict[itemId] == null)
         {
            stackTrace("Wrong itemId " + itemId);
         }
         return this._inventoryItemsDict[itemId];
      }
      
      public function getItemCount(dictId:uint) : uint
      {
         for(var i:uint = 0; i < this._inventoryItems.length; i++)
         {
            if(this._inventoryItems[i].dictId == dictId)
            {
               return this._inventoryItems[i].value;
            }
         }
         return 0;
      }
      
      public function getItemRenderer(value:int = 0) : DisplayObject
      {
         var dataProvider:DataProvider = itemsTileList.dataProvider as DataProvider;
         var l:int = (dataProvider.length);
         var renderer:*;
         var position:Number = itemsTileList.getActualScrollPosition();
         var totalRenderers:int = itemsTileList.rowCount * itemsTileList.columnCount;
         trace("IL:getItemRenderer::",value,"r:",totalRenderers,position);
         for(var i:uint = 0; i < totalRenderers; i++)
         {
            var itemData:Object;
            renderer = itemsTileList.getRendererAt(i,position);
            trace("getRendererAt",i,position,renderer);
            if(!renderer)
            {
               continue;
            }
            itemData = renderer.data;
            if((itemData))
            {
               trace("getRendererDict",itemData.name,itemData.dictId,value);
            }
            if((itemData) && itemData.dictId == value)
            {
               return renderer;
            }
         }
         return null;
      }
      
      public function hoverItem(value:int = 0) : void
      {
         var dataProvider:DataProvider = itemsTileList.dataProvider as DataProvider;
         var l:int = (dataProvider.length);
         var renderer:IInventoryItemRenderer;
         var position:Number = itemsTileList.getActualScrollPosition();
         var i:uint;
         var itemData:Object;
         trace("IL:hoverItem::",value);
         if(value == 0)
         {
            renderer = itemsTileList.getRendererAt(this._hoverIndex,position) as IInventoryItemRenderer;
            if((renderer))
            {
               renderer.hover(false);
            }
            _hoverIndex = -1;
         }
         else
         {
            for(i = 0; i < l; i++)
            {
               itemData = dataProvider[i];
               if((itemData) && itemData.itemId == value)
               {
                  renderer = itemsTileList.getRendererAt(i,position) as IInventoryItemRenderer;
                  if((renderer))
                  {
                     trace("hoverItem::",renderer,i);
                     renderer.hover();
                     renderer.validateNow();
                     _hoverIndex = i;
                  }
                  return;
               }
            }
         }
      }
      
      public function selectItem(value:int) : void
      {
         var dataProvider:DataProvider = itemsTileList.dataProvider as DataProvider;
         var l:int = (dataProvider.length);
         trace("selectItem::",value,"in",l,"items.");
         for(var i:uint = 0; i < l; i++)
         {
            var itemData:Object;
            itemData = dataProvider[i];
            if((itemData) && itemData.itemId == value)
            {
               trace("newSL::",i);
               itemsTileList.selectedIndex = i;
               return;
            }
         }
      }
      
      public function setPrimaryItem(value:ItemData) : void
      {
         if(!value)
         {
            this.bundleList.visible = false;
            return;
         }
         trace("setPrimaryItem",this._item_control,this.item_compare,value.descriptor,this.bundleList);
         this._item_control.setItemData(value);
         this.itemControlPanel.updateData(value);
      }
      
      public function invalidateList() : void
      {
         itemsTileList.dataProvider.invalidate();
      }
      
      protected function newItemOut(e:ListEvent) : void
      {
      }
      
      protected function newItemOver(e:ListEvent) : void
      {
      }
      
      protected function onItemPress(e:ListEvent) : void
      {
         var value:ItemData = e.itemData as ItemData;
         if(value.descriptor == ItemPlace.BUNDLE)
         {
            trace("bundleList::",this.bundleList.itemId,value.itemId,value.dictId);
            if(this.bundleList.itemId == value.dictId)
            {
               if(this.bundleList.visible)
               {
                  this.bundleList.visible = false;
                  return;
               }
            }
            dispatchEvent(new Event(HIDE_ITEM_HINT));
            this.bundleList.itemId = value.dictId;
            this.bundleList.visible = true;
            TextFieldEx.setVerticalAlign(this.bundleList.textField,TextFieldEx.VALIGN_CENTER);
            this.bundleList.textField.text = value.name;
            trace("bundleList",value.content);
            this.bundleList.list.dataProvider = new DataProvider(value.content);
            this.bundleList.background.height = value.content.length * this.bundleList.list.rowHeight + this.bundleList.list.y + 5;
         }
         else
         {
            this.bundleList.itemId = -1;
            this.bundleList.visible = false;
         }
      }
      
      override protected function onNewIndex(e:ListEvent) : void
      {
         var paperDoll:ILocalPaperDoll;
         if(e.index != -1)
         {
            paperDoll = parent["player_profile"].profileItems;
            trace("clearSelectionByOnNewIndex",e.type);
            if(this.updateItemCard)
            {
               paperDoll.clearSelection();
               this.setPrimaryItem(e.itemData as ItemData);
            }
            this.itemControlPanel.visible = true;
            this.onFilterShowed();
            if((this.bundleList.visible) && (e.itemData) && this.bundleList.itemId != (e.itemData as ItemData).dictId)
            {
               this.bundleList.itemId = -1;
               this.bundleList.visible = false;
            }
         }
         else
         {
            if(!this._allwayShowControlPanel)
            {
            }
            this.bundleList.itemId = -1;
            this.bundleList.visible = false;
            this.onFilterShowed();
         }
      }
      
      protected function fixElement(event:Event = null) : void
      {
         var position:Number = itemsTileList.getActualScrollPosition();
         var item:* = itemsTileList.getRendererAt(itemsTileList.selectedIndex,position);
         if(!item || !item.data)
         {
            return;
         }
         var _data:Object = item.data;
         trace("repairItem::",_data.itemId,_data.icon,_data.messIcon);
         var messageData:ItemData = new ItemData();
         messageData.itemId = _data.itemId;
         messageData.name = _data.name;
         messageData.sellerIcon = (_data.sellerIcon) ? _data.sellerIcon.clone() : null;
         messageData.icon = _data.messIcon;
         messageData.value = _data.value;
         messageData.modifiers = this.getFailModifiers(_data);
         messageData.repairCostRatio = _data.repairCostRatio;
         messageData.cost = _data.cost;
         Message.showMessage("repair",messageData,true);
      }
      
      protected function getFailModifiers(_data:Object) : Array
      {
         var modifiers:Array = _data.modifiers != null ? _data.modifiers : [];
         var modificators:Array = [];
         for(var j:uint = 0; j < modifiers.length; j++)
         {
            var _modifiersIconContainer:IconContainer;
            var icon:Icon;
            if(modifiers[j].state == ModificationState.DEFECT)
            {
               _modifiersIconContainer = IconContainer.getInstance("modifiers");
               icon = _modifiersIconContainer.getIcon(modifiers[j].icon);
               modificators.push({
                  name:modifiers[j].name,
                  props:modifiers[j].props,
                  icon:icon,
                  state:modifiers[j].state,
                  fix_time_general:modifiers[j].fix_time_general,
                  fix_cost_general:modifiers[j].fix_cost_general,
                  fix_cost_gold:modifiers[j].fix_cost_gold
               });
            }
         }
         this.fillModifiersNames(modificators);
         return modificators;
      }
      
      protected function fillModifiersNames(modificators:Array) : void
      {
         for(var i:uint = 0; i < modificators.length; i++)
         {
            var hintText:String;
            var modificatorProps:Object;
            var modifierProp:Object;
            var propDescription:Object;
            var propPostFix:String;
            var propPrefix:String;
            var propValue:Number;
            var style:String;
            hintText = "";
            modificatorProps = modificators[i].props;
            trace("props::",hintText,modificatorProps,this._propsDescriptor);
            for each(modifierProp in modificatorProps)
            {
               propDescription = this._propsDescriptor.getPropertyDescription(modifierProp.prop_id);
               if((propDescription))
               {
                  if(propDescription.postfix == undefined)
                  {
                     propDescription.postfix = "";
                  }
                  propPostFix = propDescription.modifier_type == ModifierType.ADD ? propDescription.postfix : "%";
                  propPrefix = modifierProp.prop_value >= 0 ? "+" : "";
                  if(propDescription.modifier_type == undefined)
                  {
                     propDescription.modifier_type = ModifierType.INVALID;
                     propDescription.modifier_type = modifierProp.modifier_type;
                  }
                  if(propDescription.modifier_type == ModifierType.INVALID)
                  {
                     throw Error("modifier_type at modifier " + modificators[i].name + " for prop_id " + modifierProp.prop_id + " is invalid");
                  }
                  trace("propValue::",propDescription.direction,modifierProp.direction);
                  propValue = propDescription.modifier_type == ModifierType.ADD ? (modifierProp.prop_value) : modifierProp.prop_value * 100;
                  style = propDescription.direction == PropertyDirection.LOWER_IS_BETTER && modifierProp.prop_value < 0 || propDescription.direction == PropertyDirection.HIGHER_IS_BETTER && modifierProp.prop_value > 0 ? "green" : "red";
                  hintText += propDescription.prop_name + ": " + Util.styledChar(propPrefix + (int(propValue) == propValue ? propValue.toString() : propValue.toFixed(1)) + propPostFix,style) + "\n";
               }
            }
            modificators[i].name = hintText;
         }
      }
      
      protected function wearElement(value:ItemData) : void
      {
         trace("wearElement::",value);
         if((value))
         {
            this.tryAutoFill(value);
         }
      }
      
      protected function wearItem(event:Event = null) : void
      {
         var position:Number = itemsTileList.getActualScrollPosition();
         var item:* = itemsTileList.getRendererAt(itemsTileList.selectedIndex,position);
         var value:Object;
         trace("wearItem",item);
         if((item) && (item.data))
         {
            value = item.data;
            this.wearElement(value as ItemData);
         }
      }
      
      protected function buyElement(event:Event = null) : void
      {
         var position:Number = itemsTileList.getActualScrollPosition();
         var item:* = itemsTileList.getRendererAt(itemsTileList.selectedIndex,position);
         var value:Object;
         if((item) && (item.data))
         {
            value = item.data;
            if(value.descriptor == ItemPlace.SHOP && !value.canBuy)
            {
               return;
            }
            this._item_control.onBtnClick1();
         }
      }
      
      protected function upgradeItem(e:Event = null) : void
      {
         this._item_control.upgradeItem();
      }
      
      protected function sellObject(value:ItemData) : void
      {
         var _objData:ItemData = value;
         trace("Try to Sell pr:" + " uid:" + _objData.dictId + " cnt:" + _objData.value + " iss:" + _objData.isStack + " price:" + int(GangShop.getInstance().getItemPrice(_objData.dictId)) + " cost:" + _objData.cost);
         var _obj:Object = new Object();
         if(!_objData.isStack)
         {
            _obj.count = 1;
         }
         else
         {
            _obj.count = int(_objData.value);
         }
         var shopCost:uint = value.cost;
         var itemCost:uint = _objData.isPremium ? (shopCost * PlayerInfo.premiumExchangeRate) : shopCost;
         var sellCost:uint = itemCost / ItemPrice.SELL_DIVISOR * (_objData.isStack ? 1 : _objData.value / ItemCondition.FULL);
         _obj.price = sellCost;
         _obj.id = _objData.itemId;
         _obj.uid = _objData.dictId;
         _obj.sell = true;
         _obj.name = _objData.name;
         _obj.is_stack = _objData.isStack;
         _obj.weight = _objData.weight;
         _obj.isPremium = false;
         if((_objData.clipSize))
         {
            _obj.clip_size = _objData.clipSize;
         }
         _obj.icon = (_objData.icon as Icon).clone();
         Message.showMessage("asks",_obj,true);
      }
      
      protected function onItemUse() : void
      {
         ExternalInterface.call("item_use",this._useItemId);
      }
      
      protected function upgradeElelemnt(event:Event = null) : void
      {
         this.openModifiersWindow();
      }
      
      private function openModifiersWindow() : void
      {
         dispatchEvent(new Event(Events.OPEN_MODIFIERS));
      }
      
      protected function useElement(event:Event = null) : void
      {
         var position:Number = itemsTileList.getActualScrollPosition();
         var item:* = itemsTileList.getRendererAt(itemsTileList.selectedIndex,position);
         if(!item || !item.data || !item.data.enabled)
         {
            return;
         }
         trace("useElement::",item.data.type);
         if(item.data.type == VoucherType.RENAME)
         {
            parent["statusPanel"].showChangeNameMessage();
            return;
         }
         var object:Object = {
            text:LocalizationData.getName("use_item_message"),
            onYes:this.onItemUse,
            label_ok:LocalizationData.getName("btn_use_item"),
            label_cancel:LocalizationData.getName("no")
         };
         Message.showMessage("apply",object);
         _useItemId = item.data.itemId;
      }
      
      protected function craftElement(event:Event = null) : void
      {
         var position:Number = itemsTileList.getActualScrollPosition();
         var item:* = itemsTileList.getRendererAt(itemsTileList.selectedIndex,position);
         if(!item || !item.data || !item.data.enabled)
         {
            return;
         }
         dispatchEvent(new Event(Events.CRAFFT_ITEM));
      }
      
      protected function sellElement(event:Event = null) : void
      {
         var position:Number = itemsTileList.getActualScrollPosition();
         var item:* = itemsTileList.getRendererAt(itemsTileList.selectedIndex,position);
         if(!item || !item.data || !item.data.enabled)
         {
            return;
         }
         this.sellObject(item.data);
      }
      
      override protected function configureInventoryTab() : void
      {
         var itemRendererName:String = "SrvInvRenderer";
         itemsTileList.itemRendererName = itemRendererName;
         super.configureInventoryTab();
         sellersTabPage.addEventListener(IndexEvent.INDEX_CHANGE,this.changeInventoryFilterFromSeller);
         sellersTabPage.doubleClickEnabled = true;
         sellersTabPage.container.doubleClickEnabled = true;
         sellersTabPage.container.addEventListener(MouseEvent.DOUBLE_CLICK,this.onBtnSelectAllClick);
         mainTabBar.addEventListener(IndexEvent.INDEX_CHANGE,this.changeActiveItems);
         mainTabBar.selectedIndex = InventoryTab.INVENTORY;
         mainTabBar.visible = false;
         itemsTileList.addEventListener(MouseEvent.CLICK,this.onMouseItemClick);
         itemsTileList.addEventListener(ListEvent.ITEM_DOUBLE_CLICK,this.onItemDoubleClick);
         trace("configureInventoryTab::",this.shop_lights,this.textField,black_gradient,btnClose,this.sortTab);
         inventoryTabPage.buttonWidth = 95;
         (this.bundleList.list as ScrollingList).itemRendererName = itemRendererName;
         this.itemControlPanel.updateData(null);
         this.bundleList.visible = false;
         itemsTileList.columnWidth = 185;
         itemsTileList.rowHeight = 135;
         this.onFilterShowed();
         sellersTabPage.doubleClickEnabled = true;
         doubleClickEnabled = true;
         sellersTabPage.enabled = false;
         this.checkbox_all_traders.selected = true;
         this.checkbox_all_traders.toggle = true;
         this.textField.defaultTextFormat = this.textField.getTextFormat();
         this.textField.mouseEnabled = false;
         LocalizationData.addTextField(this.textField,"sort");
         black_gradient.height -= 25;
         black_gradient.y += 25;
         black_gradient.alpha = 0.8;
         addChild(black_gradient);
         this.sortTab.itemRendererName = "RadioSharedShort";
         this.sortTab.buttonWidth = 110;
         this.sortTab.y += 5;
         this.sortTab.useRect = false;
         this.sortTab.spacing = 10;
         this.sortTab.setMaxFit(true);
         this.shopTab.visible = false;
         optionControl.addEventListener(Event.RESIZE,this.updateTileListSize);
         this.itemControlPanel.setInvenory(this);
         this.checkboxCompareItems.selected = true;
         this.checkboxCompareItems.addEventListener(ButtonEvent.CLICK,this.showCompareCheckbox);
         LocalizationData.addCallback(this.onLocal,"checkbox_compare_items");
      }
      
      private function onLocal(value:*) : void
      {
         this.checkboxCompareItems.label = LocalizationData.getName("checkbox_compare_items");
         this.checkboxCompareItems.tooltipRendererName = "SrvProfileTipRenderer";
         this.checkboxCompareItems.tipData = {name:LocalizationData.getName("checkbox_compare_items_hint")};
         this.shopTab.dataProvider = new DataProvider([LocalizationData.getName("filter_shop_and_inventory"),LocalizationData.getName("filter_shop"),LocalizationData.getName("filter_inventory")]);
      }
      
      private function showCompareCheckbox(e:ButtonEvent) : void
      {
         AllOW_COMPARE = this.checkboxCompareItems.selected;
      }
      
      private function onItemDoubleClick(e:ListEvent) : void
      {
         trace("onItemDoubleClick::DOUBLE_CLICK");
         if(itemsTileList.enabled == false)
         {
            return;
         }
         var item:ItemData = e.itemData as ItemData;
         var itemAvailable:Boolean = item.descriptor == ItemPlace.INVENTORY;
         var pp:ILocalPlayerProfile;
         var availableSlots:uint;
         var newSlot:int;
         var id:uint;
         var profile:uint;
         var dict_id:uint;
         var targetSlot:uint;
         var _relocateArray:Array;
         if(itemAvailable)
         {
            if((item) && item.usable)
            {
               this.useElement();
            }
            else if((item) && item.isStack == false)
            {
               this.wearElement(item);
            }
            else
            {
               pp = parent["player_profile"];
               availableSlots = pp.paperDoll.hasAgreements(item.category);
               if(availableSlots > 1)
               {
                  (e.itemRenderer as IInventoryItemRenderer).startItemDrag();
               }
               else if(availableSlots == 1)
               {
                  newSlot = pp.paperDoll.getSlot(item,true);
                  trace("newSlot:",newSlot);
                  if(newSlot != -1)
                  {
                     id = item.itemId;
                     profile = (pp.profileId);
                     dict_id = item.dictId;
                     targetSlot = (newSlot);
                     _relocateArray = [{
                        profile:profile,
                        id:id,
                        dict_id:dict_id,
                        sourceSlot:item.descriptor,
                        targetSlot:targetSlot,
                        count:1
                     }];
                     pp._relocateFunction(_relocateArray);
                  }
               }
               else
               {
                  return;
               }
            }
         }
         else
         {
            this.buyElement();
         }
      }
      
      private function onBtnSelectAllClick(e:MouseEvent) : void
      {
         trace("onBtnSelectAllClick::DOUBLE_CLICK");
         optionControl.checkbox_all_traders.selected = true;
         optionControl.dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE));
      }
      
      protected function updateTileListSize(e:Event = null) : void
      {
         this.onFilterShowed();
      }
      
      protected function onFilterShowed(e:Event = null) : void
      {
         var pos:int = Math.max(optionControl.y,217 - 116);
         var newHeight:int = 600;
         itemsTileList.height = newHeight;
         trace("onFilterShowed::",optionControl.y,newHeight,newHeight / itemsTileList.rowHeight,int(newHeight / itemsTileList.rowHeight) * itemsTileList.rowHeight);
         scrollBar.setSize(scrollBar.width,newHeight - 8);
         addChild(black_gradient);
      }
      
      private function isUpgradeable(value:Object) : Boolean
      {
         var cat:int = (value.category);
         return (cat == ItemCategory.BODY_ARMOR || cat == ItemCategory.HELMETS || cat == ItemCategory.PANTS || cat == ItemCategory.RIGS) && ((value.camoAvailable) || (value.decalAvailable)) || UpgradeWindow.isWeaponCategory(cat) && UpgradeWindow.hasScopes(int(value.dictId));
      }
      
      protected function onMouseItemClick(e:MouseEventEx) : void
      {
         var activeItem:DragSlot = e.target as DragSlot;
         trace("onMouseItemClick:::",itemsTileList.selectedIndex,e.target,e.currentTarget,(activeItem) ? activeItem.enabled : "null");
         if(!enabled || e.buttonIdx == MouseEventEx.LEFT_BUTTON || itemsTileList.enabled == false)
         {
            return;
         }
         this.showContextMenu(activeItem);
      }
      
      public function showContextMenu(activeItem:DragSlot) : void
      {
         if(!activeItem || !activeItem.data)
         {
            return;
         }
         var itemData:ItemData = activeItem.data as ItemData;
         var options:Array;
         var itemAvailable:Boolean;
         var fixable:Boolean;
         var rerollable:Boolean;
         var br:Boolean;
         var modifiersLength:uint;
         var i:uint;
         var modifier:Object;
         // A window working on the inventory (craft, upgrade) has its own
         // menu while it is open, whatever the inventory's lock says (the
         // server locks and unlocks it around every buy and sell).
         var craft:MessageCraftItem = this.findCraftWindow();
         var upgrade:UpgradeWindow = this.findUpgradeWindow();
         if(craft != null || upgrade != null)
         {
            this.showWindowMenu(activeItem,craft,upgrade);
            return;
         }
         if(this._inventoryLocked)
         {
            return;
         }
         trace("itemData::",itemData.enabled);
         if((itemData) && (itemData.enabled || this.buyable(itemData)))
         {
            options = [];
            itemAvailable = itemData.descriptor <= ItemPlace.INVENTORY && itemData.enabled;
            if(itemAvailable)
            {
               if(itemData.usable)
               {
                  options.push({
                     name:LocalizationData.getName("btn_use_item"),
                     click_function:this.useElement
                  });
               }
               if(!itemData.isStack && itemData.enabled)
               {
                  if(!itemData.equipped && itemData.superCategory != SuperCategory.UPGRADES)
                  {
                     if(itemData.descriptor == ItemPlace.INVENTORY)
                     {
                        options.push({
                           name:LocalizationData.getName("wear"),
                           click_function:this.wearItem
                        });
                     }
                     else
                     {
                        options.push({
                           name:LocalizationData.getName("drop"),
                           click_function:(activeItem as IPaperDollSlot).dropSlot
                        });
                     }
                  }
                  fixable = false;
                  rerollable = false;
                  br = false;
                  if(itemData.value < ItemCondition.FULL)
                  {
                     fixable = true;
                  }
                  if((itemData.modifiers))
                  {
                     modifiersLength = itemData.modifiers.length;
                     for(i = 0; i < modifiersLength; i++)
                     {
                        modifier = itemData.modifiers[i];
                        if((modifier))
                        {
                           if(!fixable && modifier.state == ModificationState.DEFECT)
                           {
                              fixable = true;
                           }
                           else if(!rerollable && modifier.state == ModificationState.MODIFICATION && !modifier.isDefault)
                           {
                              rerollable = true;
                           }
                           if(fixable && rerollable)
                           {
                              break;
                           }
                        }
                     }
                  }
                  if(fixable)
                  {
                     options.push({
                        name:LocalizationData.getName("fix"),
                        click_function:this.fixElement
                     });
                  }
                  if(rerollable && !itemData.rerollTime && !itemData.broken)
                  {
                     options.push({
                        name:LocalizationData.getName("btn_reroll_label"),
                        click_function:this.upgradeItem
                     });
                  }
                  if(this.isUpgradeable(itemData))
                  {
                     options.push({
                        name:LocalizationData.getName(this.upgradeLabelKey(itemData)),
                        click_function:this.upgradeElelemnt
                     });
                  }
               }
            }
            if(ItemFactory.isItemCraftable(itemData) && itemAvailable)
            {
               options.push({
                  name:LocalizationData.getName("craft"),
                  click_function:this.craftElement
               });
            }
            trace("IL::context:",itemData.name,itemData.canBuy,itemData.descriptor,itemData.enabled,itemData.isPremium,itemData.cost);
            if((itemData) && this.buyable(itemData))
            {
               if(itemData.isStack || !itemData.isStack && (!itemData.rank || itemAvailable))
               {
                  trace("addBuyOPtion");
                  options.push({
                     name:LocalizationData.getName("buy"),
                     click_function:this.buyElement
                  });
               }
            }
            if((itemData.descriptor <= ItemPlace.INVENTORY && itemData.enabled || itemData.isStack) && !this.isShop)
            {
               options.push({
                  name:LocalizationData.getName("sell"),
                  click_function:this.sellElement
               });
            }
            if((options.length))
            {
               SrvContextMenu.show(options,0,0,this);
            }
         }
      }
      
      protected function changeActiveItems(e:Event) : void
      {
         _activeItems = mainTabBar.selectedIndex;
         this.handleLockedWindow();
         this.changeInventoryFilterFromFilters(null);
      }
      
      protected function changeInventoryFilterFromSeller(e:Event) : void
      {
         trace("changeInventoryFilterFromSeller::-1");
         itemsTileList.selectedIndex = -1;
         itemsTileList.validateNow();
         this.changeInventoryFilterFromFilters(null);
      }
      
      override protected function changeInventoryFilterFromFilters(e:Event) : void
      {
         var supper:Array = optionControl.getSelectedIndexes();
         var filterOptions:Object = optionControl.getFilterOptions();
         var allFaction:Boolean = (filterOptions.allTraders);
         trace("fromFilter",supper,"|",sellersTabPage.selectedIndex,allFaction,sellersTabPage.mouseEnabled);
         if(allFaction && sellersTabPage.mouseEnabled)
         {
            sellersTabPage.enabled = false;
         }
         else if(!allFaction && !sellersTabPage.mouseEnabled)
         {
            sellersTabPage.enabled = true;
         }
         if((sellersTabPage.dataProvider[sellersTabPage.selectedIndex]))
         {
            this.ApplyInventoryFilter(supper,inventoryTabPage.selectedIndex,sellersTabPage.dataProvider[sellersTabPage.selectedIndex].faction_id,filterOptions);
         }
      }
      
      override protected function changeInventoryFilter(e:Event) : void
      {
         super.changeInventoryFilter(e);
         var supper:Array = optionControl.getSelectedIndexes();
         optionControl.reset(false);
         var filterOptions:Object = optionControl.getFilterOptions();
         this.ApplyInventoryFilter(supper,inventoryTabPage.selectedIndex,sellersTabPage.dataProvider[sellersTabPage.selectedIndex].faction_id,filterOptions);
      }
      
      protected function sellAll(e:ButtonEvent) : void
      {
         var dataProvider:DataProvider = itemsTileList.dataProvider as DataProvider;
         var itemsCount:int = (dataProvider.length);
         for(var i:uint = 0; i < itemsCount; i++)
         {
            var itemData:ItemData;
            itemData = dataProvider[i] as ItemData;
            if((itemData))
            {
               ExternalInterface.call("sell_ok_clicked",itemData.itemId,uint(1));
            }
         }
      }
      
      protected function checkReskin(e:Event) : void
      {
         if(!sellersTabPage.enabled)
         {
            if(optionControl.checkbox_all_traders.selected)
            {
               optionControl.checkbox_all_traders.selected = false;
               optionControl.dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE));
            }
         }
      }
      
      public function filterItems(value:Array) : void
      {
         _filterItems = value;
         var supper:Array = optionControl.getSelectedIndexes();
         var filterOptions:Object = optionControl.getFilterOptions();
         var seller:int = sellersTabPage.selectedIndex != -1 ? (sellersTabPage.dataProvider[sellersTabPage.selectedIndex].faction_id) : Faction.NONE;
         this.ApplyInventoryFilter(supper,inventoryTabPage.selectedIndex,seller,filterOptions);
      }
      
      public function filterItem(itemDictId:int) : void
      {
         _filterDictId = itemDictId;
         var supper:Array = optionControl.getSelectedIndexes();
         var filterOptions:Object = optionControl.getFilterOptions();
         var seller:int = sellersTabPage.selectedIndex != -1 ? (sellersTabPage.dataProvider[sellersTabPage.selectedIndex].faction_id) : Faction.NONE;
         trace("filterItem::",this._filterDictId,inventoryTabPage.selectedIndex);
         this.ApplyInventoryFilter(supper,this._filterDictId != -1 ? SuperCategory.ALL : (inventoryTabPage.selectedIndex),seller,filterOptions);
         if((itemDictId))
         {
            _filterselectedId = itemsTileList.selectedIndex;
         }
         else
         {
            itemsTileList.selectedIndex = this._filterselectedId;
         }
      }
      
      protected function filterInventoryItems() : void
      {
         var inventoryCount:uint = this._inventoryItems.length;
         var filtered:Object = new Object();
         var filteredItems:Array = [];
         for(var i:uint = 0; i < inventoryCount; i++)
         {
            var itemData:ItemData;
            var isFiltered:Boolean;
            itemData = this._inventoryItems[i];
            isFiltered = !filtered[itemData.dictId] && this._filterItems.indexOf(itemData.dictId) != -1;
            if(isFiltered)
            {
               filtered[itemData.dictId] = true;
               filteredItems.push(itemData);
            }
         }
         filteredItems.sortOn(["buyValue","superCategory","category","dictId","rank","value","broken","factionId"],Array.NUMERIC);
         trace("fillItems::",filteredItems.length);
         itemsTileList.dataProvider = new DataProvider(filteredItems);
         itemsTileList.validateNow();
      }
      
      protected function ApplyInventoryFilter(categoryes:Array = null, category:uint = SuperCategory.ALL, seller:int = Faction.NONE, options:Object = null) : void
      {
         function sortItems(inventory:Array, shop:Array):void
         {
            if(shopFilter)
            {
               shopCount = shop.length;
               for(i = 0; i < shopCount; ++i)
               {
                  itemData = shop[i];
                  if(!inventoryDict[itemData.dictId])
                  {
                     filteredItems.push(itemData);
                  }
               }
            }
            shop.length = 0;
            filteredItems.sortOn(["buyValue",options.sortFirst,"superCategory","category","dictId","rank","value","broken",options.sortSecond,"factionId"],Array.NUMERIC);
         }
         function isEnabled(value:ItemData):Boolean
         {
            if(value.descriptor == ItemPlace.INVENTORY || value.canBuy)
            {
               return true;
            }
            return false;
         }
         function isStacked(value:ItemData):Boolean
         {
            if(value.isStack && value.value == 0 && value.descriptor != ItemPlace.SHOP && value.descriptor != ItemPlace.BUNDLE)
            {
               return false;
            }
            return true;
         }
         function isSeller(data:Object, value:int):Boolean
         {
            var i:int;
            if(value != Faction.ANY)
            {
               return value == seller;
            }
            if(data.sellers is Array)
            {
               for(i = 0; i < data.sellers.length; i++)
               {
                  if(seller == data.sellers[i].faction_id)
                  {
                     return true;
                  }
               }
            }
            return false;
         }
         function checkBaseOptions(value:ItemData):Boolean
         {
            var isWeapon:Boolean = options.weapons == WeaponHands.ALL || ItemFactory.getCategory(value.category) != SuperCategory.WEAPONS || (value.oneHanded && options.weapons == WeaponHands.ONE_HANDED || value.oneHanded == false && options.weapons == WeaponHands.TWO_HANDED);
            return (options.allTraders || isSeller(value,value.factionId)) && isWeapon && (isStacked(value)) && (!options.onlyEnabled || (isEnabled(value)));
         }
         if((this._filterItems))
         {
            this.filterInventoryItems();
            return;
         }
         var showWeapons:Boolean = category == SuperCategory.WEAPONS;
         optionControl.showWeaponsSort(showWeapons);
         var currentItems:Array = this._activeItems == InventoryTab.SHOP ? this._shopItems : this._inventoryItems;
         var sellerId:int;
         var filteredItems:Array;
         stackTrace("ApplyInventoryFilter");
         trace("ApplyInventoryFilter",(currentItems) ? currentItems.length : "no_items","cat::",category,options.onlyEnabled,this._filterDictId,seller);
         if(currentItems == null)
         {
            return;
         }
         filteredItems = new Array();
         var filteredItemsShop:Array = new Array();
         var inventoryCount:uint = this._inventoryItems.length;
         var shopCount:uint;
         var inventoryDict:Dictionary;
         shopCount = this._shopItems.length;
         inventoryDict = new Dictionary();
         var shopDict:Dictionary = new Dictionary();
         var itemData:ItemData;
         var relativeItem:ItemData;
         var i:uint;
         var shopFilter:Boolean;
         sellerId = (options.allTraders) ? Faction.ANY : seller;
         shopFilter = options.shopItems == ShopFilter.SHOP_AND_INVENTORY || options.shopItems == ShopFilter.SHOP;
         var inventoryFilter:Boolean = options.shopItems == ShopFilter.SHOP_AND_INVENTORY || options.shopItems == ShopFilter.INVENTORY;
         if(category == SuperCategory.ALL || this._filterDictId != -1)
         {
            if(this._filterDictId == -1)
            {
               for(i = 0; i < shopCount; i++)
               {
                  itemData = this._shopItems[i];
                  if((checkBaseOptions(itemData)))
                  {
                     filteredItemsShop.push(itemData);
                  }
               }
               this.deleteDoubleItems(filteredItemsShop,sellerId,shopDict);
            }
            if(this._filterDictId == -1)
            {
               for(i = 0; i < this._bundleItems.length; i++)
               {
                  itemData = this._bundleItems[i];
                  if(shopFilter && (sellerId == Faction.ANY || sellerId == Faction.NONE))
                  {
                     filteredItems.push(itemData);
                  }
               }
            }
            if(inventoryFilter)
            {
               for(i = 0; i < inventoryCount; i++)
               {
                  itemData = this._inventoryItems[i];
                  isFiltered = this._filterDictId == -1 ? (checkBaseOptions(itemData)) : this._filterDictId == itemData.dictId;
                  if(isFiltered)
                  {
                     relativeItem = shopDict[itemData.dictId];
                     if((relativeItem))
                     {
                        itemData.canBuy = relativeItem.canBuy;
                        itemData.unlockLevelLabel = relativeItem.unlockLevelLabel;
                        itemData.lockText = relativeItem.lockText;
                        itemData.premiumCost = relativeItem.premiumCost;
                     }
                     if((itemData.isStack || !itemData.rank) && itemData.canBuy)
                     {
                        inventoryDict[itemData.dictId] = true;
                     }
                     filteredItems.push(itemData);
                  }
               }
            }
            sortItems(filteredItems,filteredItemsShop);
            trace("fillItems::",filteredItems.length);
            itemsTileList.dataProvider = new DataProvider(filteredItems);
            itemsTileList.validateNow();
            return;
         }
         var fullCheck:Boolean = !(categoryes == null || categoryes.length == 0);
         var isFiltered:Boolean;
         var j:uint;
         var cat:int;
         if(fullCheck)
         {
            for(i = 0; i < shopCount; i++)
            {
               itemData = this._shopItems[i];
               for(j = 0; j < categoryes.length; j++)
               {
                  cat = (categoryes[j]);
                  if(cat == itemData.category && (checkBaseOptions(itemData)))
                  {
                     filteredItemsShop.push(itemData);
                     break;
                  }
               }
            }
            this.deleteDoubleItems(filteredItemsShop,sellerId,shopDict);
            if(inventoryFilter)
            {
               for(i = 0; i < inventoryCount; i++)
               {
                  itemData = this._inventoryItems[i];
                  for(j = 0; j < categoryes.length; j++)
                  {
                     cat = (categoryes[j]);
                     if(cat == itemData.category && (checkBaseOptions(itemData)))
                     {
                        relativeItem = shopDict[itemData.dictId];
                        if((relativeItem))
                        {
                           itemData.canBuy = relativeItem.canBuy;
                           itemData.unlockLevelLabel = relativeItem.unlockLevelLabel;
                           itemData.lockText = relativeItem.lockText;
                           itemData.premiumCost = relativeItem.premiumCost;
                        }
                        if((itemData.isStack || !itemData.rank) && itemData.canBuy)
                        {
                           inventoryDict[itemData.dictId] = true;
                        }
                        filteredItems.push(itemData);
                        break;
                     }
                  }
               }
            }
         }
         else
         {
            for(i = 0; i < shopCount; i++)
            {
               itemData = this._shopItems[i];
               if(category == ItemFactory.getCategory(itemData.category) && (checkBaseOptions(itemData)))
               {
                  filteredItemsShop.push(itemData);
               }
            }
            this.deleteDoubleItems(filteredItemsShop,sellerId,shopDict);
            if(inventoryFilter)
            {
               for(i = 0; i < inventoryCount; i++)
               {
                  itemData = this._inventoryItems[i];
                  if(category == ItemFactory.getCategory(itemData.category) && (checkBaseOptions(itemData)))
                  {
                     relativeItem = shopDict[itemData.dictId];
                     if((relativeItem))
                     {
                        itemData.canBuy = relativeItem.canBuy;
                        itemData.unlockLevelLabel = relativeItem.unlockLevelLabel;
                        itemData.lockText = relativeItem.lockText;
                        itemData.premiumCost = relativeItem.premiumCost;
                     }
                     if((itemData.isStack || !itemData.rank) && itemData.canBuy)
                     {
                        inventoryDict[itemData.dictId] = true;
                     }
                     filteredItems.push(itemData);
                  }
               }
            }
         }
         sortItems(filteredItems,filteredItemsShop);
         trace("fillItems::",filteredItems.length);
         itemsTileList.dataProvider = new DataProvider(filteredItems);
         itemsTileList.validateNow();
         trace("items:Filled::");
      }
      
      private function upgradeLabelKey(value:Object) : String
      {
         return UpgradeWindow.isWeaponCategory(int(value.category)) ? "scope_window_title" : "btn_upgrade_label";
      }
      
      // The craft window ("Upgrade": two items of a kind make a better one)
      // locks the list. While it is open, the right-click menu has Buy and
      // Sell only, by the list's own rules for them; nothing else (no
      // Upgrade). Locked otherwise: no menu.
      private function showWindowMenu(activeItem:DragSlot, craft:MessageCraftItem, upgrade:UpgradeWindow) : void
      {
         var item:Object = activeItem.data;
         if(item == null || !(item.enabled || this.buyable(item)))
         {
            return;
         }
         var owned:Boolean = item.descriptor <= ItemPlace.INVENTORY;
         var held:Boolean = craft != null ? craft.holds(item.itemId) : item.itemId == upgrade.getItemId();
         var options:Array = [];
         // As the menu's own: Buy for an owned item (the one the window
         // works on too), a stack or an item of no rarity. Sell for every
         // owned item, the window's own too, unless it is the account's last
         // of its kind (a stack always); selling the window's item closes
         // the window, which would name an item no longer owned.
         if(this.buyable(item) && (item.isStack || !item.rank || owned))
         {
            options.push({
               name:LocalizationData.getName("buy"),
               click_function:function(e:* = null):void
               {
                  buyFromCraftMenu(item);
               }
            });
         }
         if(owned && !this.isShop && (item.isStack || this.ownedCopies(item.dictId) > 1))
         {
            options.push({
               name:LocalizationData.getName("sell"),
               click_function:function(e:* = null):void
               {
                  sellItem(item.itemId);
                  if(held)
                  {
                     if(craft != null)
                     {
                        craft.dispatchEvent(new Event("closeCraft"));
                     }
                     else
                     {
                        upgrade.dispatchEvent(new Event(Event.CLOSE));
                     }
                  }
               }
            });
         }
         if(options.length > 0)
         {
            SrvContextMenu.show(options,0,0,craft != null ? craft : upgrade);
         }
      }

      private function findUpgradeWindow() : UpgradeWindow
      {
         var lobby:LobbyMenu = parent as LobbyMenu;
         if(lobby == null || lobby.modifierWindow == null || !lobby.modifierWindow.visible)
         {
            return null;
         }
         return lobby.modifierWindow;
      }
      
      // Whether the shop sells this item now. An item's own canBuy is copied from
      // the shop only when the list is filtered, so one bought since (in the
      // upgrade window, whose list stays as it is) had none and showed no Buy.
      private function buyable(item:Object) : Boolean
      {
         return item.canBuy || GangShop.getInstance().getItemBuyable(item.dictId);
      }

      // How many items of this kind the account owns.
      private function ownedCopies(dictId:uint) : int
      {
         var count:int = 0;
         for each(var owned:ItemData in this._inventoryItemsDict)
         {
            if(owned != null && owned.dictId == dictId && owned.descriptor <= ItemPlace.INVENTORY)
            {
               count++;
            }
         }
         return count;
      }
      
      private function findCraftWindow() : MessageCraftItem
      {
         var lobby:DisplayObjectContainer = parent;
         if(lobby == null)
         {
            return null;
         }
         for(var i:int = 0; i < lobby.numChildren; i++)
         {
            if(lobby.getChildAt(i) is MessageCraftItem)
            {
               return lobby.getChildAt(i) as MessageCraftItem;
            }
         }
         return null;
      }
      
      // As the item card's Buy: by the dictionary id, at the shop's price;
      // the shop names the seller. A buy on an owned item is not asked
      // whether the account wants another.
      private function buyFromCraftMenu(item:Object) : void
      {
         var profile:* = parent != null ? parent["player_profile"] : null;
         if(profile == null)
         {
            return;
         }
         var price:int = item.category == ItemCategory.BUNDLE ? int(item.cost) : int(GangShop.getInstance().getItemPrice(item.dictId));
         GangShop.buyFunction(profile.profileId,item.itemId,item.dictId,item.descriptor,ItemPlace.INVENTORY,1,price,Faction.ANY);
      }
   }
}

