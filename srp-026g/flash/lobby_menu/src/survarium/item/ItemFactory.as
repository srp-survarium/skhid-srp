package survarium.item
{
   import flash.utils.Dictionary;
   import survarium.Util;
   import survarium.constants.Category;
   import survarium.constants.Reward;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.ItemData;
   import survarium.data.ItemDescriptor;
   import survarium.data.LocalizationData;
   import survarium.data.PropertiesDescriptor;
   import survarium.interfaces.IItemFactory;
   import survarium.upgrade.UpgradeWindow;
   
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
         if(current_item_category_id == 1 || current_item_category_id == 2 || current_item_category_id == 3 || current_item_category_id == 4 || current_item_category_id == 5 || current_item_category_id == 6 || current_item_category_id == 7)
         {
            return 1;
         }
         if(current_item_category_id == 8 || current_item_category_id == 13 || current_item_category_id == 14 || current_item_category_id == 15 || current_item_category_id == 16 || current_item_category_id == 17)
         {
            return 2;
         }
         if(current_item_category_id == 9 || current_item_category_id == 18 || current_item_category_id == 19 || current_item_category_id == 20 || current_item_category_id == 21)
         {
            return 3;
         }
         if(current_item_category_id == 25 || current_item_category_id == 26 || current_item_category_id == 27 || current_item_category_id == 28 || current_item_category_id == 29)
         {
            return 5;
         }
         return 4;
      }
      
      public static function isItemCraftable(value:ItemData) : Boolean
      {
         return !value.isStack && value.descriptor <= 100 && !value.repairTime && (value.superCategory == 1 || value.superCategory == 2 || value.superCategory == 3);
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
         if(superCategory == 1)
         {
            icons = IconContainer.getInstance("equipment");
         }
         else if(superCategory == 2)
         {
            icons = IconContainer.getInstance("weapons");
         }
         else if(superCategory == 5)
         {
            if(description.category == Category.VOUCHER || description.category == 28 || description.category == Category.GOLD)
            {
               icons = IconContainer.getInstance("vouchers");
            }
            else if(description.category == Category.DECAL)
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
         item.isWeapon = superCategory == 2;
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
         item.fromBundle = item.rank > 0;
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
            description.camo_available = 1;
         }
         if(description.decal_available == undefined)
         {
            description.decal_available = 0;
         }
         item.sellers = description.sellers;
         item.camoAvailable = description.camo_available;
         item.decalAvailable = description.decal_available;
         if(description.upgrades_available == undefined)
         {
            description.upgrades_available = 4;
         }
         item.upgradesAvailable = description.upgrades_available;
         item.weight = description.weight;
         item.clipSize = description.clip_size;
         item.quickSlotItemSize = description.fills_whole_pocket ? 2 : 1;
         item.clipsInBag = description.clips_in_bag;
         if(description.required_bags != undefined)
         {
            item.clipsInBag = 1 / description.required_bags;
         }
         item.usable = item.category == Category.VOUCHER && (item.type == 3 || item.type == 0);
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
      
      public function createItem(dictId:uint, defaultFactionId:* = -1) : ItemData
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
               factionId = -1;
            }
         }
         if(factionId != -1 && factionIcons)
         {
            sellerIcon = factionIcons.getIcon(factionId);
         }
         else
         {
            sellerIcon = null;
         }
         item.sellerIcon = sellerIcon;
         item.factionId = factionId;
         trace("IL::create_item::",item.name,item.category,item.usable,item.type,item.category == Category.VOUCHER,description.descr);
         return item;
      }
      
      public function createItemData(itemInfo:Object) : ItemData
      {
         trace("IF::createItemData",itemInfo.dictId);
         if(itemInfo.dictId == undefined)
         {
            itemInfo.dictId = 70;
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
               postfix = propDesc.modifier_type == 0 ? propDesc.postfix : "%";
               sign = prop.prop_value >= 0 ? "+" : "";
               color = propDesc.direction == 0 && prop.prop_value < 0 || propDesc.direction == 1 && prop.prop_value > 0 ? "green" : "red";
               propValue = propDesc.modifier_type == 0 ? (prop.prop_value) : prop.prop_value * 100;
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
                  postfix = propDesc.modifier_type == 0 ? propDesc.postfix : "%";
                  sign = prop.prop_value >= 0 ? "+" : "";
                  color = propDesc.direction == 0 && prop.prop_value < 0 || propDesc.direction == 1 && prop.prop_value > 0 ? "green" : "red";
                  propValue = propDesc.modifier_type == 0 ? (prop.prop_value) : prop.prop_value * 100;
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
               postfix = propDesc.modifier_type == 0 ? propDesc.postfix : "%";
               sign = prop.prop_value >= 0 ? "+" : "";
               if(propDesc.modifier_type == undefined)
               {
                  propDesc.modifier_type = 2;
                  propDesc.modifier_type = prop.modifier_type;
               }
               propValue = propDesc.modifier_type == 0 ? (prop.prop_value) : prop.prop_value * 100;
               propValue = Math.round(propValue / 0.01) * 0.01;
               color = propDesc.direction == 0 && prop.prop_value < 0 || propDesc.direction == 1 && prop.prop_value > 0 ? "green" : "red";
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
               item.superCategory = Category.BASE_VOUCHER;
               item.tipText = LocalizationData.getName("experience");
               reward.data = item;
            }
            trace("rewardCreated::",item.name,item.icon);
            i++;
         }
      }
      
      protected function updateItemTipText(item:ItemData) : void
      {
         var levelText:String = item.superCategory != 5 ? Util.styledChar(LocalizationData.getName("equipment_level") + " - " + item.itemLevel.toString(),"gray") : "";
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
            if(modifiers[i].state == 1)
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
                  postfix = propDesc.modifier_type == 0 ? propDesc.postfix : "%";
                  sign = prop.prop_value >= 0 ? "+" : "";
                  if(propDesc.modifier_type == undefined)
                  {
                     propDesc.modifier_type = 2;
                     propDesc.modifier_type = prop.modifier_type;
                  }
                  if(propDesc.modifier_type == 2)
                  {
                     throw Error("modifier_type at modifier " + modifiers[i].name + " for prop_id " + prop.prop_id + " is invalid");
                  }
                  trace("propValue::",propDesc.direction,prop.direction);
                  propValue = propDesc.modifier_type == 0 ? (prop.prop_value) : prop.prop_value * 100;
                  color = propDesc.direction == 0 && prop.prop_value < 0 || propDesc.direction == 1 && prop.prop_value > 0 ? "green" : "red";
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

