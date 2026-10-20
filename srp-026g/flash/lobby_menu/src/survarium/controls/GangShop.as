package survarium.controls
{
   import flash.events.Event;
   import flash.utils.Dictionary;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.core.UIComponent;
   import survarium.Util;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.item.ItemFactory;
   
   [Event(name="NEW_ITEMS_UNCLOKED",type="flash.events.Event")]
   public class GangShop extends UIComponent
   {
      
      public static var buyFunction:Function;
      
      public static const NEW_ITEMS_COUNT_CHANGED:String = "newItemsCountChanged";
      
      private static var id:GangShop;
      
      protected static var _sellers:Array = new Array();
      
      protected var _inventory:InventoryList;
      
      protected var _reputationContainer:QuestWindow;
      
      protected var _levels:Array = [];
      
      protected var _names:Array = [[],[],[],[],[],[],[]];
      
      protected var _items:Array = [];
      
      protected var _reputations:Array = [];
      
      protected var _currentItems:Array;
      
      protected var _currentProgress:Array = [0,0,0,0,0,0];
      
      protected var _unlockedLevel:Array = [0,0,0,0,0,0];
      
      protected var _itemsDict:Dictionary = new Dictionary();
      
      protected var _itemsBuyDict:Dictionary = new Dictionary();
      
      protected var _itemDescriptor:*;
      
      public var btn_buy:Button;
      
      protected var _factionBonus:Array = [];
      
      protected var _ammoItems:Array;
      
      protected var _new_objects:Array = [];
      
      protected var _newItems:uint = 0;
      
      protected var disabled:Array = ["","","","",""];
      
      protected var _initedSeller:Object = new Object();
      
      public function GangShop()
      {
         super();
         id = this;
      }
      
      public static function getInstance() : GangShop
      {
         return id;
      }
      
      public static function getSellerName(value:uint) : String
      {
         var sellerName:String = "noSuchSeller";
         for(var j:int = 0; j < _sellers.length; j++)
         {
            if(_sellers[j].faction_id == value)
            {
               sellerName = _sellers[j].name;
               break;
            }
         }
         return sellerName;
      }
      
      public static function getSellerRank(value:uint) : String
      {
         var unlockedLevel:int = (id._unlockedLevel[value]);
         return id._names[value][unlockedLevel];
      }
      
      public static function getUnlockedItems(seller:uint, level:uint) : Array
      {
         trace("getUnlockedItems::",seller,level);
         var sellerItems:Array = id._items[seller];
         if((sellerItems))
         {
            var newItems:Array = [];
            var sellerItemsCount:uint = sellerItems.length;
            for(var k:uint = 0; k < sellerItemsCount; k++)
            {
               var item:ItemData = sellerItems[k];
               if(item.sellerLevel == level)
               {
                  newItems.push(item);
               }
            }
            return newItems;
         }
         return [];
      }
      
      public function registerItemDescriptor(value:Object) : void
      {
         _itemDescriptor = value;
      }
      
      public function setFactionBonus(value:Array) : void
      {
         _factionBonus = value;
         invalidateData();
      }
      
      public function forEachItem(value:Function) : void
      {
         for(var i:uint = 0; i < this._items.length; i++)
         {
            if((this._items[i]))
            {
               for(var k:uint = 0; k < this._items[i].length; k++)
               {
                  if((value(this._items[i][k])))
                  {
                     return;
                  }
               }
            }
         }
      }
      
      public function setInventory(value:InventoryList) : void
      {
         _inventory = value;
      }
      
      public function forEachAmmo(value:Function) : void
      {
         var items:uint = this._ammoItems.length;
         trace("GS:forEachAmmo::",items);
         for(var i:uint = 0; i < items; i++)
         {
            if((value(this._ammoItems[i])))
            {
               break;
            }
         }
      }
      
      public function setupShopItems(p:Array) : void
      {


         trace("setupShopItems::",p.length);
         var objects:Array = [];
         if((this._ammoItems))
         {
            this._ammoItems.length = 0;
         }
         _ammoItems = new Array();
         var l:uint = p.length;
         for(var i:uint = 0; i < l; i++)
         {
            var item:ItemData = this.createItem(p[i].dictId);
            if(item.isStack && item.superCategory == 3)
            {
               this._ammoItems.push(item);
            }
            var seller:int = p[i].faction_id != undefined ? (p[i].faction_id) : (p[i].seller);
            if(p[i].double_handed != undefined)
            {
               item.oneHanded = !p[i].double_handed;
            }
            item.sellerLevel = p[i].level;
            item.factionId = seller;
            item.modifiers = item.modifiersDefault;
            if(!this._items[seller])
            {
               this._items[seller] = [];
            }
            var lvlTip:String = item.superCategory != 5 ? Util.styledChar(LocalizationData.getName("equipment_level") + " - " + item.itemLevel.toString(),"gray") : "";
            var tipStr:String = item.name + "\n" + (item.category == 27 ? "" : "") + ((item.rank) ? Util.styledChar(LocalizationData.getName("modified") + "\n","green") : "") + ((item.ammoText) ? LocalizationData.getName("ammo") + ":\n" + item.ammoText : "") + ((lvlTip) ? Util.styledChar(" \n","small") + lvlTip : "");
            item.tipText = tipStr;
            trace("setupShopItem::",p[i].dictId,item.name,"lockLabel::",this._names[seller][item.sellerLevel],"level::",seller,item.sellerLevel,"sc:",item.superCategory,"faction:",p[i].faction_id,p[i].seller,"mods::",item.modifiers,"cost::",p[i].cost,"cat",item.category);
            this._items[seller].push(item);
            item.cost = p[i].cost;
            item.price = item.cost;
            item.premiumCost = p[i].premium_cost;
            item.canBuy = this._unlockedLevel[seller] >= item.sellerLevel;
            item.buyValue = !item.canBuy;
            item.value = p[i].condition_or_stack;
            item.lockText = "item_sell_precondition_faction_" + seller;
            if(seller == 0)
            {
               this._itemsBuyDict[item.dictId] = true;
            }
            else if(item.canBuy)
            {
               this._itemsBuyDict[item.dictId] = true;
            }
            item.unlockLevelLabel = this._names[seller][item.sellerLevel];
            objects[i] = item;
         }
         l = objects.length;
         for(var k:uint = 0; k < l; k++)
         {
            item = objects[k];
            seller = (item.factionId);
            var descrData:Object = this._itemDescriptor.getItemDescription(item.dictId);
            trace("updateDescrCost::",descrData.name,item.cost,"current::",descrData.cost);
            if(descrData.cost == undefined)
            {
               descrData.cost = item.cost;
            }
            if(descrData.sellers == undefined)
            {
               descrData.sellers = [{
                  name:getSellerName(seller),
                  faction_id:seller,
                  label:"some label"
               }];
            }
            else if((noSeller(descrData.sellers,seller)))
            {
               descrData.sellers.push({
                  name:getSellerName(seller),
                  faction_id:seller,
                  label:"some label"
               });
            }
            item.sellers = descrData.sellers;
         }
         function getSellerName(faction_id:int):String
         {
            if(!_sellers)
            {
               return "name";
            }
            for(var k:uint = 0; k < _sellers.length; k++)
            {
               if(_sellers.faction_id == faction_id)
               {
                  return _sellers.name;
               }
            }
            return "name";
         }
         function noSeller(value:Array, index:uint):Boolean
         {
            for(var k:uint = 0; k < value.length; k++)
            {
               if(value[k].faction_id == index)
               {
                  return false;
               }
            }
            return true;
         }
      }
      
      private function OnNewRender(e:Event) : void
      {
         trace("onRender::");
         removeEventListener(e.type,this.OnNewRender);
         this.setupShopItems(this._new_objects);
      }
      
      public function setupLevelData(value:Array) : void
      {
         var l:uint = value.length;
         for(var i:uint = 0; i < l; i++)
         {
            var seller:uint = (value[i].faction_id);
            var level:uint = (value[i].level);
            var name:String = value[i].name;
            var reputation:uint = (value[i].reputation);
            this._names[seller][level] = name;
            if(!this._reputations[seller])
            {
               this._reputations[seller] = [];
            }
            this._reputations[seller][level] = reputation;
         }
      }
      
      public function setupShopDataLevel(seller:uint, p:Array, level:uint, name:String, reputation:uint) : void
      {
         var objects:Array = new Array();
         if(!this._levels[seller])
         {
            this._levels[seller] = new Array();
         }
         trace("setupShopDataLevel:",seller,level,"items::",p.length,name,reputation);
         this._names[seller][level] = name;
         if(!this._reputations[seller])
         {
            this._reputations[seller] = [];
         }
         this._reputations[seller][level] = reputation;
         if(!p.length)
         {
            if(seller == 4 && level == 10)
            {
               this.setupShopItems(this._new_objects);
            }
            return;
         }
         this._items[seller] = new Array();
         for(var i:uint = 0; i < p.length; i++)
         {
            var obj:ItemData = p[i];
            obj.itemLevel = level;
            obj.factionId = seller;
            this._new_objects.push(obj);
         }
         if(seller == 4 && level == 10)
         {
            this.setupShopItems(this._new_objects);
         }
      }
      
      public function getNewItems() : uint
      {
         return this._newItems;
      }
      
      public function unclockItem() : void
      {
         if(this._newItems > 0)
         {
            --this._newItems;
         }
      }
      
      public function setPlayerProgress(seller:uint, level:uint, progress:uint, disableText:String) : void
      {
         trace("setPlayerProgress",seller,level,progress,disableText,"trade::",isInvalid(InvalidationType.DATA));
         var firstTime:Boolean = this._initedSeller[seller] == undefined;
         this._initedSeller[seller] = true;
         this._unlockedLevel[seller] = level;
         this._currentProgress[seller] = progress;
         if(disableText != "")
         {
            this.disabled[seller] = disableText;
         }
         else
         {
            this.disabled[seller] = "";
         }
         var newItems:uint = 0;
         if(!isInvalid(InvalidationType.DATA))
         {
            _newItems = 0;
         }
         var currentNewItems:uint = this._newItems;
         if((this._items[seller]))
         {
            for(var k:uint = 0; k < this._items[seller].length; k++)
            {
               var item:ItemData = this._items[seller][k];
               var buyable:Boolean = this._unlockedLevel[seller] >= item.sellerLevel;
               if(item.canBuy == false && buyable && !firstTime)
               {
                  trace("updateItemCanBuy_onProgress:: item_level",item.name,item.sellerLevel,newItems,"unlocked_level:",this._unlockedLevel[seller]);
                  newItems++;
                  item.isNew = true;
               }
               if(buyable)
               {
                  this._itemsBuyDict[item.dictId] = buyable;
               }
               item.canBuy = buyable;
               item.buyValue = !item.canBuy;
            }
         }
         trace("totalNewItems::",this._newItems,newItems);
         _newItems += newItems;
         if(currentNewItems != newItems)
         {
            dispatchEvent(new Event(NEW_ITEMS_COUNT_CHANGED));
         }
         invalidateData();
      }
      
      public function getItemSeller(value:int) : uint
      {
         var lowest:uint = uint.MAX_VALUE;
         var lowestSeller:uint = 0;
         trace("getItemSeller",this._items.length);
         for(var i:uint = 0; i < this._items.length; i++)
         {
            if((this._items[i]))
            {
               for(var k:uint = 0; k < this._items[i].length; k++)
               {
                  if(this._items[i][k].dictId == value)
                  {
                     if(this._items[i][k].cost < lowest)
                     {
                        lowest = (this._items[i][k].cost);
                        lowestSeller = (this._items[i][k].seller);
                     }
                  }
               }
            }
         }
         return lowestSeller;
      }
      
      public function onPlayerMoneyUpdate(e:Event = null) : void
      {
         this._inventory.itemsTileList.invalidate(InvalidationType.ALL);
      }
      
      public function registerReputationsContainer(value:QuestWindow) : void
      {
         _reputationContainer = value;
      }
      
      public function registerBuyFunction(link:Function) : void
      {
         buyFunction = link;
      }
      
      public function getItemBuyable(dictId:uint) : Boolean
      {
         trace("getItemBuyable::",dictId,this._itemsBuyDict[dictId]);
         if((this._itemsBuyDict[dictId]))
         {
            return this._itemsBuyDict[dictId];
         }
         return false;
      }
      
      public function getItemLockedCost(dictId:uint) : uint
      {
         if(this.getItemBuyable(dictId))
         {
            return 0;
         }
         if((this._itemsDict[dictId]))
         {
            return this._itemsDict[dictId].premiumCost;
         }
         return 0;
      }
      
      public function getItemPrice(dictId:uint) : uint
      {
         if((this._itemsDict[dictId]))
         {
            return this._itemsDict[dictId].cost;
         }
         return 0;
      }
      
      public function fillSellers(sellers:Array) : void
      {
         _sellers = sellers;
         parent["inventory_list"].fillSellers(sellers);
         LocalizationData.addCallback(this.onLocalSeller,"non_faction_items");
      }
      
      public function selectTrader(trader:int) : void
      {
      }
      
      public function callTrader() : void
      {
      }
      
      public function getAciveSeller() : int
      {
         var seller:uint = 0;
         return seller;
      }
      
      override protected function draw() : void
      {
         var progress:Array;
         var i:*;
         var seller_icons:IconContainer;
         var factionId:int;
         var seller:uint;
         var sellerName:String;
         var j:int;
         var factionBonus:int;
         var iconContainer:IconContainer;
         var unlockedLevel:int;
         var last:int;
         var tot:int;
         var next:int;
         var cur:int;
         var statusStr:String;
         var hintStr:String;
         var names:Array;
         var factionRank:String;
         var l:int;
         var factionIcon:Icon;
         var factionSelectedIcon:Icon;
         var factionEnabled:Boolean;
         var k:uint;
         var itemData:ItemData;
         var descrData:Object;
         var seller_icon:Icon;
         super.draw();
         if((isInvalid(InvalidationType.DATA)) && (this._reputations) && (this._reputationContainer))
         {
            progress = new Array();
            for(i in this._reputations)
            {
               sellerName = "";
               for(j = 0; j < _sellers.length; j++)
               {
                  if(_sellers[j].faction_id == i)
                  {
                     sellerName = _sellers[j].name;
                     break;
                  }
               }
               factionBonus = 0;
               for(j = 0; j < this._factionBonus.length; j++)
               {
                  if(this._factionBonus[j].faction_id == i)
                  {
                     factionBonus = (this._factionBonus[j].bonus);
                     break;
                  }
               }
               iconContainer = IconContainer.getInstance("factionIconsBig");
               if((sellerName))
               {
                  unlockedLevel = (this._unlockedLevel[i]);
                  last = (this._reputations[i][unlockedLevel]);
                  if(this._reputations[i][unlockedLevel + 1] == undefined)
                  {
                     tot = (this._reputations[i][unlockedLevel]);
                     next = (this._reputations[i][unlockedLevel]);
                  }
                  else
                  {
                     tot = this._reputations[i][unlockedLevel + 1] - last;
                     next = (this._reputations[i][unlockedLevel + 1]);
                  }
                  cur = next == last ? next : (this._currentProgress[i] - last);
                  statusStr = "";
                  if(this.disabled[i] != "")
                  {
                     statusStr = this.disabled[i];
                  }
                  else
                  {
                     statusStr = this._names[i][unlockedLevel];
                  }
                  hintStr = "";
                  names = this._names[i];
                  factionRank = "";
                  trace("genSellerNames::",i,"_",names);
                  for(l = 0; l < names.length; l++)
                  {
                     if(unlockedLevel == l)
                     {
                        factionRank = names[l];
                        hintStr += this.styledChar(factionRank,"current") + "\n";
                     }
                     else
                     {
                        hintStr += names[l] + "\n";
                     }
                  }
                  factionIcon = iconContainer.getIcon(i * 3);
                  factionSelectedIcon = iconContainer.getIcon(i * 3 + 2);
                  factionEnabled = this.disabled[i] == "";
                  trace("addRepItem::",i,hintStr,factionIcon,cur / tot * 100,sellerName,"reputation_progress::",this._currentProgress[i],"Level::",unlockedLevel,"cur::",cur,"tot",tot,"prev",last,this._names[i][unlockedLevel],"|",this._reputations[i][0],"unl::",this._reputations[i][unlockedLevel],"unl_nxt::",this._reputations[i][unlockedLevel + 1],this._currentProgress[i],"::",this._currentProgress[i] / this._reputations[i][unlockedLevel] * 100);
                  progress.push({
                     faction_bonus:(factionBonus == 0 ? "" : (factionBonus > 0 ? "+" : "") + factionBonus.toString()),
                     faction_hint:hintStr,
                     faction_progress:(factionEnabled ? int(cur / tot * 100) : 0),
                     faction_label:String(Util.styledChar(String(Util.getMoneyString(Math.min(cur,next))),"highlight") + " / " + Util.getMoneyString(tot)),
                     faction_name:sellerName,
                     faction_icon:factionIcon,
                     faction_selected_icon:factionSelectedIcon,
                     faction_status:statusStr,
                     faction_enabled:factionEnabled,
                     faction_color_state:(factionEnabled ? 10477451 : 15101265),
                     faction_id:i,
                     faction_rank:factionRank,
                     faction_current:Math.min(this._currentProgress[i],next),
                     faction_next:next
                  });
               }
            }
            this._reputationContainer.setFactionProgress(progress);
            seller_icons = IconContainer.getInstance("factionItemIcons");
            for(seller = 0; seller < this._items.length; seller++)
            {
               for(k = 0; k < this._items[seller].length; k++)
               {
                  itemData = this._items[seller][k];
                  descrData = this._itemDescriptor.getItemDescription(itemData.dictId);
                  itemData.sellers = descrData.sellers;
                  seller_icon = seller_icons.getIcon(itemData.factionId);
                  itemData.sellerIcon = seller_icon;
               }
            }
            if((this._inventory))
            {
               this._inventory.setupShopItems(this.getAllItems());
            }
         }
      }
      
      protected function styledChar(value:String, style:String) : String
      {
         value = value.split("<").join("&lt;");
         return "<" + style + ">" + value + "</" + style + ">";
      }
      
      protected function getAllItems() : Array
      {
         var _allItems:Array = [];
         for(var i:uint = 0; i < this._items.length; i++)
         {
            _allItems = _allItems.concat(this._items[i]);
         }
         return _allItems;
      }
      
      protected function getItemsBySellers(value:int) : Array
      {
         return this._items[value];
      }
      
      protected function onLocalSeller(value:String) : void
      {
         parent["inventory_list"].fillSellers(_sellers);
      }
      
      protected function createItem(value:int) : ItemData
      {
         var obj:ItemData = new ItemData();
         obj.dictId = value;
         var descrData:Object = this._itemDescriptor.getItemDescription(value);
         var icons:IconContainer;
         var category:int = ItemFactory.getCategory(descrData.category);
         if(category == 1)
         {
            icons = IconContainer.getInstance("equipment");
         }
         else if(category == 2)
         {
            icons = IconContainer.getInstance("weapons");
         }
         else if(category == 5)
         {
            if(descrData.category == FiltersWindow.CAT_VOUCHER || descrData.category == 28 || descrData.category == 29)
            {
               icons = IconContainer.getInstance("vouchers");
            }
            else if(descrData.category == 26)
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
         if(!icons)
         {
            return null;
         }
         var iconRow:int = descrData.icon_row - 1;
         var iconColumn:int = descrData.icon_column - 1;
         obj.iconId = iconColumn * icons.rows + iconRow;
         if(descrData.modifiers == undefined)
         {
            descrData.modifiers = [];
         }
         obj.matchMakingLevel = descrData.match_making_level;
         obj.isPremium = descrData.is_premium;
         obj.type = descrData.voucher_type;
         obj.modifiersDefault = descrData.modifiers;
         obj.rank = descrData.rarity_level;
         var defModifersCount:uint = obj.modifiersDefault.length;
         for(var i:uint = 0; i < defModifersCount; i++)
         {
            obj.modifiersDefault[i].isDefault = true;
         }
         obj.isWeapon = category == 2;
         obj.oneHanded = !descrData.double_handed;
         obj.name = descrData.name;
         obj.itemLevel = descrData.level;
         if((descrData.descr))
         {
            obj.itemDescription = descrData.descr;
         }
         obj.properties = descrData.item_properties;
         obj.descriptor = 200;
         obj.category = descrData.category;
         obj.superCategory = category;
         obj.icon = icons.getIcon(obj.iconId);
         obj.dragIcon = icons.getIcon(obj.iconId);
         obj.isStack = descrData.is_stack;
         obj.weight = descrData.weight;
         obj.clipSize = descrData.clip_size;
         obj.clipsInBag = descrData.clips_in_bag;
         if(descrData.required_bags != undefined)
         {
            obj.clipsInBag = 1 / descrData.required_bags;
         }
         var str:String = "";
         if(obj.isWeapon)
         {
            if(!descrData.ammo_text)
            {
               var ammoItems:Array = ItemFactory.getItemCombableItemNames(obj.dictId);
               if((ammoItems))
               {
                  var ammoStr:String = "";
                  var ammoL:uint = ammoItems.length;
                  obj.ammoText = "";
                  for(var j:uint = 0; j < ammoL; j++)
                  {
                     var descrName:String = this._itemDescriptor.getItemDescription(ammoItems[j]).name;
                     if((descrName))
                     {
                        str += "   " + descrName + "\n";
                        obj.ammoText += (j == 0 ? "" : ", ") + descrName;
                     }
                  }
               }
               descrData.ammo_text = obj.ammoText;
            }
            else
            {
               obj.ammoText = descrData.ammo_text;
            }
         }
         obj.quickSlotItemSize = (descrData.fills_whole_pocket) ? 2 : 1;
         this._itemsDict[value] = obj;
         return obj;
      }
   }
}

