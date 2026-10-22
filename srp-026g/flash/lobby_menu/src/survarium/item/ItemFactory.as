package survarium.item
{
   import flash.utils.Dictionary;
   import survarium.Util;
   import survarium.constants.Reward;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.ItemData;
   import survarium.data.ItemDescriptor;
   import survarium.data.LocalizationData;
   import survarium.data.PropertiesDescriptor;
   import survarium.interfaces.IItemFactory;
   import survarium.upgrade.UpgradeWindow;
   import survarium.constants.Faction;
   import survarium.constants.ItemCategory;
   import survarium.constants.ItemDefaults;
   import survarium.constants.ItemDictId;
   import survarium.constants.ItemPlace;
   import survarium.constants.ItemRank;
   import survarium.constants.ModificationState;
   import survarium.constants.ModifierType;
   import survarium.constants.QuickSlotSize;
   import survarium.constants.SuperCategory;
   import survarium.constants.VoucherType;
   
   public class ItemFactory implements IItemFactory
   {
      
      protected static var _itemsAgreements:Dictionary = new Dictionary();
      
      private var _propsDescriptor:PropertiesDescriptor;
      
      private var _itemDescriptor:ItemDescriptor;
      
      private var _rewardIcons:Object;
      
      public function ItemFactory()
      {
         super();
      }
      
      public static function setItemsCompatibility(pairs:Array) : void
      {
         var pair:Object = null;
         var i:int = 0;
         while(i < pairs.length)
         {
            pair = pairs[i];
            if(!UpgradeWindow.addCompatibility(int(pair.first_item_dict_id),int(pair.second_item_dict_id)))
            {
               if(!_itemsAgreements[pair.first_item_dict_id])
               {
                  _itemsAgreements[pair.first_item_dict_id] = new Array();
               }
               _itemsAgreements[pair.first_item_dict_id].push(pair.second_item_dict_id);
            }
            i++;
         }
      }
      
      public static function getItemCombableItemNames(value:int) : Array
      {
         var result:Array = getList(value);
         return result;
      }
      
      public static function getList(dictId:int) : Array
      {
         var itemIds:Array = new Array();
         var partners:Array;
         var count:uint = 0;
         var key:String;
         var i:int;
         if(_itemsAgreements[dictId] != undefined)
         {
            partners = _itemsAgreements[dictId];
            count = partners.length;
            i = 0;
            while(i < count)
            {
               itemIds.push(partners[i]);
               i++;
            }
         }
         for(key in _itemsAgreements)
         {
            if(key != dictId.toString())
            {
               partners = _itemsAgreements[key];
               if(partners.indexOf(dictId) != -1)
               {
                  itemIds.push(int(key));
               }
            }
         }
         itemIds.sort(Array.NUMERIC);
         return itemIds;
      }
      
      public static function getPair(firstId:int, secondId:int) : Boolean
      {
         var paired:Boolean;
         if(_itemsAgreements[firstId] as Array && (_itemsAgreements[firstId] as Array).indexOf(secondId) != -1)
         {
            return true;
         }
         if(_itemsAgreements[secondId] as Array && (_itemsAgreements[secondId] as Array).indexOf(firstId) != -1)
         {
            return true;
         }
         return false;
      }
      
      public static function isSame(firstId:int, secondId:int) : Boolean
      {
         var firstList:Array = getList(firstId);
         var secondList:Array = getList(secondId);
         if(firstList.length != secondList.length)
         {
            return false;
         }
         var i:int = 0;
         while(i < firstList.length)
         {
            if(firstList[i] != secondList[i])
            {
               return false;
            }
            i++;
         }
         return true;
      }
      
      public static function getCategory(_cat:int) : int
      {
         var current_item_category_id:int = _cat;
         if(current_item_category_id == ItemCategory.HELMETS || current_item_category_id == ItemCategory.MASKS || current_item_category_id == ItemCategory.BODY_ARMOR || current_item_category_id == ItemCategory.RIGS || current_item_category_id == ItemCategory.PANTS || current_item_category_id == ItemCategory.GLOVES || current_item_category_id == ItemCategory.BOOTS)
         {
            return SuperCategory.EQUIPMENT;
         }
         if(current_item_category_id == ItemCategory.ASSAULT_RIFLES || current_item_category_id == ItemCategory.SNIPER_RIFLES || current_item_category_id == ItemCategory.SHOTGUNS || current_item_category_id == ItemCategory.SUBMACHINE_GUNS || current_item_category_id == ItemCategory.MACHINE_GUNS || current_item_category_id == ItemCategory.PISTOLS)
         {
            return SuperCategory.WEAPONS;
         }
         if(current_item_category_id == ItemCategory.ASSAULT_RIFLE_AMMO || current_item_category_id == ItemCategory.SNIPER_RIFLE_AMMO || current_item_category_id == ItemCategory.SHOTGUN_AMMO || current_item_category_id == ItemCategory.PISTOL_AMMO || current_item_category_id == ItemCategory.MACHINE_GUN_AMMO)
         {
            return SuperCategory.AMMO;
         }
         if(current_item_category_id == ItemCategory.CAMOUFLAGES || current_item_category_id == ItemCategory.DECALS || current_item_category_id == ItemCategory.VOUCHERS || current_item_category_id == ItemCategory.GENERIC_MONEY || current_item_category_id == ItemCategory.PREMIUM_MONEY)
         {
            return SuperCategory.UPGRADES;
         }
         return SuperCategory.ITEMS;
      }
      
      // The inventory tab whose filters list a category, for a tab's "all": as getCategory, but
      // scopes are in the upgrades tab's filters (beside camouflages and decals)
      public static function filterGroup(category:int) : int
      {
         return category == ItemCategory.SCOPES ? SuperCategory.UPGRADES : getCategory(category);
      }
      
      // A golden item: a golden weapon or a premium armour piece (golden::is_golden).
      public static function isGolden(dictId:int) : Boolean
      {
         return ItemDictId.GOLDEN_WEAPONS.indexOf(dictId) != -1 || ItemDictId.PREMIUM_ARMOUR.indexOf(dictId) != -1;
      }
      
      // A golden item's plain item; anything else itself.
      public static function plainOf(dictId:int) : int
      {
         var index:int = ItemDictId.GOLDEN_WEAPONS.indexOf(dictId);
         if(index != -1)
         {
            return int(ItemDictId.GOLDEN_PLAIN[index]);
         }
         index = ItemDictId.PREMIUM_ARMOUR.indexOf(dictId);
         return index == -1 ? dictId : int(ItemDictId.PREMIUM_ARMOUR_PLAIN[index]);
      }
      
      // The bonuses srp gives a new golden item as its own first modifications
      // (golden::starting_modifications), as the exe gives the movie a modification ({state, icon,
      // name, props} from its ui_desc; the name is unused for a modification): [id, icon, prop_id,
      // prop_value] a line. A test holds these to the data (survarium::v026g::golden::tests).
      public static const BONUS_UI:Array = [
         [1230,66,126,"0.10"], // +10 % silver
         [1240,65,127,"0.05"], // +5 % experience
         [1250,67,128,"0.05"], // +5 % reputation with the Scavengers
         [1260,68,129,"0.05"] // +5 % reputation with the Black Market
      ];
      
      public static const SILVER_BONUS:int = 1230;
      
      // Modification id as the exe gives it to the movie, of BONUS_UI; null for another id.
      public static function bonusModifier(id:int) : Object
      {
         var row:Array;
         for each(row in BONUS_UI)
         {
            if(row[0] == id)
            {
               return {
                  state:ModificationState.MODIFICATION,
                  icon:row[1],
                  props:[{
                     prop_id:row[2],
                     prop_value:row[3]
                  }]
               };
            }
         }
         return null;
      }
      
      // The modifications of its own a new item of dictId has, as srp gives them
      // (golden::starting_modifications): the silver bonus for a golden weapon, its premium bonuses
      // for a premium armour piece, none for anything else. For entries of items not owned (the
      // shop's, a bundle's contents); an owned item has the server's.
      public static function startingModifiers(dictId:int) : Array
      {
         var ids:Array;
         if(ItemDictId.GOLDEN_WEAPONS.indexOf(dictId) != -1)
         {
            ids = [SILVER_BONUS];
         }
         else
         {
            var index:int = ItemDictId.PREMIUM_ARMOUR.indexOf(dictId);
            if(index == -1)
            {
               return [];
            }
            ids = ItemDictId.PREMIUM_ARMOUR_BONUSES[index];
         }
         var modifiers:Array = [];
         var id:int;
         for each(id in ids)
         {
            modifiers.push(bonusModifier(id));
         }
         return modifiers;
      }
      
      // Whether an item of donorDictId may be used up to upgrade one of
      // itemDictId, as the server has it (golden::may_donate): the same
      // item, or for a golden item its plain item and its other golden
      // copies; never a golden item for a plain one.
      public static function isCraftDonor(donorDictId:int, itemDictId:int) : Boolean
      {
         return donorDictId == itemDictId || isGolden(itemDictId) && plainOf(donorDictId) == plainOf(itemDictId);
      }
      
      // Skhid: not with three modifications (rank MAX; the server refuses its craft): the item
      // panel's Upgrade and the right-click menu's entry are off then (owner 2026-10-22).
      public static function isItemCraftable(value:ItemData) : Boolean
      {
         return !value.isStack && value.descriptor <= ItemPlace.INVENTORY && !value.repairTime && value.rank < ItemRank.MAX && (value.superCategory == SuperCategory.EQUIPMENT || value.superCategory == SuperCategory.WEAPONS || value.superCategory == SuperCategory.AMMO);
      }
      
      public function registerItemDescriptor(link:ItemDescriptor) : void
      {
         _itemDescriptor = link;
      }
      
      public function getInventoryItem(itemId:uint) : ItemData
      {
         return new ItemData();
      }
      
      public function createBaseItem(dictId:uint) : ItemData
      {
         trace("factory::",this._itemDescriptor);
         var description:Object = this._itemDescriptor.getItemDescription(dictId);
         var item:ItemData;
         var icons:IconContainer;
         trace("createBaseItem",dictId,description);
         item = new ItemData();
         var superCategory:int = getCategory(description.category);
         if(superCategory == SuperCategory.EQUIPMENT)
         {
            icons = IconContainer.getInstance("equipment");
         }
         else if(superCategory == SuperCategory.WEAPONS)
         {
            icons = IconContainer.getInstance("weapons");
         }
         else if(superCategory == SuperCategory.UPGRADES)
         {
            if(description.category == ItemCategory.VOUCHERS || description.category == ItemCategory.GENERIC_MONEY || description.category == ItemCategory.PREMIUM_MONEY)
            {
               icons = IconContainer.getInstance("vouchers");
            }
            else if(description.category == ItemCategory.DECALS)
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
         if(description.icon_row == undefined && icons)
         {
            description.icon_row = description.icon % icons.rows + 1;
            description.icon_column = int(description.icon / icons.rows) + 1;
            trace("create_icon",description.icon,description.icon_row,description.icon_column);
         }
         var iconRow:int = description.icon_row - 1;
         var iconColumn:int = description.icon_column - 1;
         var icon:Icon;
         trace("IL_icon",item.iconId,"|",iconRow,iconColumn);
         if(icons)
         {
            item.iconId = iconColumn * icons.rows + iconRow;
            icon = icons.getIcon(item.iconId);
            item.icon = icon;
            item.dragIcon = icon.clone();
            item.messIcon = icon.clone();
         }
         else
         {
            item.iconId = 0;
            icon = new Icon();
            item.icon = icon;
            item.dragIcon = icon;
            item.messIcon = icon;
         }
         if(description.double_handed == undefined)
         {
            description.double_handed = true;
         }
         item.matchMakingLevel = description.match_making_level;
         item.additionalSlots = description.additional_slots;
         item.type = description.voucher_type;
         item.superCategory = superCategory;
         item.category = description.category;
         item.isWeapon = superCategory == SuperCategory.WEAPONS;
         item.isStack = description.is_stack;
         item.oneHanded = !description.double_handed;
         item.isPremium = description.is_premium;
         item.name = description.name;
         item.dictId = dictId;
         item.repairCostRatio = description.repair_cost_ratio;
         if(description.modifiers == undefined)
         {
            description.modifiers = [];
         }
         item.modifiers = description.modifiers;
         item.modifiersDefault = description.modifiers;
         item.rank = description.rarity_level;
         // A golden item keeps its gold background with no descriptor rarity.
         item.fromBundle = item.rank > 0 || isGolden(dictId);
         var defaultCount:uint = item.modifiersDefault.length;
         var i:uint = 0;
         while(i < defaultCount)
         {
            item.modifiersDefault[i].isDefault = true;
            i++;
         }
         item.properties = description.item_properties;
         if(description.descr)
         {
            item.itemDescription = description.descr;
         }
         item.cost = description.cost;
         item.itemLevel = description.level;
         if(description.camo_available == undefined)
         {
            description.camo_available = ItemDefaults.CAMO_AVAILABLE;
         }
         if(description.decal_available == undefined)
         {
            description.decal_available = ItemDefaults.DECAL_AVAILABLE;
         }
         item.sellers = description.sellers;
         item.camoAvailable = description.camo_available;
         item.decalAvailable = description.decal_available;
         if(description.upgrades_available == undefined)
         {
            description.upgrades_available = ItemDefaults.UPGRADES_AVAILABLE;
         }
         item.upgradesAvailable = description.upgrades_available;
         item.weight = description.weight;
         item.clipSize = description.clip_size;
         item.quickSlotItemSize = description.fills_whole_pocket ? QuickSlotSize.WHOLE : QuickSlotSize.HALF;
         item.clipsInBag = description.clips_in_bag;
         if(description.required_bags != undefined)
         {
            item.clipsInBag = 1 / description.required_bags;
         }
         item.usable = item.category == ItemCategory.VOUCHERS && (item.type == VoucherType.PREMIUM || item.type == VoucherType.RENAME);
         var ammoList:String = "";
         var ammoIds:Array;
         var unusedText:String;
         var ammoCount:uint;
         var ammoIndex:uint;
         var ammoName:String;
         if(item.isWeapon)
         {
            if(!description.ammo_text)
            {
               ammoIds = null;
               if(ammoIds)
               {
                  unusedText = "";
                  ammoCount = ammoIds.length;
                  item.ammoText = "";
                  ammoIndex = 0;
                  while(ammoIndex < ammoCount)
                  {
                     ammoName = this._itemDescriptor.getItemDescription(ammoIds[ammoIndex]).name;
                     if(ammoName)
                     {
                        ammoList += "   " + ammoName + "\n";
                        item.ammoText += (ammoIndex == 0 ? "" : ", ") + ammoName;
                     }
                     ammoIndex++;
                  }
               }
               description.ammo_text = item.ammoText;
            }
            else
            {
               item.ammoText = description.ammo_text;
            }
         }
         return item;
      }
      
      public function createItem(dictId:uint, defaultFactionId:* = Faction.ANY) : ItemData
      {
         var factionIcons:IconContainer = IconContainer.getInstance("factionItemIcons");
         var description:Object = this._itemDescriptor.getItemDescription(dictId);
         var item:ItemData = this.createBaseItem(dictId);
         var sellerIcon:Icon;
         var factionId:int = defaultFactionId;
         if(description && description.sellers)
         {
            if((description.sellers as Array).length == 1)
            {
               factionId = (description.sellers[0].faction_id);
            }
            else
            {
               factionId = Faction.ANY;
            }
         }
         if(factionId != Faction.ANY && factionIcons)
         {
            sellerIcon = factionIcons.getIcon(factionId);
         }
         else
         {
            sellerIcon = null;
         }
         item.sellerIcon = sellerIcon;
         item.factionId = factionId;
         trace("IL::create_item::",item.name,item.category,item.usable,item.type,item.category == ItemCategory.VOUCHERS,description.descr);
         return item;
      }
      
      public function createItemData(itemInfo:Object) : ItemData
      {
         trace("IF::createItemData",itemInfo.dictId);
         if(itemInfo.dictId == undefined)
         {
            itemInfo.dictId = ItemDictId.AMMO_9X18_MAKAROV;
         }
         var item:ItemData = this.createItem(itemInfo.dictId,itemInfo.faction_id);
         item.value = itemInfo.condition_or_stack;
         if(itemInfo.double_handed != undefined)
         {
            item.oneHanded = !itemInfo.double_handed;
         }
         if(itemInfo.modifiers != undefined)
         {
            item.broken = this.getItemBroken(itemInfo.modifiers);
            item.modifiers = (itemInfo.modifiers as Array).concat(item.modifiersDefault);
         }
         if(itemInfo.rarity_level != undefined)
         {
            item.rank = itemInfo.rarity_level;
         }
         if(itemInfo.cost)
         {
            item.cost = itemInfo.cost;
         }
         item.camoItem = itemInfo.camo_item;
         item.decalItem = itemInfo.decal_item;
         this.updateItemTipText(item);
         return item;
      }
      
      public function fillModifierDescription(modifier:Object) : void
      {
         var propDesc:Object;
         _propsDescriptor = PropertiesDescriptor.getPropertiesDescriptor("properties");
         var props:Object = modifier.props;
         var prop:Object;
         var propValue:Number;
         var postfix:String;
         var sign:String;
         var color:String;
         var line:String;
         trace("modifier::",props);
         modifier.textValue = "";
         for each(prop in props)
         {
            propValue = Number(prop.prop_value);
            if(propValue != Number(prop.prop_value))
            {
               propValue = Util.stringToNumber(prop.prop_value);
            }
            propDesc = this._propsDescriptor.getPropertyDescription(prop.prop_id);
            if(propDesc)
            {
               postfix = propDesc.modifier_type == ModifierType.ADD ? propDesc.postfix : "%";
               sign = prop.prop_value >= 0 ? "+" : "";
               color = ModificationRolls.style(propDesc,prop.prop_id,ModificationRolls.value(prop));
               propValue = propDesc.modifier_type == ModifierType.ADD ? (prop.prop_value) : prop.prop_value * 100;
               line = propDesc.prop_name + ": " + Util.styledChar(sign + propValue + (postfix ? postfix : ""),color);
               modifier.textValue += line + "\n";
            }
         }
      }
      
      public function fillModifiersDescription(item:ItemData) : void
      {
         _propsDescriptor = PropertiesDescriptor.getPropertiesDescriptor("properties");
         var modifiers:Array = item.modifiers;
         trace("fillModifiersDescription::",item.name,item.itemId,modifiers);
         if(!modifiers)
         {
            return;
         }
         var summary:String = "";
         var totals:Object = new Object();
         var itemName:String = item.name;
         var propDesc:Object;
         var postfix:String;
         var sign:String;
         var color:String;
         var i:uint = 0;
         var props:Object;
         var prop:Object;
         var propValue:Number;
         var line:String;
         var totalLine:String;
         while(i < modifiers.length)
         {
            props = modifiers[i].props;
            trace("modifier::",summary,props);
            modifiers[i].textValue = "";
            for each(prop in props)
            {
               if(totals[prop.prop_id] == undefined)
               {
                  trace("addPropValue::",prop.prop_id,prop.prop_value);
                  totals[prop.prop_id] = {
                     prop_id:prop.prop_id,
                     prop_value:0,
                     modifier:modifiers[i]
                  };
               }
               propValue = Number(prop.prop_value);
               if(propValue != Number(prop.prop_value))
               {
                  propValue = Util.stringToNumber(prop.prop_value);
               }
               totals[prop.prop_id].prop_value += propValue;
               propDesc = this._propsDescriptor.getPropertyDescription(prop.prop_id);
               if(propDesc)
               {
                  postfix = propDesc.modifier_type == ModifierType.ADD ? propDesc.postfix : "%";
                  sign = prop.prop_value >= 0 ? "+" : "";
                  color = ModificationRolls.style(propDesc,prop.prop_id,ModificationRolls.value(prop));
                  propValue = propDesc.modifier_type == ModifierType.ADD ? (prop.prop_value) : prop.prop_value * 100;
                  line = propDesc.prop_name + ": " + Util.styledChar(sign + propValue + (postfix ? postfix : ""),color);
                  modifiers[i].textValue += line + "\n";
               }
            }
            i++;
         }
         for each(prop in totals)
         {
            propDesc = this._propsDescriptor.getPropertyDescription(prop.prop_id);
            if(propDesc)
            {
               if(propDesc.postfix == undefined)
               {
                  propDesc.postfix = "";
               }
               postfix = propDesc.modifier_type == ModifierType.ADD ? propDesc.postfix : "%";
               sign = prop.prop_value >= 0 ? "+" : "";
               if(propDesc.modifier_type == undefined)
               {
                  propDesc.modifier_type = ModifierType.INVALID;
                  propDesc.modifier_type = prop.modifier_type;
               }
               propValue = propDesc.modifier_type == ModifierType.ADD ? (prop.prop_value) : prop.prop_value * 100;
               propValue = Math.round(propValue / 0.01) * 0.01;
               color = ModificationRolls.style(propDesc,prop.prop_id,ModificationRolls.value(prop));
               totalLine = propDesc.prop_name + ": " + Util.styledChar(sign + propValue + postfix,color);
               if(!prop.modifier.textValue)
               {
                  prop.modifier.textValue = totalLine;
               }
               summary += totalLine + "\n";
            }
         }
         item.modifiersDescription = summary;
      }
      
      public function getItemData(dictId:uint) : Object
      {
         var result:Object = this._itemDescriptor.getItemDescription(dictId);
         return result;
      }
      
      public function setRewardIcons(value:Object) : void
      {
         _rewardIcons = value;
      }
      
      public function fillRewardItems(rewards:Array) : void
      {
         var count:uint = rewards.length;
         var i:uint = 0;
         var reward:Object;
         var item:ItemData;
         while(i < count)
         {
            reward = rewards[i];
            if(reward.type == Reward.ITEM)
            {
               item = this.createBaseItem(reward.param);
               if(item.isStack)
               {
                  item.value = reward.amount;
               }
               item.tipText = item.name;
               reward.data = item;
            }
            else if(reward.type == Reward.EXPERIENCE)
            {
               item = new ItemData();
               item.icon = this._rewardIcons[reward.type] ? (this._rewardIcons[reward.type] as Icon).clone() : null;
               item.value = reward.amount;
               item.name = "";
               item.isStack = true;
               item.superCategory = SuperCategory.UPGRADES;
               item.tipText = LocalizationData.getName("experience");
               reward.data = item;
            }
            trace("rewardCreated::",item.name,item.icon);
            i++;
         }
      }
      
      protected function updateItemTipText(item:ItemData) : void
      {
         var levelText:String = item.superCategory != SuperCategory.UPGRADES ? Util.styledChar(LocalizationData.getName("equipment_level") + " - " + item.itemLevel.toString(),"gray") : "";
         var tip:String = Util.styledChar(item.name,"white") + "\n" + (item.rank ? Util.styledChar(LocalizationData.getName("modified") + "\n","green") : "") + (item.ammoText ? LocalizationData.getName("ammo") + ":\n" + item.ammoText : "") + (levelText ? Util.styledChar(" \n","small") + levelText : "");
         item.tipText = tip;
      }
      
      protected function getFailModifiers(itemInfo:Object) : Array
      {
         var modifiers:Array = itemInfo.modifiers != null ? itemInfo.modifiers : [];
         var failed:Array = [];
         var i:uint = 0;
         var icons:IconContainer;
         var modifierIcon:Icon;
         while(i < modifiers.length)
         {
            if(modifiers[i].state == ModificationState.DEFECT)
            {
               icons = IconContainer.getInstance("modifiers");
               modifierIcon = icons.getIcon(modifiers[i].icon);
               failed.push({
                  name:modifiers[i].name,
                  props:modifiers[i].props,
                  icon:modifierIcon,
                  state:modifiers[i].state,
                  fix_time_general:modifiers[i].fix_time_general,
                  fix_cost_general:modifiers[i].fix_cost_general,
                  fix_cost_gold:modifiers[i].fix_cost_gold
               });
            }
            i++;
         }
         this.fillModifiersNames(failed);
         return failed;
      }
      
      protected function fillModifiersNames(modifiers:Array) : void
      {
         var i:uint = 0;
         var propsText:String;
         var props:Object;
         var prop:Object;
         var propDesc:Object;
         var postfix:String;
         var sign:String;
         var propValue:Number;
         var color:String;
         while(i < modifiers.length)
         {
            propsText = "";
            props = modifiers[i].props;
            trace("props::",propsText,props,this._propsDescriptor);
            for each(prop in props)
            {
               propDesc = this._propsDescriptor.getPropertyDescription(prop.prop_id);
               if(propDesc)
               {
                  if(propDesc.postfix == undefined)
                  {
                     propDesc.postfix = "";
                  }
                  postfix = propDesc.modifier_type == ModifierType.ADD ? propDesc.postfix : "%";
                  sign = prop.prop_value >= 0 ? "+" : "";
                  if(propDesc.modifier_type == undefined)
                  {
                     propDesc.modifier_type = ModifierType.INVALID;
                     propDesc.modifier_type = prop.modifier_type;
                  }
                  if(propDesc.modifier_type == ModifierType.INVALID)
                  {
                     throw Error("modifier_type at modifier " + modifiers[i].name + " for prop_id " + prop.prop_id + " is invalid");
                  }
                  trace("propValue::",propDesc.direction,prop.direction);
                  propValue = propDesc.modifier_type == ModifierType.ADD ? (prop.prop_value) : prop.prop_value * 100;
                  color = ModificationRolls.style(propDesc,prop.prop_id,ModificationRolls.value(prop));
                  propsText += propDesc.prop_name + ": " + Util.styledChar(sign + (int(propValue) == propValue ? propValue.toString() : propValue.toFixed(1)) + postfix,color) + "\n";
               }
            }
            modifiers[i].name = propsText;
            i++;
         }
      }
      
      private function getItemBroken(modifiers:Array) : Boolean
      {
         if(!modifiers)
         {
            return false;
         }
         var count:uint = modifiers.length;
         var i:uint = 0;
         var modifier:Object;
         while(i < count)
         {
            modifier = modifiers[i];
            if(modifier.state)
            {
               return true;
            }
            i++;
         }
         return false;
      }
   }
}

