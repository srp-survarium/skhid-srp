package survarium.item
{
   import flash.utils.Dictionary;
   import survarium.Util;
   import survarium.constants.Category;
   import survarium.constants.ItemPlace;
   import survarium.constants.Reward;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.ItemData;
   import survarium.data.ItemDescriptor;
   import survarium.data.LocalizationData;
   import survarium.data.PropertiesDescriptor;
   import survarium.interfaces.IItemFactory;
   
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
      
      public static function setItemsCompatibility(_desc:Array) : void
      {
         for(var i:int = 0; i < _desc.length; i++)
         {
            var _vObject:Object;
            _vObject = _desc[i];
            if(!_itemsAgreements[_vObject.first_item_dict_id])
            {
               _itemsAgreements[_vObject.first_item_dict_id] = new Array();
            }
            _itemsAgreements[_vObject.first_item_dict_id].push(_vObject.second_item_dict_id);
         }
      }
      
      public static function getItemCombableItemNames(value:int) : Array
      {
         var result:Array = getList(value);
         return result;
      }
      
      public static function getList(value:int) : Array
      {
         var _list1:Array = new Array();
         var _temp_list:Array;
         var l:uint = 0;
         var str:String;
         var i:int;
         if(_itemsAgreements[value] != undefined)
         {
            _temp_list = _itemsAgreements[value];
            l = _temp_list.length;
            for(i = 0; i < l; i++)
            {
               _list1.push(_temp_list[i]);
            }
         }
         for(str in _itemsAgreements)
         {
            if(str != value.toString())
            {
               _temp_list = _itemsAgreements[str];
               if(_temp_list.indexOf(value) != -1)
               {
                  _list1.push(int(str));
               }
            }
         }
         _list1.sort(Array.NUMERIC);
         return _list1;
      }
      
      public static function getPair(dictId1:int, dictId2:int) : Boolean
      {
         var ds:Boolean;
         if((_itemsAgreements[dictId1] as Array) && (_itemsAgreements[dictId1] as Array).indexOf(dictId2) != -1)
         {
            return true;
         }
         if((_itemsAgreements[dictId2] as Array) && (_itemsAgreements[dictId2] as Array).indexOf(dictId1) != -1)
         {
            return true;
         }
         return false;
      }
      
      public static function isSame(_item1:int, _item2:int) : Boolean
      {
         var list1:Array = getList(_item1);
         var list2:Array = getList(_item2);
         if(list1.length != list2.length)
         {
            return false;
         }
         for(var i:int = 0; i < list1.length; i++)
         {
            if(list1[i] != list2[i])
            {
               return false;
            }
         }
         return true;
      }
      
      public static function getCategory(_cat:int) : int
      {
         var current_item_category_id:int = _cat;
         if(current_item_category_id == 1 || current_item_category_id == 2 || current_item_category_id == 3 || current_item_category_id == 4 || current_item_category_id == 5 || current_item_category_id == 6 || current_item_category_id == 7)
         {
            return Category.BASE_EQUIPMENT;
         }
         if(current_item_category_id == 8 || current_item_category_id == 13 || current_item_category_id == 14 || current_item_category_id == 15 || current_item_category_id == 16 || current_item_category_id == 17)
         {
            return Category.BASE_WEAPON;
         }
         if(current_item_category_id == 9 || current_item_category_id == 18 || current_item_category_id == 19 || current_item_category_id == 20 || current_item_category_id == 21)
         {
            return Category.BASE_AMMO;
         }
         if(current_item_category_id == Category.CAMO || current_item_category_id == Category.DECAL || current_item_category_id == Category.VOUCHER || current_item_category_id == Category.REGULAR || current_item_category_id == Category.GOLD)
         {
            return Category.BASE_VOUCHER;
         }
         return Category.BASE_CONSUMABLE;
      }
      
      public static function isItemCraftable(value:ItemData) : Boolean
      {
         return !value.isStack && value.descriptor <= ItemPlace.INVENTORY && !value.repairTime && (value.superCategory == Category.BASE_EQUIPMENT || value.superCategory == Category.BASE_WEAPON || value.superCategory == Category.BASE_AMMO);
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
         var descrData:Object = this._itemDescriptor.getItemDescription(dictId);
         var item_data:ItemData;
         var icons:IconContainer;
         trace("createBaseItem",dictId,descrData);
         item_data = new ItemData();
         var category:int = getCategory(descrData.category);
         if(category == Category.BASE_EQUIPMENT)
         {
            icons = IconContainer.getInstance("equipment");
         }
         else if(category == Category.BASE_WEAPON)
         {
            icons = IconContainer.getInstance("weapons");
         }
         else if(category == Category.BASE_VOUCHER)
         {
            if(descrData.category == Category.VOUCHER || descrData.category == Category.REGULAR || descrData.category == Category.GOLD)
            {
               icons = IconContainer.getInstance("vouchers");
            }
            else if(descrData.category == Category.DECAL)
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
         if(descrData.icon_row == undefined && (icons))
         {
            descrData.icon_row = descrData.icon % icons.rows + 1;
            descrData.icon_column = int(descrData.icon / icons.rows) + 1;
            trace("create_icon",descrData.icon,descrData.icon_row,descrData.icon_column);
         }
         var iconRow:int = descrData.icon_row - 1;
         var iconColumn:int = descrData.icon_column - 1;
         var itemIcon:Icon;
         trace("IL_icon",item_data.iconId,"|",iconRow,iconColumn);
         if((icons))
         {
            item_data.iconId = iconColumn * icons.rows + iconRow;
            itemIcon = icons.getIcon(item_data.iconId);
            item_data.icon = itemIcon;
            item_data.dragIcon = itemIcon.clone();
            item_data.messIcon = itemIcon.clone();
         }
         else
         {
            item_data.iconId = 0;
            itemIcon = new Icon();
            item_data.icon = itemIcon;
            item_data.dragIcon = itemIcon;
            item_data.messIcon = itemIcon;
         }
         if(descrData.double_handed == undefined)
         {
            descrData.double_handed = true;
         }
         item_data.matchMakingLevel = descrData.match_making_level;
         item_data.additionalSlots = descrData.additional_slots;
         item_data.type = descrData.voucher_type;
         item_data.superCategory = category;
         item_data.category = descrData.category;
         item_data.isWeapon = category == 2;
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
            descrData.camo_available = 1;
         }
         if(descrData.decal_available == undefined)
         {
            descrData.decal_available = 0;
         }
         item_data.sellers = descrData.sellers;
         item_data.camoAvailable = descrData.camo_available;
         item_data.decalAvailable = descrData.decal_available;
         if(descrData.upgrades_available == undefined)
         {
            descrData.upgrades_available = 4;
         }
         item_data.upgradesAvailable = descrData.upgrades_available;
         item_data.weight = descrData.weight;
         item_data.clipSize = descrData.clip_size;
         item_data.quickSlotItemSize = (descrData.fills_whole_pocket) ? 2 : 1;
         item_data.clipsInBag = descrData.clips_in_bag;
         if(descrData.required_bags != undefined)
         {
            item_data.clipsInBag = 1 / descrData.required_bags;
         }
         item_data.usable = item_data.category == Category.VOUCHER && (item_data.type == 3 || item_data.type == 0);
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
               ammoItems = null;
               if((ammoItems))
               {
                  ammoStr = "";
                  ammoL = ammoItems.length;
                  item_data.ammoText = "";
                  for(j = 0; j < ammoL; j++)
                  {
                     descrName = this._itemDescriptor.getItemDescription(ammoItems[j]).name;
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
      
      public function createItem(dictId:uint, seller:* = -1) : ItemData
      {
         var seller_icons:IconContainer = IconContainer.getInstance("factionItemIcons");
         var descrData:Object = this._itemDescriptor.getItemDescription(dictId);
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
               factionId = -1;
            }
         }
         if(factionId != -1 && (seller_icons))
         {
            seller_icon = seller_icons.getIcon(factionId);
         }
         else
         {
            seller_icon = null;
         }
         item_data.sellerIcon = seller_icon;
         item_data.factionId = factionId;
         trace("IL::create_item::",item_data.name,item_data.category,item_data.usable,item_data.type,item_data.category == Category.VOUCHER,descrData.descr);
         return item_data;
      }
      
      public function createItemData(value:Object) : ItemData
      {
         trace("IF::createItemData",value.dictId);
         if(value.dictId == undefined)
         {
            value.dictId = 70;
         }
         var item:ItemData = this.createItem(value.dictId,value.faction_id);
         item.value = value.condition_or_stack;
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
         return item;
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
               propPostFix = propDescription.modifier_type == 0 ? propDescription.postfix : "%";
               propPrefix = modifierProp.prop_value >= 0 ? "+" : "";
               style = propDescription.direction == 0 && modifierProp.prop_value < 0 || propDescription.direction == 1 && modifierProp.prop_value > 0 ? "green" : "red";
               propValue = propDescription.modifier_type == 0 ? modifierProp.prop_value : modifierProp.prop_value * 100;
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
                  propPostFix = propDescription.modifier_type == 0 ? propDescription.postfix : "%";
                  propPrefix = modifierProp.prop_value >= 0 ? "+" : "";
                  style = propDescription.direction == 0 && modifierProp.prop_value < 0 || propDescription.direction == 1 && modifierProp.prop_value > 0 ? "green" : "red";
                  propValue = propDescription.modifier_type == 0 ? modifierProp.prop_value : modifierProp.prop_value * 100;
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
               propPostFix = propDescription.modifier_type == 0 ? propDescription.postfix : "%";
               propPrefix = modifierProp.prop_value >= 0 ? "+" : "";
               if(propDescription.modifier_type == undefined)
               {
                  propDescription.modifier_type = 2;
                  propDescription.modifier_type = modifierProp.modifier_type;
               }
               propValue = propDescription.modifier_type == 0 ? modifierProp.prop_value : modifierProp.prop_value * 100;
               propValue = Math.round(propValue / 0.01) * 0.01;
               style = propDescription.direction == 0 && modifierProp.prop_value < 0 || propDescription.direction == 1 && modifierProp.prop_value > 0 ? "green" : "red";
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
      
      public function getItemData(dictId:uint) : Object
      {
         var result:Object = this._itemDescriptor.getItemDescription(dictId);
         return result;
      }
      
      public function setRewardIcons(value:Object) : void
      {
         _rewardIcons = value;
      }
      
      public function fillRewardItems(value:Array) : void
      {
         var l:uint = value.length;
         for(var i:uint = 0; i < l; i++)
         {
            var reward:Object;
            var itemData:ItemData;
            reward = value[i];
            if(reward.type == Reward.ITEM)
            {
               itemData = this.createBaseItem(reward.param);
               if(itemData.isStack)
               {
                  itemData.value = reward.amount;
               }
               itemData.tipText = itemData.name;
               reward.data = itemData;
            }
            else if(reward.type == Reward.EXPERIENCE)
            {
               itemData = new ItemData();
               itemData.icon = (this._rewardIcons[reward.type]) ? (this._rewardIcons[reward.type] as Icon).clone() : null;
               itemData.value = reward.amount;
               itemData.name = "";
               itemData.isStack = true;
               itemData.superCategory = Category.BASE_VOUCHER;
               itemData.tipText = LocalizationData.getName("experience");
               reward.data = itemData;
            }
            trace("rewardCreated::",itemData.name,itemData.icon);
         }
      }
      
      protected function updateItemTipText(item:ItemData) : void
      {
         var lvlTip:String = item.superCategory != Category.BASE_VOUCHER ? Util.styledChar(LocalizationData.getName("equipment_level") + " - " + item.itemLevel.toString(),"gray") : "";
         var tipStr:String = Util.styledChar(item.name,"white") + "\n" + ((item.rank) ? Util.styledChar(LocalizationData.getName("modified") + "\n","green") : "") + ((item.ammoText) ? LocalizationData.getName("ammo") + ":\n" + item.ammoText : "") + ((lvlTip) ? Util.styledChar(" \n","small") + lvlTip : "");
         item.tipText = tipStr;
      }
      
      protected function getFailModifiers(_data:Object) : Array
      {
         var modifiers:Array = _data.modifiers != null ? _data.modifiers : [];
         var modificators:Array = [];
         for(var j:uint = 0; j < modifiers.length; j++)
         {
            var _modifiersIconContainer:IconContainer;
            var icon:Icon;
            if(modifiers[j].state == 1)
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
                  propPostFix = propDescription.modifier_type == 0 ? propDescription.postfix : "%";
                  propPrefix = modifierProp.prop_value >= 0 ? "+" : "";
                  if(propDescription.modifier_type == undefined)
                  {
                     propDescription.modifier_type = 2;
                     propDescription.modifier_type = modifierProp.modifier_type;
                  }
                  if(propDescription.modifier_type == 2)
                  {
                     throw Error("modifier_type at modifier " + modificators[i].name + " for prop_id " + modifierProp.prop_id + " is invalid");
                  }
                  trace("propValue::",propDescription.direction,modifierProp.direction);
                  propValue = propDescription.modifier_type == 0 ? (modifierProp.prop_value) : modifierProp.prop_value * 100;
                  style = propDescription.direction == 0 && modifierProp.prop_value < 0 || propDescription.direction == 1 && modifierProp.prop_value > 0 ? "green" : "red";
                  hintText += propDescription.prop_name + ": " + Util.styledChar(propPrefix + (int(propValue) == propValue ? propValue.toString() : propValue.toFixed(1)) + propPostFix,style) + "\n";
               }
            }
            modificators[i].name = hintText;
         }
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
   }
}

