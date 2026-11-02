package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.KeyboardEvent;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.geom.Point;
   import flash.text.TextField;
   import flash.text.TextLineMetrics;
   import flash.ui.Keyboard;
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
   import survarium.data.Relocation;
   import survarium.Util;
   import survarium.constants.ItemRank;
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
   import survarium.item.WeaponMovement;
   import survarium.item.ModificationRolls;
   import survarium.messages.MessageCraftItem;
   import survarium.upgrade.UpgradeWindow;
   import survarium.constants.Faction;
   import survarium.constants.InventoryTab;
   import survarium.constants.KeyCode;
   import survarium.constants.ItemCategory;
   import survarium.constants.ItemCondition;
   import survarium.constants.ItemDefaults;
   import survarium.constants.ItemPlace;
   import survarium.constants.ItemPrice;
   import survarium.constants.ModificationState;
   import survarium.constants.ModifierType;
   import survarium.constants.QuickSlotSize;
   import survarium.constants.ShopFilter;
   import survarium.constants.SuperCategory;
   import survarium.constants.VoucherType;
   import survarium.constants.WeaponHands;
   import survarium.constants.ProfileSlot;
   import survarium.player.LocalPaperDoll;
   
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

      // The item the right-click menu was opened on (a list item or a kit item): its Repair,
      // Modification, Upgrade and Attachments act on it (menuItem).
      private var _menuItem:ItemData = null;

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

      // The craft window's items in its slots (craft-menu.md, "Items in the slots"): left out of
      // its list, still owned.
      private var _craftSlotted:Array = [];
      
      // The search box (docs/notes/item-search.md), always there under the grid: the grid is
      // SEARCH_ROW shorter (the fade at its foot moves up with it), and the box sits in that row,
      // centred between the grid's foot and the item panel's buttons (placeSearch),
      // centred on the list: the friends window's name box SEARCH_WIDE times as wide, its text and
      // height SEARCH_GROW px bigger. All in the list's own units: the lobby scales the list.
      private static const SEARCH_ROW:int = 40;
      
      // where the item panel's buttons start below its y (its art; measured in the game: the
      // panel's bounds reach higher than what shows)
      private static const SEARCH_PANEL_TOP:int = 19;
      
      private static const SEARCH_WIDE:Number = 1.25;
      
      private static const SEARCH_GROW:int = 4;
      
      // a magnifier at the box's right (drawn: the friends search's is part of its window's art):
      // its place from the right, its size, its grey (the empty box's word's), and the text's gap
      // before it
      private static const SEARCH_ICON_RIGHT:int = 10;
      
      private static const SEARCH_ICON_SIZE:int = 13;
      
      private static const SEARCH_ICON_COLOUR:uint = 0xAAAAAA;
      
      private static const SEARCH_ICON_GAP:int = 4;
      
      // A word typed shorter than this matches only as a part of the name; from SEARCH_SPREAD on,
      // its letters in order with others between (at most SEARCH_SPREAD_MOST times its length
      // apart); from SEARCH_TYPO on, also with one letter wrong, missing, extra or swapped, and
      // from SEARCH_TYPO_TWO on with two.
      private static const SEARCH_SPREAD:int = 3;
      
      private static const SEARCH_SPREAD_MOST:int = 3;
      
      private static const SEARCH_TYPO:int = 4;
      
      private static const SEARCH_TYPO_TWO:int = 8;
      
      // A word's match, best first: at the name's start, at a word's start, inside a word, its
      // letters in order (less the further apart), with typos (less for each).
      private static const MATCH_START:int = 400;
      
      // A number as the first word is the item's level first (owner 2026-10-22): an item of that
      // level matches it above any name, so "15" lists the level 15 items first (then the names
      // with 15 in them, as "74" still finds the AK-74).
      private static const MATCH_LEVEL:int = 1000;
      
      // A word naming a kind of item (owner 2026-10-22), or its start from 3 letters on, matches
      // the items of that kind above any name: [the words, the category, or with group the super
      // category]. The words of every language match, whatever the game's language (search words,
      // not texts shown: they are not in the dictionaries).
      private static const MATCH_KIND:int = 450;
      
      private static const SEARCH_KIND_LEAST:int = 3;
      
      private static const SEARCH_KINDS:Array = [
         {words:["gear","equipment","спорядження","снаряжение","экипировка", "oprema","výbava","vybaveni","felszerelés","felszereles","varusteet","рыштунак","экіпіроўка"],group:SuperCategory.EQUIPMENT},
         {words:["items","предмети","предметы","predmeti","předměty","predmety","tárgyak","targyak","esineet","прадметы"],group:SuperCategory.ITEMS},
         {words:["helmet","шолом","шлем","šlem","helma","sisak","kypärä","kypara"],category:ItemCategory.HELMETS},
         {words:["mask","маска","maska","maszk","naamari"],category:ItemCategory.MASKS},
         {words:["body","armour","armor","vest","бронежилет","pancir","vesta","pancíř","páncél","pancel","liivi","panssari","бронекамізэлька"],category:ItemCategory.BODY_ARMOR},
         {words:["rig","розвантаження","разгрузка","prsluk","nosný systém","taktikai mellény","taisteluliivi"],category:ItemCategory.RIGS},
         {words:["gloves","рукавички","перчатки","rukavice","kesztyű","kesztyu","käsineet","kasineet","пальчаткі"],category:ItemCategory.GLOVES},
         {words:["pants","trousers","штани","штаны","pantalone","kalhoty","nadrág","nadrag","housut"],category:ItemCategory.PANTS},
         {words:["boots","черевики","ботинки","čizme","boty","bakancs","saappaat","боты"],category:ItemCategory.BOOTS}];
      
      // SEARCH_KINDS' words as the search compares them (made once)
      private static var _kindForms:Array;
      
      private static const MATCH_WORD:int = 350;
      
      private static const MATCH_PART:int = 300;
      
      private static const MATCH_SPREAD:int = 250;
      
      private static const MATCH_SPREAD_LEAST:int = 200;
      
      private static const MATCH_TYPO:int = 150;
      
      private static const MATCH_TYPO_EACH:int = 25;
      
      // The search compares small letters without accents, and Cyrillic in Latin letters (so that
      // "ak" finds АК-74М and a Ukrainian name finds its Russian spelling); every other letter and
      // digit as it is, anything else separates words.
      private static const SEARCH_LETTERS:Object = {
         "à":"a","á":"a","â":"a","ã":"a","ä":"a","å":"a","ā":"a","ą":"a","Ą":"a",
         "ç":"c","ć":"c","č":"c","Ć":"c","Č":"c",
         "ď":"d","đ":"d","Ď":"d","Đ":"d",
         "è":"e","é":"e","ê":"e","ë":"e","ę":"e","ě":"e","Ę":"e","Ě":"e",
         "ì":"i","í":"i","î":"i","ï":"i",
         "ł":"l","Ł":"l",
         "ñ":"n","ń":"n","ň":"n","Ń":"n","Ň":"n",
         "ò":"o","ó":"o","ô":"o","õ":"o","ö":"o","ø":"o","ő":"o","Ő":"o",
         "ř":"r","Ř":"r",
         "ś":"s","š":"s","Ś":"s","Š":"s","ß":"ss",
         "ù":"u","ú":"u","û":"u","ü":"u","ů":"u","Ů":"u","ű":"u","Ű":"u",
         "ť":"t","Ť":"t",
         "ý":"y","ÿ":"y",
         "ź":"z","ż":"z","ž":"z","Ź":"z","Ż":"z","Ž":"z",
         "а":"a","б":"b","в":"v","г":"g","ґ":"g","д":"d","е":"e","ё":"e","є":"e","ж":"zh","з":"z",
         "и":"i","і":"i","ї":"i","й":"i","к":"k","л":"l","м":"m","н":"n","о":"o","п":"p","р":"r",
         "с":"s","т":"t","у":"u","ў":"u","ф":"f","х":"h","ц":"c","ч":"ch","ш":"sh","щ":"sch","ъ":"",
         "ы":"y","ь":"","э":"e","ю":"yu","я":"ya"
      };
      
      private var _searchBox:SrvTextInput;
      
      private var _searchIcon:Sprite;
      
      // the words typed, as the search compares them; null: no search
      private var _searchWords:Array;
      
      // an item name as the search compares it ([text, its words' starts]), by the name
      private var _searchNames:Dictionary = new Dictionary();

      // The shop's "not owned" check box (makeOwnedToggle).
      private var _ownedToggle:CheckBoxShared;

      // The sort's place on the timeline (its label's x, its choices' x and spacing), the last row's
      // outside the shop (placeLastRow).
      private var _sortLabelX:Number;

      private var _sortTabX:Number;

      private var _sortSpacing:Number;

      // The lobby's inventory list (there is one), for the shop's tiles (owned).
      private static var me:InventoryList;

      public function InventoryList()
      {
         super();
         me = this;
         addEventListener(Event.ADDED_TO_STAGE,this.listenSearchKey);
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
         if(this._ownedToggle != null)
         {
            this._ownedToggle.visible = value;
         }
         this.shopTab.selectedIndex = this.isShop ? ShopFilter.SHOP : ShopFilter.INVENTORY;
         this.itemControlPanel.toggleShop(value);
         if(this._ownedToggle != null)
         {
            this.placeLastRow();
         }
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
            // a closed list (switching Shop and Inventory too) opens again without the search
            this.clearSearch();
            dispatchEvent(new Event(Event.CLOSE));
         }
         var pp:MovieClip = parent["player_profile"];
         trace("II:visible",pp);
         super.visible = value;
         // the character menu (kits, paper doll) on the left in the shop too (owner 2026-10-22)
         pp.visible = value;
         if(!this.isShop)
         {
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
      
      // Whether the account owns an item of this dictionary id: any condition, a stack while it
      // holds any (the buy's "already purchased"; the shop's owned mark, shop-owned-mark.md).
      public function hasItem(dictId:int) : Boolean
      {
         if(this._inventoryItems == null)
         {
            return false;
         }
         for each(var item:Object in this._inventoryItems)
         {
            if(item.dictId == dictId && (!item.isStack || item.value > 0))
            {
               return true;
            }
         }
         return false;
      }

      // ItemRendererInv: the lobby's inventory's hasItem (the shop's owned mark).
      public static function owned(dictId:int) : Boolean
      {
         return me != null && me.hasItem(dictId);
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
         // A golden item keeps its gold background with no descriptor rarity.
         item_data.fromBundle = item_data.rank > 0 || ItemFactory.isGolden(dictId);
         var defModifersCount:uint = item_data.modifiersDefault.length;
         for(var i:uint = 0; i < defModifersCount; i++)
         {
            item_data.modifiersDefault[i].isDefault = true;
         }
         item_data.properties = descrData.item_properties;
         WeaponMovement.expose(item_data);
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
               style = ModificationRolls.style(propDescription,modifierProp.prop_id,ModificationRolls.value(modifierProp));
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
                  style = ModificationRolls.style(propDescription,modifierProp.prop_id,ModificationRolls.value(modifierProp));
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
               style = ModificationRolls.style(propDescription,modifierProp.prop_id,ModificationRolls.value(modifierProp));
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
               // As the server gives it: a golden item with its bonuses.
               newItem.modifiers = ItemFactory.startingModifiers(newItem.dictId).concat(newItem.modifiersDefault);
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
                  _relocateArray = [Relocation.move(pp.profileId,value.itemId,value.dictId,ItemPlace.INVENTORY,trgSlt,1)];
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
      
      // The first item of the inventory of a dictionary id, or null (the weapon kit names its
      // attachments by dictionary id: UpgradeWindow.kitUpgrades).
      public function getInventoryItemOf(dictId:int) : ItemData
      {
         var item:ItemData = null;
         var found:ItemData = null;
         for each(item in this._inventoryItemsDict)
         {
            if(Boolean(item) && int(item.dictId) == dictId && (found == null || item.itemId < found.itemId))
            {
               found = item;
            }
         }
         return found;
      }

      // The dictionary ids of the weapons in the chosen kit's two weapon slots.
      public function kitWeaponDicts() : Array
      {
         var ids:Array = [];
         var pp:ILocalPlayerProfile = parent != null ? parent["player_profile"] as ILocalPlayerProfile : null;
         if(pp == null || pp.paperDoll == null)
         {
            return ids;
         }
         for each(var slot:int in [ProfileSlot.WEAPON_1,ProfileSlot.WEAPON_2])
         {
            var weapon:ItemData = pp.paperDoll.getSlotAt(slot) != null ? pp.paperDoll.getSlotAt(slot).data as ItemData : null;
            if(weapon != null)
            {
               ids.push(weapon.dictId);
            }
         }
         return ids;
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
      
      // The listed sights that fit a hovered weapon look hovered too; -1 ends it (weapon_scopes.md,
      // section 4a).
      private var _linkedHovers:Array = [];

      public function hoverScopesFor(weaponDictId:int = -1) : void
      {
         var dataProvider:DataProvider = itemsTileList.dataProvider as DataProvider;
         var position:Number = itemsTileList.getActualScrollPosition();
         var renderer:IInventoryItemRenderer;
         for each(var linked:int in this._linkedHovers)
         {
            renderer = linked != this._hoverIndex ? itemsTileList.getRendererAt(linked,position) as IInventoryItemRenderer : null;
            if(renderer != null)
            {
               renderer.hover(false);
            }
         }
         _linkedHovers = [];
         if(weaponDictId < 0 || dataProvider == null)
         {
            return;
         }
         for(var i:int = 0; i < dataProvider.length; i++)
         {
            var itemData:Object = dataProvider[i];
            if(itemData != null && itemData.category == ItemCategory.SCOPES && UpgradeWindow.fits(weaponDictId,int(itemData.dictId)))
            {
               renderer = itemsTileList.getRendererAt(i,position) as IInventoryItemRenderer;
               if(renderer != null)
               {
                  renderer.hover();
                  this._linkedHovers.push(i);
               }
            }
         }
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
         // The shop lists no owned item: a kit item chosen there (the character menu,
         // shop-character-menu.md) is the card's item all the same, so its menu acts on it.
         if(this.isShop && this.owns(value))
         {
            this.setPrimaryItem(this._inventoryItemsDict[value]);
         }
      }

      // Whether the account owns this item (by its id).
      public function owns(itemId:uint) : Boolean
      {
         return this._inventoryItemsDict != null && this._inventoryItemsDict[itemId] != null;
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
         var _data:Object = this.menuItem();
         if(_data == null)
         {
            return;
         }
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
                  style = ModificationRolls.style(propDescription,modifierProp.prop_id,ModificationRolls.value(modifierProp));
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
      
      // The menu's Buy: the item the menu was opened on, put on the card and bought as the card's
      // Buy buys it (shop-character-menu.md). Not the list's selected row, as a double click's
      // buyElement: a kit item's menu leaves the selection as it was (in the shop the list holds
      // only the shop's entries, none of them the kit's item), and the menu lists Buy for an item
      // the shop sells at any seller (buyable), a shop entry locked at its own seller too.
      protected function buyMenuItem(event:Event = null) : void
      {
         var item:ItemData = this._menuItem;
         if(item == null || !this.buyable(item))
         {
            return;
         }
         item = this.ownedCopy(item);
         if(this._item_control.getItemData() != item)
         {
            this.setPrimaryItem(item);
         }
         // an owned item's own menu: no "already purchased" question; a shop entry's: the card's Buy
         if(item.descriptor <= ItemPlace.INVENTORY)
         {
            this._item_control.buyFromOwnMenu();
         }
         else
         {
            this._item_control.onBtnClick1();
         }
      }

      // An owned item (a row of the inventory, a kit's slot) as the inventory has it, as the card
      // shows it from the list; any other item (a shop entry, a bundle) itself. Not by its id
      // alone: a shop entry's is 0 and a bundle's is the bundle's id (8 and up), which can be an
      // owned item's too.
      private function ownedCopy(item:ItemData) : ItemData
      {
         var owned:Boolean = item.descriptor <= ItemPlace.INVENTORY && this.owns(item.itemId);
         return owned ? this._inventoryItemsDict[item.itemId] : item;
      }

      protected function upgradeItem(e:Event = null) : void
      {
         if(this.menuItem() == null)
         {
            return;
         }
         this._item_control.upgradeItem();
      }

      // The owned item the menu was opened on, put on the card (Modification, Upgrade and
      // Attachments open for the card's item); null for one not owned. Not the list's selection or
      // the card as they were: in the shop the list holds shop entries (item id 0) and a kit item
      // has no row there, so either can be a shop entry (shop-character-menu.md).
      private function menuItem() : ItemData
      {
         var item:ItemData = this._menuItem != null && this.owns(this._menuItem.itemId) ? this._inventoryItemsDict[this._menuItem.itemId] : null;
         if(item != null && this._item_control.getItemData() != item)
         {
            this.setPrimaryItem(item);
         }
         return item;
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
         if(this.menuItem() == null)
         {
            return;
         }
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
         var item:ItemData = this.menuItem();
         if(item == null || !item.enabled)
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
         this.makeSearch();
         this.makeOwnedToggle();
         LocalizationData.addCallback(this.onLocal,"checkbox_compare_items");
      }

      // The shop's "Hide owned" check box (shop-owned-mark.md): the compare box's symbol
      // (CheckBoxShared; Autobuy's, CheckBoxSharedPD, fills its bitmaps unsmoothed, pixelated once the
      // lobby is scaled to the screen), with Autobuy's label sizing and tooltip; on the compare box's
      // row, at its right (placeLastRow); shown in the shop only. Off when the lobby opens.
      private function makeOwnedToggle() : void
      {
         _ownedToggle = new CheckBoxShared();
         LocalizationData.addButton(this._ownedToggle,"st_srp_shop_not_owned");
         this._ownedToggle.tooltipRendererName = "SrvProfileTipRendererStyled";
         this._ownedToggle.fitLabel();
         this._ownedToggle.x = this.checkboxCompareItems.x;
         this._ownedToggle.y = this.checkboxCompareItems.y;
         this._ownedToggle.visible = this.isShop;
         this._ownedToggle.addEventListener(ButtonEvent.CLICK,this.changeInventoryFilterFromFilters);
         addChild(this._ownedToggle);
         // Above the compare box, below the sort's choices: each takes the presses on itself.
         setChildIndex(this.sortTab,numChildren - 1);
         _sortLabelX = this.textField.x;
         _sortTabX = this.sortTab.x;
         _sortSpacing = this.sortTab.spacing;
      }

      // The window's last row in the shop (shop-owned-mark.md, the owner's of 2026-11-02), left to
      // right: [Comparing items] [Hide owned] Sort [By level] [By price], from the window's margin
      // (the compare box's) to the same margin at its right, the five evenly apart, each as wide as
      // its text: a box or a choice from its square or circle to its label's end. Laid out when the
      // shop opens and when the texts come; outside the shop the row is the timeline's.
      private function placeLastRow() : void
      {
         var sortLabel:TextField = this.textField;
         if(!this.isShop)
         {
            sortLabel.x = this._sortLabelX;
            this.sortTab.x = this._sortTabX;
            this.sortTab.spacing = this._sortSpacing;
            return;
         }
         var compare:Button = this.checkboxCompareItems;
         compare.validateNow();
         this._ownedToggle.validateNow();
         this.sortTab.validateNow();
         var level:Button = this.sortTab.getButtonAt(0);
         var price:Button = this.sortTab.getButtonAt(1);
         var margin:Number = compare.x;
         var widths:Array = [this.textRight(compare.textField) - compare.x,this.textRight(this._ownedToggle.textField) - this._ownedToggle.x,this.textRight(sortLabel) - this.textLeft(sortLabel),this.textRight(level.textField) - this.leftOf(level),this.textRight(price.textField) - this.leftOf(price)];
         var used:Number = 0;
         for each(var w:Number in widths)
         {
            used += w;
         }
         var gap:Number = (background.width - margin * 2 - used) / (widths.length - 1);
         this._ownedToggle.x = Math.round(margin + widths[0] + gap);
         this._ownedToggle.y = compare.y;
         sortLabel.x += Math.round(this._ownedToggle.x + widths[1] + gap - this.textLeft(sortLabel));
         this.sortTab.x += Math.round(this.textRight(sortLabel) + gap - this.leftOf(level));
         // The bar steps by a choice's width (its CLIK width, not its drawn one) and the spacing.
         this.sortTab.spacing = Math.round(widths[3] + gap - level.width);
         this.sortTab.validateNow();
      }

      // A text's ends (its first line), in the list's units.
      private function textLeft(field:TextField) : Number
      {
         var line:TextLineMetrics = field.getLineMetrics(0);
         return globalToLocal(field.localToGlobal(new Point(line.x,0))).x;
      }

      private function textRight(field:TextField) : Number
      {
         var line:TextLineMetrics = field.getLineMetrics(0);
         return globalToLocal(field.localToGlobal(new Point(line.x + line.width,0))).x;
      }

      // A control's left edge (its square or circle), in the list's units.
      private function leftOf(control:DisplayObject) : Number
      {
         return globalToLocal(control.localToGlobal(new Point(0,0))).x;
      }

      // Whether the shop's list leaves out this entry: the box is on and the account owns the item
      // (an entry of the shop's; a bundle never, as its mark).
      private function hiddenAsOwned(item:ItemData) : Boolean
      {
         return this.isShop && this._ownedToggle != null && this._ownedToggle.selected && item.descriptor == ItemPlace.SHOP && this.hasItem(item.dictId);
      }
      
      private function onLocal(value:*) : void
      {
         this.checkboxCompareItems.label = LocalizationData.getName("checkbox_compare_items");
         this.checkboxCompareItems.tooltipRendererName = "SrvProfileTipRenderer";
         this.checkboxCompareItems.tipData = {name:LocalizationData.getName("checkbox_compare_items_hint")};
         this.shopTab.dataProvider = new DataProvider([LocalizationData.getName("filter_shop_and_inventory"),LocalizationData.getName("filter_shop"),LocalizationData.getName("filter_inventory")]);
         this._searchBox.defaultText = LocalizationData.getName("st_srp_inventory_search");
         this._ownedToggle.tipData = {name:LocalizationData.getName("st_srp_shop_not_owned_hint")};
         this.placeLastRow();
      }
      
      // The search box (docs/notes/item-search.md): the friends window's name box, always there,
      // with a magnifier at its right; the grid gives it its row (onFilterShowed).
      private function makeSearch() : void
      {
         _searchBox = new TextInputFriendName();
         var w:int = int(this._searchBox.width * SEARCH_WIDE);
         var h:int = int(this._searchBox.height) + SEARCH_GROW;
         this._searchBox.setSize(w,h);
         var margin:int = SEARCH_ICON_RIGHT + SEARCH_ICON_SIZE + SEARCH_ICON_GAP;
         this._searchBox.getTextFormat().rightMargin = margin;
         this._searchBox.defaultTextFormat.rightMargin = margin;
         this._searchBox.textSize = Number(this._searchBox.getTextFormat().size) + SEARCH_GROW;
         this._searchBox.x = int((background.width - w) / 2);
         this._searchBox.addEventListener(Event.CHANGE,this.onSearchChange);
         this._searchBox.textField.addEventListener(KeyboardEvent.KEY_UP,this.onSearchBoxKey);
         addChild(this._searchBox);
         _searchIcon = this.makeSearchIcon();
         this._searchIcon.x = this._searchBox.x + w - SEARCH_ICON_RIGHT - SEARCH_ICON_SIZE;
         this._searchIcon.addEventListener(MouseEvent.CLICK,this.onSearchIcon);
         addChild(this._searchIcon);
         black_gradient.y -= SEARCH_ROW;
         this.placeSearch();
      }
      
      // The box (and its magnifier) centred in its row: from the grid's foot to the item panel's
      // buttons.
      private function placeSearch() : void
      {
         var top:Number = itemsTileList.y + itemsTileList.height;
         var bottom:Number = this.itemControlPanel.y + SEARCH_PANEL_TOP;
         var h:Number = this._searchBox.height;
         this._searchBox.y = Math.round(top + (bottom - top - h) / 2);
         this._searchIcon.y = this._searchBox.y + Math.round((h - SEARCH_ICON_SIZE) / 2);
      }
      
      private function listenSearchKey(e:Event) : void
      {
         stage.addEventListener(KeyboardEvent.KEY_DOWN,this.onSearchKey,false,0,true);
         stage.addEventListener(KeyboardEvent.KEY_UP,this.onSearchKeyUp,false,0,true);
      }
      
      // A window key (S, I, C) acted on this key press (LobbyMenu.windowKeysOff): it may have just
      // opened this list, and its letter is not the search's. Until the key comes up.
      private var _windowKey:Boolean;
      
      public function windowKeyActed() : void
      {
         _windowKey = true;
      }
      
      private function onSearchKeyUp(e:KeyboardEvent) : void
      {
         _windowKey = false;
      }
      
      // Over an open item list (the shop, the inventory, the craft and upgrade windows' list), while
      // no other text box has the keys and no message or popup is up (the upgrade window aside):
      // - typing a letter, a digit or a key of a letter of another layout gives the box the keys and
      //   puts that key's character at its end. The keys go to the box on the next render (a focus
      //   set here does not hold, and the key's own character is gone by then), so the characters
      //   typed until then are kept here (_searchTyped): the key's charCode, else its letter or
      //   digit;
      // - Ctrl+F gives the box the keys with its text chosen (or chooses it, when it has them).
      // The exe sends Ctrl with each key (ctrlKey); the lobby also keeps whether Ctrl is down
      // (MainScene.isCtrl), which can stay stuck (Ctrl let go outside the game): only typing then
      // is off.
      private function onSearchKey(e:KeyboardEvent) : void
      {
         var ctrl:Boolean = e.ctrlKey || MainScene.isCtrl;
         if((e.keyCode == Keyboard.W || e.keyCode == Keyboard.U) && e.ctrlKey && this.searchFocused())
         {
            this.deleteLeft(e.keyCode == Keyboard.W);
            return;
         }
         var find:Boolean = e.keyCode == KeyCode.F && ctrl;
         if(!find && (ctrl || e.altKey || this._windowKey || !typesSearch(e.keyCode)) || !this.takesTyping())
         {
            return;
         }
         if(this.searchFocused())
         {
            if(find && e.ctrlKey)
            {
               this._searchBox.textField.setSelection(0,this._searchBox.textField.length);
            }
            return;
         }
         if(stage.focus is TextField)
         {
            return;
         }
         if(!find)
         {
            _searchTyped += e.charCode > 32 ? String.fromCharCode(e.charCode) : keyLetter(e.keyCode);
            this._searchBox.addEventListener(Event.RENDER,this.focusTyped);
            stage.invalidate();
            return;
         }
         // Ctrl+F: the keys go to the box on the next render, as the exe sends the F's character
         // after the key (as the chat's box takes them: LobbyChatTab.onRenderFocus)
         this._searchBox.addEventListener(Event.RENDER,this.focusSearch);
         stage.invalidate();
      }
      
      // A key that types a letter or a digit: A to Z, 0 to 9 (the row and the pad), and the keys
      // other layouts put letters on (; = , - . / ` [ \ ] ').
      private static function typesSearch(code:uint) : Boolean
      {
         return code >= 65 && code <= 90 || code >= 48 && code <= 57 || code >= 96 && code <= 105 || code >= 186 && code <= 192 || code >= 219 && code <= 222;
      }
      
      // Whether typing over the list would go to its search (LobbyMenu.windowKeysOff: the exe's
      // window keys then do nothing, as their letters are the search's).
      public function takesTyping() : Boolean
      {
         if(this._searchBox == null || !visible || stage == null)
         {
            return false;
         }
         if(Message.isAnyMessage() != -1 || MainScene.activePop != null && MainScene.activePop != this.findUpgradeWindow())
         {
            return false;
         }
         return !(stage.focus is TextField) || this.searchFocused();
      }
      
      // In the box (owner 2026-10-22): Ctrl+W deletes the word before the caret, with the spaces
      // after it; Ctrl+U everything before the caret.
      private function deleteLeft(word:Boolean) : void
      {
         var field:TextField = this._searchBox.textField;
         var text:String = this._searchBox.text;
         var end:int = Math.min(field.caretIndex,text.length);
         var start:int = word ? end : 0;
         while(start > 0 && text.charAt(start - 1) == " ")
         {
            start--;
         }
         while(start > 0 && text.charAt(start - 1) != " ")
         {
            start--;
         }
         if(start == end)
         {
            return;
         }
         this._searchBox.text = text.substring(0,start) + text.substring(end);
         this._searchBox.validateNow();
         field.setSelection(start,start);
         this.onSearchChange(null);
      }
      
      // the characters typed before the box had the keys (onSearchKey)
      private var _searchTyped:String = "";
      
      // A letter key's lower case letter, a digit key's digit; "" for the others.
      private static function keyLetter(code:uint) : String
      {
         if(code >= 65 && code <= 90)
         {
            return String.fromCharCode(code + 32);
         }
         if(code >= 48 && code <= 57)
         {
            return String.fromCharCode(code);
         }
         if(code >= 96 && code <= 105)
         {
            return String.fromCharCode(code - 48);
         }
         return "";
      }
      
      private function focusTyped(e:Event) : void
      {
         this._searchBox.removeEventListener(Event.RENDER,this.focusTyped);
         var typed:String = this._searchTyped;
         _searchTyped = "";
         if(!visible || stage == null)
         {
            return;
         }
         stage.focus = this._searchBox.textField;
         if(typed != "")
         {
            this._searchBox.text += typed;
            this._searchBox.validateNow();
            this.onSearchChange(null);
         }
         this._searchBox.textField.setSelection(this._searchBox.textField.length,this._searchBox.textField.length);
      }
      
      private function focusSearch(e:Event) : void
      {
         this._searchBox.removeEventListener(Event.RENDER,this.focusSearch);
         if(!visible || stage == null)
         {
            return;
         }
         stage.focus = this._searchBox.textField;
         this._searchBox.textField.setSelection(0,this._searchBox.textField.length);
      }
      
      // Whether the search box has the keys (LobbyMenu.close_active_window: Esc clears it then).
      public function searchFocused() : Boolean
      {
         return this._searchBox != null && stage != null && (stage.focus == this._searchBox.textField || stage.focus == this._searchBox);
      }
      
      // Esc in the box, or the list closing: the box empty, without the keys, and the list shows
      // everything again.
      public function clearSearch() : void
      {
         if(this._searchBox == null)
         {
            return;
         }
         if(this.searchFocused())
         {
            stage.focus = this;
         }
         if(this._searchBox.text != "")
         {
            this._searchBox.text = "";
         }
         if(this._searchWords != null)
         {
            _searchWords = null;
            this.applySearch();
         }
      }
      
      // A magnifier SEARCH_ICON_SIZE square: a ring (two circles, even-odd) and a handle down to the
      // right, fills only (a line style would draw a stray line from the pen).
      private function makeSearchIcon() : Sprite
      {
         var icon:Sprite = new Sprite();
         var r:Number = SEARCH_ICON_SIZE * 0.36;
         var c:Number = r + 0.5;
         icon.graphics.beginFill(SEARCH_ICON_COLOUR);
         icon.graphics.drawCircle(c,c,r);
         icon.graphics.drawCircle(c,c,r - 1.6);
         icon.graphics.endFill();
         var a:Number = c + r * 0.7;
         var b:Number = SEARCH_ICON_SIZE - 0.5;
         var t:Number = 1.1;
         icon.graphics.beginFill(SEARCH_ICON_COLOUR);
         icon.graphics.moveTo(a - t,a + t);
         icon.graphics.lineTo(a + t,a - t);
         icon.graphics.lineTo(b + t,b - t);
         icon.graphics.lineTo(b - t,b + t);
         icon.graphics.lineTo(a - t,a + t);
         icon.graphics.endFill();
         // the whole square takes the click
         icon.graphics.beginFill(0,0.01);
         icon.graphics.drawRect(0,0,SEARCH_ICON_SIZE,SEARCH_ICON_SIZE);
         icon.graphics.endFill();
         icon.buttonMode = true;
         return icon;
      }
      
      // the magnifier gives the box the keys
      private function onSearchIcon(e:MouseEvent) : void
      {
         if(stage != null)
         {
            stage.focus = this._searchBox.textField;
         }
      }
      
      // Enter leaves the box: the list stays as found, and the lobby's keys work again.
      private function onSearchBoxKey(e:KeyboardEvent) : void
      {
         if(e.keyCode == Keyboard.ENTER && this.searchFocused())
         {
            stage.focus = this;
         }
      }
      
      private function onSearchChange(e:Event) : void
      {
         var words:Array = searchWords(this._searchBox.text);
         if(words == null && this._searchWords == null)
         {
            return;
         }
         _searchWords = words;
         this.applySearch();
      }
      
      // The craft window's slots hold these items: its list leaves them out.
      public function hideCraftSlotted(itemIds:Array) : void
      {
         _craftSlotted = itemIds;
         if(this._filterDictId != -1)
         {
            this.applySearch();
         }
      }

      // The list again with the search's words (as filterItem asks it), the item chosen kept chosen
      // while it is still listed, from the top.
      private function applySearch() : void
      {
         var dataProvider:DataProvider = itemsTileList.dataProvider as DataProvider;
         var chosen:Object = dataProvider != null && itemsTileList.selectedIndex >= 0 && itemsTileList.selectedIndex < dataProvider.length ? dataProvider[itemsTileList.selectedIndex] : null;
         var supper:Array = optionControl.getSelectedIndexes();
         var filterOptions:Object = optionControl.getFilterOptions();
         var seller:int = sellersTabPage.selectedIndex != -1 ? (sellersTabPage.dataProvider[sellersTabPage.selectedIndex].faction_id) : Faction.NONE;
         this.ApplyInventoryFilter(supper,this._filterDictId != -1 ? SuperCategory.ALL : (inventoryTabPage.selectedIndex),seller,filterOptions);
         dataProvider = itemsTileList.dataProvider as DataProvider;
         var index:int = -1;
         for(var i:int = 0; chosen != null && dataProvider != null && i < dataProvider.length; i++)
         {
            if(dataProvider[i] == chosen)
            {
               index = i;
               break;
            }
         }
         itemsTileList.selectedIndex = index;
         itemsTileList.scrollBar.position = 0;
      }
      
      // The items of a list whose name matches every word typed, best first, in the list's own order
      // among equals; the whole list without a search. The loose matches (letters spread, typos)
      // only when nothing matches otherwise (owner 2026-10-22: "mask" listed Master's gloves).
      private function searchItems(items:Array) : Array
      {
         if(this._searchWords == null)
         {
            return items;
         }
         var found:Array = [];
         var exact:Boolean = false;
         for(var i:int = 0; i < items.length; i++)
         {
            var score:int = this.searchScore(items[i]);
            if(score > 0)
            {
               exact ||= !this._searchLoose;
               found.push({
                  item:items[i],
                  score:score,
                  index:i,
                  loose:this._searchLoose
               });
            }
         }
         if(exact)
         {
            found = found.filter(function(f:Object, i:int, a:Array):Boolean
            {
               return !f.loose;
            });
         }
         found.sort(byScore);
         for(i = 0; i < found.length; i++)
         {
            found[i] = found[i].item;
         }
         return found;
      }
      
      private static function byScore(a:Object, b:Object) : int
      {
         return b.score != a.score ? b.score - a.score : a.index - b.index;
      }
      
      // whether the last item searchScore matched took a loose match (spread letters, typos)
      private var _searchLoose:Boolean;
      
      // An item against the words typed: the sum of each word's match (its name's; a number first,
      // its level's too), 0 when one does not match.
      private function searchScore(item:Object) : int
      {
         _searchLoose = false;
         var text:String = item.name == null ? "" : String(item.name);
         var form:Array = this._searchNames[text] as Array;
         if(form == null)
         {
            form = searchForm(text);
            this._searchNames[text] = form;
         }
         var total:int = 0;
         for(var i:int = 0; i < this._searchWords.length; i++)
         {
            var word:String = this._searchWords[i];
            var score:int = i == 0 && /^[0-9]+$/.test(word) && item.itemLevel != null && int(item.itemLevel) == int(word) ? MATCH_LEVEL : (kindOf(word,item) ? MATCH_KIND : wordScore(word,form[0],form[1]));
            if(score == 0)
            {
               return 0;
            }
            _searchLoose ||= score < MATCH_PART;
            total += score;
         }
         return total;
      }
      
      // Whether a word typed names the item's kind (SEARCH_KINDS).
      private static function kindOf(word:String, item:Object) : Boolean
      {
         if(word.length < SEARCH_KIND_LEAST)
         {
            return false;
         }
         if(_kindForms == null)
         {
            _kindForms = [];
            for each(var kind:Object in SEARCH_KINDS)
            {
               var forms:Array = [];
               for each(var name:String in kind.words)
               {
                  forms.push(searchForm(name)[0]);
               }
               _kindForms.push(forms);
            }
         }
         for(var i:int = 0; i < SEARCH_KINDS.length; i++)
         {
            var k:Object = SEARCH_KINDS[i];
            if(k.group != null ? item.superCategory != k.group : item.category != k.category)
            {
               continue;
            }
            for each(var f:String in _kindForms[i])
            {
               if(f.indexOf(word) == 0)
               {
                  return true;
               }
            }
         }
         return false;
      }
      
      // The words typed as the search compares them; null when there are none.
      private static function searchWords(text:String) : Array
      {
         var form:Array = searchForm(text == null ? "" : text);
         var letters:String = form[0];
         var starts:Array = form[1];
         var words:Array = [];
         for(var i:int = 0; i < starts.length; i++)
         {
            var word:String = letters.substring(starts[i],i + 1 < starts.length ? starts[i + 1] : letters.length);
            if(word.length > 0)
            {
               words.push(word);
            }
         }
         return words.length > 0 ? words : null;
      }
      
      // A text as the search compares it: [its letters and digits (SEARCH_LETTERS), where each
      // word starts in them].
      private static function searchForm(text:String) : Array
      {
         var letters:String = "";
         var starts:Array = [];
         var inWord:Boolean = false;
         for(var i:int = 0; i < text.length; i++)
         {
            var code:int = text.charCodeAt(i);
            if(code >= 0x41 && code <= 0x5A || code >= 0xC0 && code <= 0xDE && code != 0xD7 || code >= 0x410 && code <= 0x42F)
            {
               code += 0x20;
            }
            else if(code >= 0x400 && code <= 0x40F)
            {
               code += 0x50;
            }
            else if(code == 0x490)
            {
               code = 0x491;
            }
            var letter:String = String.fromCharCode(code);
            var latin:* = SEARCH_LETTERS[letter];
            if(latin == null)
            {
               var digitOrLetter:Boolean = code >= 0x30 && code <= 0x39 || code >= 0x61 && code <= 0x7A || code >= 0xC0 && code != 0xD7 && code != 0xF7 && !(code >= 0x2000 && code <= 0x206F) && code != 0x2116;
               if(!digitOrLetter)
               {
                  inWord = false;
                  continue;
               }
               latin = letter;
            }
            if(!inWord)
            {
               starts.push(letters.length);
               inWord = true;
            }
            letters += latin;
         }
         return [letters,starts];
      }
      
      // One word typed against a name's letters (MATCH_*); 0: no match.
      private static function wordScore(word:String, letters:String, starts:Array) : int
      {
         var at:int = letters.indexOf(word);
         if(at == 0)
         {
            return MATCH_START;
         }
         if(at > 0)
         {
            for(; at != -1; at = letters.indexOf(word,at + 1))
            {
               if(starts.indexOf(at) != -1)
               {
                  return MATCH_WORD;
               }
            }
            return MATCH_PART;
         }
         if(word.length >= SEARCH_SPREAD)
         {
            var span:int = spreadSpan(word,letters);
            if(span != -1 && span <= word.length * SEARCH_SPREAD_MOST)
            {
               return Math.max(MATCH_SPREAD_LEAST,MATCH_SPREAD - (span - word.length));
            }
         }
         if(word.length >= SEARCH_TYPO)
         {
            var typos:int = typoCount(word,letters,word.length >= SEARCH_TYPO_TWO ? 2 : 1);
            if(typos != -1)
            {
               return MATCH_TYPO - typos * MATCH_TYPO_EACH;
            }
         }
         return 0;
      }
      
      // The shortest stretch of the letters that holds the word's letters in order; -1: none does.
      private static function spreadSpan(word:String, letters:String) : int
      {
         var best:int = -1;
         var first:String = word.charAt(0);
         for(var from:int = letters.indexOf(first); from != -1; from = letters.indexOf(first,from + 1))
         {
            var at:int = from;
            for(var k:int = 1; k < word.length && at != -1; k++)
            {
               at = letters.indexOf(word.charAt(k),at + 1);
            }
            if(at == -1)
            {
               // a later start finds the rest no sooner
               break;
            }
            if(best == -1 || at - from + 1 < best)
            {
               best = at - from + 1;
            }
         }
         return best;
      }
      
      // The fewest typos (a letter wrong, missing, extra, or two swapped) that make the word a part
      // of the letters, when at most `most`; -1 otherwise.
      private static function typoCount(word:String, letters:String, most:int) : int
      {
         var m:int = word.length;
         var n:int = letters.length;
         var i:int;
         var j:int;
         var missing:int = 0;
         for(i = 0; i < m; i++)
         {
            if(letters.indexOf(word.charAt(i)) == -1 && ++missing > most)
            {
               return -1;
            }
         }
         // rows of the edit distance from the word's first i letters to a part of the letters
         // ending at j (a part may start anywhere: row 0 is all 0)
         var before:Array = new Array(n + 1);
         var last:Array = new Array(n + 1);
         var row:Array = new Array(n + 1);
         for(j = 0; j <= n; j++)
         {
            last[j] = 0;
         }
         for(i = 1; i <= m; i++)
         {
            row[0] = i;
            var least:int = i;
            var w:int = word.charCodeAt(i - 1);
            for(j = 1; j <= n; j++)
            {
               var t:int = letters.charCodeAt(j - 1);
               var d:int = last[j - 1] + (w == t ? 0 : 1);
               if(last[j] + 1 < d)
               {
                  d = last[j] + 1;
               }
               if(row[j - 1] + 1 < d)
               {
                  d = row[j - 1] + 1;
               }
               if(i > 1 && j > 1 && w == letters.charCodeAt(j - 2) && word.charCodeAt(i - 2) == t && before[j - 2] + 1 < d)
               {
                  d = before[j - 2] + 1;
               }
               row[j] = d;
               if(d < least)
               {
                  least = d;
               }
            }
            if(least > most)
            {
               return -1;
            }
            var spare:Array = before;
            before = last;
            last = row;
            row = spare;
         }
         var result:int = most + 1;
         for(j = 0; j <= n; j++)
         {
            if(last[j] < result)
            {
               result = last[j];
            }
         }
         return result <= most ? result : -1;
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
            // the upgrade window open: a camouflage, a decal or a scope goes into its slot there,
            // as a drop on it would
            if(UpgradeWindow.attach(item))
            {
               return;
            }
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
                     _relocateArray = [Relocation.move(profile,id,dict_id,item.descriptor,targetSlot,newSlot >= ProfileSlot.QUICK_1 && newSlot <= ProfileSlot.QUICK_6 ? LocalPaperDoll.pocketUnits(pp.paperDoll.getSlotAt(newSlot).data as ItemData,item) : 1)];
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
         var newHeight:int = 600 - SEARCH_ROW;
         itemsTileList.height = newHeight;
         trace("onFilterShowed::",optionControl.y,newHeight,newHeight / itemsTileList.rowHeight,int(newHeight / itemsTileList.rowHeight) * itemsTileList.rowHeight);
         scrollBar.setSize(scrollBar.width,newHeight - 8);
         addChild(black_gradient);
         if(this._searchBox != null)
         {
            this.placeSearch();
            setChildIndex(this._searchBox,numChildren - 1);
            setChildIndex(this._searchIcon,numChildren - 1);
         }
         if(this._ownedToggle != null)
         {
            // Above the compare box, below the sort's choices: each takes the presses on itself.
            setChildIndex(this._ownedToggle,numChildren - 1);
            setChildIndex(this.sortTab,numChildren - 1);
         }
      }
      
      private function isUpgradeable(value:Object) : Boolean
      {
         var cat:int = (value.category);
         return (cat == ItemCategory.BODY_ARMOR || cat == ItemCategory.HELMETS || cat == ItemCategory.PANTS || cat == ItemCategory.RIGS) && ((value.camoAvailable) || (value.decalAvailable)) || UpgradeWindow.isUpgradeableWeapon(value);
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
         var owned:Boolean;
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
         _menuItem = itemData;
         if((itemData) && (itemData.enabled || this.buyable(itemData)))
         {
            options = [];
            itemAvailable = itemData.descriptor <= ItemPlace.INVENTORY && itemData.enabled;
            // Repair, Modification, Upgrade and Attachments: only for an item the account owns
            // (the server refuses any other)
            owned = this.owns(itemData.itemId);
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
                  if(fixable && owned)
                  {
                     options.push({
                        name:LocalizationData.getName("fix"),
                        click_function:this.fixElement
                     });
                  }
                  if(owned && rerollable && !itemData.rerollTime && !itemData.broken)
                  {
                     options.push({
                        name:LocalizationData.getName("btn_reroll_label"),
                        click_function:this.upgradeItem
                     });
                  }
                  if(owned && this.isUpgradeable(itemData))
                  {
                     options.push({
                        name:LocalizationData.getName(this.upgradeLabelKey(itemData)),
                        click_function:this.upgradeElelemnt
                     });
                  }
               }
            }
            if(ItemFactory.isItemCraftable(itemData) && itemAvailable && owned)
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
                     click_function:this.buyMenuItem
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
         if(itemDictId == -1)
         {
            _craftSlotted = [];
         }
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
            return (options.allTraders || isSeller(value,value.factionId)) && isWeapon && (isStacked(value)) && (!options.onlyEnabled || (isEnabled(value))) && !leftOutAsOwned(value);
         }
         var leftOutAsOwned:Function = this.hiddenAsOwned;
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
                  // The craft's donors: the server's (a golden item also takes its plain item); not
                  // one in the window's slots, nor one at the most modifications (craft-menu.md).
                  isFiltered = this._filterDictId == -1 ? (checkBaseOptions(itemData)) : ItemFactory.isCraftDonor(itemData.dictId,this._filterDictId) && this._craftSlotted.indexOf(itemData.itemId) == -1 && itemData.rank < ItemRank.MAX;
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
            filteredItems = this.searchItems(filteredItems);
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
               if(category == ItemFactory.filterGroup(itemData.category) && (checkBaseOptions(itemData)))
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
                  if(category == ItemFactory.filterGroup(itemData.category) && (checkBaseOptions(itemData)))
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
         filteredItems = this.searchItems(filteredItems);
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

      // The craft window's slots (the item being upgraded, the donor): a click gives Buy, by the
      // list's rule for an owned item, to buy another of the kind without leaving the window
      // (craft-menu.md, "Buy from the slots"). No Sell: the slot's item is the craft's. Not sold:
      // no menu, as in the list.
      public function showCraftSlotMenu(item:Object, craft:MessageCraftItem) : void
      {
         if(item == null || !this.buyable(item))
         {
            return;
         }
         SrvContextMenu.show([{
            name:LocalizationData.getName("buy"),
            click_function:function(e:* = null):void
            {
               buyFromCraftMenu(item);
            }
         }],0,0,craft);
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
      // the shop names the seller. An item owned already is asked about
      // first, as every buy (LobbyMenu.try_buy_item).
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

