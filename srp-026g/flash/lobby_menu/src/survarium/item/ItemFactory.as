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
      
      public static function setItemsCompatibility(param1:Array) : void
      {
         var _loc3_:Object = null;
         var _loc2_:int = 0;
         while(_loc2_ < param1.length)
         {
            _loc3_ = param1[_loc2_];
            if(!UpgradeWindow.addCompatibility(int(_loc3_.first_item_dict_id),int(_loc3_.second_item_dict_id)))
            {
               if(!_itemsAgreements[_loc3_.first_item_dict_id])
               {
                  _itemsAgreements[_loc3_.first_item_dict_id] = new Array();
               }
               _itemsAgreements[_loc3_.first_item_dict_id].push(_loc3_.second_item_dict_id);
            }
            _loc2_++;
         }
      }
      
      public static function getItemCombableItemNames(param1:int) : Array
      {
         var _loc2_:Array = getList(param1);
         return _loc2_;
      }
      
      public static function getList(param1:int) : Array
      {
         var _loc2_:Array = new Array();
         var _loc3_:Array;
         var _loc4_:uint = 0;
         var _loc5_:String;
         var _loc6_:int;
         if(_itemsAgreements[param1] != undefined)
         {
            _loc3_ = _itemsAgreements[param1];
            _loc4_ = _loc3_.length;
            _loc6_ = 0;
            while(_loc6_ < _loc4_)
            {
               _loc2_.push(_loc3_[_loc6_]);
               _loc6_++;
            }
         }
         for(_loc5_ in _itemsAgreements)
         {
            if(_loc5_ != param1.toString())
            {
               _loc3_ = _itemsAgreements[_loc5_];
               if(_loc3_.indexOf(param1) != -1)
               {
                  _loc2_.push(int(_loc5_));
               }
            }
         }
         _loc2_.sort(Array.NUMERIC);
         return _loc2_;
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
      
      public static function isSame(param1:int, param2:int) : Boolean
      {
         var _loc3_:Array = getList(param1);
         var _loc4_:Array = getList(param2);
         if(_loc3_.length != _loc4_.length)
         {
            return false;
         }
         var _loc5_:int = 0;
         while(_loc5_ < _loc3_.length)
         {
            if(_loc3_[_loc5_] != _loc4_[_loc5_])
            {
               return false;
            }
            _loc5_++;
         }
         return true;
      }
      
      public static function getCategory(param1:int) : int
      {
         var _loc2_:int = param1;
         if(_loc2_ == 1 || _loc2_ == 2 || _loc2_ == 3 || _loc2_ == 4 || _loc2_ == 5 || _loc2_ == 6 || _loc2_ == 7)
         {
            return 1;
         }
         if(_loc2_ == 8 || _loc2_ == 13 || _loc2_ == 14 || _loc2_ == 15 || _loc2_ == 16 || _loc2_ == 17)
         {
            return 2;
         }
         if(_loc2_ == 9 || _loc2_ == 18 || _loc2_ == 19 || _loc2_ == 20 || _loc2_ == 21)
         {
            return 3;
         }
         if(_loc2_ == 25 || _loc2_ == 26 || _loc2_ == 27 || _loc2_ == 28 || _loc2_ == 29)
         {
            return 5;
         }
         return 4;
      }
      
      public static function isItemCraftable(param1:ItemData) : Boolean
      {
         return !param1.isStack && param1.descriptor <= 100 && !param1.repairTime && (param1.superCategory == 1 || param1.superCategory == 2 || param1.superCategory == 3);
      }
      
      public function registerItemDescriptor(param1:ItemDescriptor) : void
      {
         _itemDescriptor = param1;
      }
      
      public function getInventoryItem(param1:uint) : ItemData
      {
         return new ItemData();
      }
      
      public function createBaseItem(param1:uint) : ItemData
      {
         trace("factory::",this._itemDescriptor);
         var _loc2_:Object = this._itemDescriptor.getItemDescription(param1);
         var _loc3_:ItemData;
         var _loc4_:IconContainer;
         trace("createBaseItem",param1,_loc2_);
         _loc3_ = new ItemData();
         var _loc5_:int = getCategory(_loc2_.category);
         if(_loc5_ == 1)
         {
            _loc4_ = IconContainer.getInstance("equipment");
         }
         else if(_loc5_ == 2)
         {
            _loc4_ = IconContainer.getInstance("weapons");
         }
         else if(_loc5_ == 5)
         {
            if(_loc2_.category == Category.VOUCHER || _loc2_.category == 28 || _loc2_.category == Category.GOLD)
            {
               _loc4_ = IconContainer.getInstance("vouchers");
            }
            else if(_loc2_.category == Category.DECAL)
            {
               _loc4_ = IconContainer.getInstance("decals");
            }
            else
            {
               _loc4_ = IconContainer.getInstance("camo");
            }
         }
         else
         {
            _loc4_ = IconContainer.getInstance("consumables");
         }
         if(_loc2_.icon_row == undefined && _loc4_)
         {
            _loc2_.icon_row = _loc2_.icon % _loc4_.rows + 1;
            _loc2_.icon_column = int(_loc2_.icon / _loc4_.rows) + 1;
            trace("create_icon",_loc2_.icon,_loc2_.icon_row,_loc2_.icon_column);
         }
         var _loc6_:int = _loc2_.icon_row - 1;
         var _loc7_:int = _loc2_.icon_column - 1;
         var _loc8_:Icon;
         trace("IL_icon",_loc3_.iconId,"|",_loc6_,_loc7_);
         if(_loc4_)
         {
            _loc3_.iconId = _loc7_ * _loc4_.rows + _loc6_;
            _loc8_ = _loc4_.getIcon(_loc3_.iconId);
            _loc3_.icon = _loc8_;
            _loc3_.dragIcon = _loc8_.clone();
            _loc3_.messIcon = _loc8_.clone();
         }
         else
         {
            _loc3_.iconId = 0;
            _loc8_ = new Icon();
            _loc3_.icon = _loc8_;
            _loc3_.dragIcon = _loc8_;
            _loc3_.messIcon = _loc8_;
         }
         if(_loc2_.double_handed == undefined)
         {
            _loc2_.double_handed = true;
         }
         _loc3_.matchMakingLevel = _loc2_.match_making_level;
         _loc3_.additionalSlots = _loc2_.additional_slots;
         _loc3_.type = _loc2_.voucher_type;
         _loc3_.superCategory = _loc5_;
         _loc3_.category = _loc2_.category;
         _loc3_.isWeapon = _loc5_ == 2;
         _loc3_.isStack = _loc2_.is_stack;
         _loc3_.oneHanded = !_loc2_.double_handed;
         _loc3_.isPremium = _loc2_.is_premium;
         _loc3_.name = _loc2_.name;
         _loc3_.dictId = param1;
         _loc3_.repairCostRatio = _loc2_.repair_cost_ratio;
         if(_loc2_.modifiers == undefined)
         {
            _loc2_.modifiers = [];
         }
         _loc3_.modifiers = _loc2_.modifiers;
         _loc3_.modifiersDefault = _loc2_.modifiers;
         _loc3_.rank = _loc2_.rarity_level;
         _loc3_.fromBundle = _loc3_.rank > 0;
         var _loc9_:uint = _loc3_.modifiersDefault.length;
         var _loc10_:uint = 0;
         while(_loc10_ < _loc9_)
         {
            _loc3_.modifiersDefault[_loc10_].isDefault = true;
            _loc10_++;
         }
         _loc3_.properties = _loc2_.item_properties;
         if(_loc2_.descr)
         {
            _loc3_.itemDescription = _loc2_.descr;
         }
         _loc3_.cost = _loc2_.cost;
         _loc3_.itemLevel = _loc2_.level;
         if(_loc2_.camo_available == undefined)
         {
            _loc2_.camo_available = 1;
         }
         if(_loc2_.decal_available == undefined)
         {
            _loc2_.decal_available = 0;
         }
         _loc3_.sellers = _loc2_.sellers;
         _loc3_.camoAvailable = _loc2_.camo_available;
         _loc3_.decalAvailable = _loc2_.decal_available;
         if(_loc2_.upgrades_available == undefined)
         {
            _loc2_.upgrades_available = 4;
         }
         _loc3_.upgradesAvailable = _loc2_.upgrades_available;
         _loc3_.weight = _loc2_.weight;
         _loc3_.clipSize = _loc2_.clip_size;
         _loc3_.quickSlotItemSize = _loc2_.fills_whole_pocket ? 2 : 1;
         _loc3_.clipsInBag = _loc2_.clips_in_bag;
         if(_loc2_.required_bags != undefined)
         {
            _loc3_.clipsInBag = 1 / _loc2_.required_bags;
         }
         _loc3_.usable = _loc3_.category == Category.VOUCHER && (_loc3_.type == 3 || _loc3_.type == 0);
         var _loc11_:String = "";
         var _loc12_:Array;
         var _loc13_:String;
         var _loc14_:uint;
         var _loc15_:uint;
         var _loc16_:String;
         if(_loc3_.isWeapon)
         {
            if(!_loc2_.ammo_text)
            {
               _loc12_ = null;
               if(_loc12_)
               {
                  _loc13_ = "";
                  _loc14_ = _loc12_.length;
                  _loc3_.ammoText = "";
                  _loc15_ = 0;
                  while(_loc15_ < _loc14_)
                  {
                     _loc16_ = this._itemDescriptor.getItemDescription(_loc12_[_loc15_]).name;
                     if(_loc16_)
                     {
                        _loc11_ += "   " + _loc16_ + "\n";
                        _loc3_.ammoText += (_loc15_ == 0 ? "" : ", ") + _loc16_;
                     }
                     _loc15_++;
                  }
               }
               _loc2_.ammo_text = _loc3_.ammoText;
            }
            else
            {
               _loc3_.ammoText = _loc2_.ammo_text;
            }
         }
         return _loc3_;
      }
      
      public function createItem(param1:uint, param2:* = -1) : ItemData
      {
         var _loc3_:IconContainer = IconContainer.getInstance("factionItemIcons");
         var _loc4_:Object = this._itemDescriptor.getItemDescription(param1);
         var _loc5_:ItemData = this.createBaseItem(param1);
         var _loc6_:Icon;
         var _loc7_:int = param2;
         if(_loc4_ && _loc4_.sellers)
         {
            if((_loc4_.sellers as Array).length == 1)
            {
               _loc7_ = (_loc4_.sellers[0].faction_id);
            }
            else
            {
               _loc7_ = -1;
            }
         }
         if(_loc7_ != -1 && _loc3_)
         {
            _loc6_ = _loc3_.getIcon(_loc7_);
         }
         else
         {
            _loc6_ = null;
         }
         _loc5_.sellerIcon = _loc6_;
         _loc5_.factionId = _loc7_;
         trace("IL::create_item::",_loc5_.name,_loc5_.category,_loc5_.usable,_loc5_.type,_loc5_.category == Category.VOUCHER,_loc4_.descr);
         return _loc5_;
      }
      
      public function createItemData(param1:Object) : ItemData
      {
         trace("IF::createItemData",param1.dictId);
         if(param1.dictId == undefined)
         {
            param1.dictId = 70;
         }
         var _loc2_:ItemData = this.createItem(param1.dictId,param1.faction_id);
         _loc2_.value = param1.condition_or_stack;
         if(param1.double_handed != undefined)
         {
            _loc2_.oneHanded = !param1.double_handed;
         }
         if(param1.modifiers != undefined)
         {
            _loc2_.broken = this.getItemBroken(param1.modifiers);
            _loc2_.modifiers = (param1.modifiers as Array).concat(_loc2_.modifiersDefault);
         }
         if(param1.rarity_level != undefined)
         {
            _loc2_.rank = param1.rarity_level;
         }
         if(param1.cost)
         {
            _loc2_.cost = param1.cost;
         }
         _loc2_.camoItem = param1.camo_item;
         _loc2_.decalItem = param1.decal_item;
         this.updateItemTipText(_loc2_);
         return _loc2_;
      }
      
      public function fillModifierDescription(param1:Object) : void
      {
         var _loc2_:Object;
         _propsDescriptor = PropertiesDescriptor.getPropertiesDescriptor("properties");
         var _loc3_:Object = param1.props;
         var _loc4_:Object;
         var _loc5_:Number;
         var _loc6_:String;
         var _loc7_:String;
         var _loc8_:String;
         var _loc9_:String;
         trace("modifier::",_loc3_);
         param1.textValue = "";
         for each(_loc4_ in _loc3_)
         {
            _loc5_ = Number(_loc4_.prop_value);
            if(_loc5_ != Number(_loc4_.prop_value))
            {
               _loc5_ = Util.stringToNumber(_loc4_.prop_value);
            }
            _loc2_ = this._propsDescriptor.getPropertyDescription(_loc4_.prop_id);
            if(_loc2_)
            {
               _loc6_ = _loc2_.modifier_type == 0 ? _loc2_.postfix : "%";
               _loc7_ = _loc4_.prop_value >= 0 ? "+" : "";
               _loc8_ = _loc2_.direction == 0 && _loc4_.prop_value < 0 || _loc2_.direction == 1 && _loc4_.prop_value > 0 ? "green" : "red";
               _loc5_ = _loc2_.modifier_type == 0 ? (_loc4_.prop_value) : _loc4_.prop_value * 100;
               _loc9_ = _loc2_.prop_name + ": " + Util.styledChar(_loc7_ + _loc5_ + (_loc6_ ? _loc6_ : ""),_loc8_);
               param1.textValue += _loc9_ + "\n";
            }
         }
      }
      
      public function fillModifiersDescription(param1:ItemData) : void
      {
         _propsDescriptor = PropertiesDescriptor.getPropertiesDescriptor("properties");
         var _loc2_:Array = param1.modifiers;
         trace("fillModifiersDescription::",param1.name,param1.itemId,_loc2_);
         if(!_loc2_)
         {
            return;
         }
         var _loc3_:String = "";
         var _loc4_:Object = new Object();
         var _loc5_:String = param1.name;
         var _loc6_:Object;
         var _loc7_:String;
         var _loc8_:String;
         var _loc9_:String;
         var _loc10_:uint = 0;
         var _loc11_:Object;
         var _loc12_:Object;
         var _loc13_:Number;
         var _loc14_:String;
         var _loc15_:String;
         while(_loc10_ < _loc2_.length)
         {
            _loc11_ = _loc2_[_loc10_].props;
            trace("modifier::",_loc3_,_loc11_);
            _loc2_[_loc10_].textValue = "";
            for each(_loc12_ in _loc11_)
            {
               if(_loc4_[_loc12_.prop_id] == undefined)
               {
                  trace("addPropValue::",_loc12_.prop_id,_loc12_.prop_value);
                  _loc4_[_loc12_.prop_id] = {
                     prop_id:_loc12_.prop_id,
                     prop_value:0,
                     modifier:_loc2_[_loc10_]
                  };
               }
               _loc13_ = Number(_loc12_.prop_value);
               if(_loc13_ != Number(_loc12_.prop_value))
               {
                  _loc13_ = Util.stringToNumber(_loc12_.prop_value);
               }
               _loc4_[_loc12_.prop_id].prop_value += _loc13_;
               _loc6_ = this._propsDescriptor.getPropertyDescription(_loc12_.prop_id);
               if(_loc6_)
               {
                  _loc7_ = _loc6_.modifier_type == 0 ? _loc6_.postfix : "%";
                  _loc8_ = _loc12_.prop_value >= 0 ? "+" : "";
                  _loc9_ = _loc6_.direction == 0 && _loc12_.prop_value < 0 || _loc6_.direction == 1 && _loc12_.prop_value > 0 ? "green" : "red";
                  _loc13_ = _loc6_.modifier_type == 0 ? (_loc12_.prop_value) : _loc12_.prop_value * 100;
                  _loc14_ = _loc6_.prop_name + ": " + Util.styledChar(_loc8_ + _loc13_ + (_loc7_ ? _loc7_ : ""),_loc9_);
                  _loc2_[_loc10_].textValue += _loc14_ + "\n";
               }
            }
            _loc10_++;
         }
         for each(_loc12_ in _loc4_)
         {
            _loc6_ = this._propsDescriptor.getPropertyDescription(_loc12_.prop_id);
            if(_loc6_)
            {
               if(_loc6_.postfix == undefined)
               {
                  _loc6_.postfix = "";
               }
               _loc7_ = _loc6_.modifier_type == 0 ? _loc6_.postfix : "%";
               _loc8_ = _loc12_.prop_value >= 0 ? "+" : "";
               if(_loc6_.modifier_type == undefined)
               {
                  _loc6_.modifier_type = 2;
                  _loc6_.modifier_type = _loc12_.modifier_type;
               }
               _loc13_ = _loc6_.modifier_type == 0 ? (_loc12_.prop_value) : _loc12_.prop_value * 100;
               _loc13_ = Math.round(_loc13_ / 0.01) * 0.01;
               _loc9_ = _loc6_.direction == 0 && _loc12_.prop_value < 0 || _loc6_.direction == 1 && _loc12_.prop_value > 0 ? "green" : "red";
               _loc15_ = _loc6_.prop_name + ": " + Util.styledChar(_loc8_ + _loc13_ + _loc7_,_loc9_);
               if(!_loc12_.modifier.textValue)
               {
                  _loc12_.modifier.textValue = _loc15_;
               }
               _loc3_ += _loc15_ + "\n";
            }
         }
         param1.modifiersDescription = _loc3_;
      }
      
      public function getItemData(param1:uint) : Object
      {
         var _loc2_:Object = this._itemDescriptor.getItemDescription(param1);
         return _loc2_;
      }
      
      public function setRewardIcons(param1:Object) : void
      {
         _rewardIcons = param1;
      }
      
      public function fillRewardItems(param1:Array) : void
      {
         var _loc2_:uint = param1.length;
         var _loc3_:uint = 0;
         var _loc4_:Object;
         var _loc5_:ItemData;
         while(_loc3_ < _loc2_)
         {
            _loc4_ = param1[_loc3_];
            if(_loc4_.type == Reward.ITEM)
            {
               _loc5_ = this.createBaseItem(_loc4_.param);
               if(_loc5_.isStack)
               {
                  _loc5_.value = _loc4_.amount;
               }
               _loc5_.tipText = _loc5_.name;
               _loc4_.data = _loc5_;
            }
            else if(_loc4_.type == Reward.EXPERIENCE)
            {
               _loc5_ = new ItemData();
               _loc5_.icon = this._rewardIcons[_loc4_.type] ? (this._rewardIcons[_loc4_.type] as Icon).clone() : null;
               _loc5_.value = _loc4_.amount;
               _loc5_.name = "";
               _loc5_.isStack = true;
               _loc5_.superCategory = Category.BASE_VOUCHER;
               _loc5_.tipText = LocalizationData.getName("experience");
               _loc4_.data = _loc5_;
            }
            trace("rewardCreated::",_loc5_.name,_loc5_.icon);
            _loc3_++;
         }
      }
      
      protected function updateItemTipText(param1:ItemData) : void
      {
         var _loc2_:String = param1.superCategory != 5 ? Util.styledChar(LocalizationData.getName("equipment_level") + " - " + param1.itemLevel.toString(),"gray") : "";
         var _loc3_:String = Util.styledChar(param1.name,"white") + "\n" + (param1.rank ? Util.styledChar(LocalizationData.getName("modified") + "\n","green") : "") + (param1.ammoText ? LocalizationData.getName("ammo") + ":\n" + param1.ammoText : "") + (_loc2_ ? Util.styledChar(" \n","small") + _loc2_ : "");
         param1.tipText = _loc3_;
      }
      
      protected function getFailModifiers(param1:Object) : Array
      {
         var _loc2_:Array = param1.modifiers != null ? param1.modifiers : [];
         var _loc3_:Array = [];
         var _loc4_:uint = 0;
         var _loc5_:IconContainer;
         var _loc6_:Icon;
         while(_loc4_ < _loc2_.length)
         {
            if(_loc2_[_loc4_].state == 1)
            {
               _loc5_ = IconContainer.getInstance("modifiers");
               _loc6_ = _loc5_.getIcon(_loc2_[_loc4_].icon);
               _loc3_.push({
                  name:_loc2_[_loc4_].name,
                  props:_loc2_[_loc4_].props,
                  icon:_loc6_,
                  state:_loc2_[_loc4_].state,
                  fix_time_general:_loc2_[_loc4_].fix_time_general,
                  fix_cost_general:_loc2_[_loc4_].fix_cost_general,
                  fix_cost_gold:_loc2_[_loc4_].fix_cost_gold
               });
            }
            _loc4_++;
         }
         this.fillModifiersNames(_loc3_);
         return _loc3_;
      }
      
      protected function fillModifiersNames(param1:Array) : void
      {
         var _loc2_:uint = 0;
         var _loc3_:String;
         var _loc4_:Object;
         var _loc5_:Object;
         var _loc6_:Object;
         var _loc7_:String;
         var _loc8_:String;
         var _loc9_:Number;
         var _loc10_:String;
         while(_loc2_ < param1.length)
         {
            _loc3_ = "";
            _loc4_ = param1[_loc2_].props;
            trace("props::",_loc3_,_loc4_,this._propsDescriptor);
            for each(_loc5_ in _loc4_)
            {
               _loc6_ = this._propsDescriptor.getPropertyDescription(_loc5_.prop_id);
               if(_loc6_)
               {
                  if(_loc6_.postfix == undefined)
                  {
                     _loc6_.postfix = "";
                  }
                  _loc7_ = _loc6_.modifier_type == 0 ? _loc6_.postfix : "%";
                  _loc8_ = _loc5_.prop_value >= 0 ? "+" : "";
                  if(_loc6_.modifier_type == undefined)
                  {
                     _loc6_.modifier_type = 2;
                     _loc6_.modifier_type = _loc5_.modifier_type;
                  }
                  if(_loc6_.modifier_type == 2)
                  {
                     throw Error("modifier_type at modifier " + param1[_loc2_].name + " for prop_id " + _loc5_.prop_id + " is invalid");
                  }
                  trace("propValue::",_loc6_.direction,_loc5_.direction);
                  _loc9_ = _loc6_.modifier_type == 0 ? (_loc5_.prop_value) : _loc5_.prop_value * 100;
                  _loc10_ = _loc6_.direction == 0 && _loc5_.prop_value < 0 || _loc6_.direction == 1 && _loc5_.prop_value > 0 ? "green" : "red";
                  _loc3_ += _loc6_.prop_name + ": " + Util.styledChar(_loc8_ + (int(_loc9_) == _loc9_ ? _loc9_.toString() : _loc9_.toFixed(1)) + _loc7_,_loc10_) + "\n";
               }
            }
            param1[_loc2_].name = _loc3_;
            _loc2_++;
         }
      }
      
      private function getItemBroken(param1:Array) : Boolean
      {
         if(!param1)
         {
            return false;
         }
         var _loc2_:uint = param1.length;
         var _loc3_:uint = 0;
         var _loc4_:Object;
         while(_loc3_ < _loc2_)
         {
            _loc4_ = param1[_loc3_];
            if(_loc4_.state)
            {
               return true;
            }
            _loc3_++;
         }
         return false;
      }
   }
}

