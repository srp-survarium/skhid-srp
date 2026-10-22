package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.text.StyleSheet;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.interfaces.IDataRenderer;
   import survarium.constants.Colors;
   import survarium.item.ItemFactory;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.ItemData;
   import survarium.data.ItemDescriptor;
   import survarium.data.LocalizationData;
   import survarium.upgrade.UpgradeWindow;
   import survarium.constants.ItemCategory;
   import survarium.constants.ItemPlace;
   import survarium.constants.ItemRank;
   import survarium.constants.ModificationState;
   import survarium.constants.SuperCategory;
   
   public class ItemInfoRenderer extends Sprite implements IDataRenderer
   {
      
      public var topBlackContainer:MovieClip;
      
      public var orangeOverlay:MovieClip;
      
      public var itemCompare:ItemCompare;
      
      public var modifiersList:ButtonBarDynam;
      
      public var defaultModifiersList:ButtonBarDynam;
      
      public var criticalModifiersList:ButtonBarDynam;
      
      public var bottomLine:MovieClip;
      
      public var propsTopLine:MovieClip;
      
      public var propsBottomLine:MovieClip;
      
      public var criticalSeparator:MovieClip;
      
      public var bottomBackContainer:MovieClip;
      
      public var background:MovieClip;
      
      public var overBackground:MovieClip;
      
      public var defaultModfiersBack:MovieClip;
      
      public var manufacturerIcon:Sprite;
      
      public var itemAmmoName:FixedText;
      
      public var itemName:FixedText;
      
      public var textFieldItemLevel:TextField;
      
      public var textFieldStatus:TextField;
      
      public var textFieldDescription:TextField;
      
      protected var _data:Object = null;
      
      protected var _spr:Sprite = new Sprite();
      
      protected var _displayOrange:Boolean = false;
      
      public function ItemInfoRenderer()
      {
         super();
         this.init();
      }
      
      override public function get width() : Number
      {
         return this.background.width;
      }
      
      override public function set width(value:Number) : void
      {
         super.width = value;
      }
      
      protected function set sellerIcon(value:Sprite) : void
      {
         if(this.manufacturerIcon.numChildren)
         {
            this.manufacturerIcon.removeChildAt(0);
         }
         if(this.manufacturerIcon && value)
         {
            this.manufacturerIcon.addChild(value);
         }
      }
      
      protected function init() : void
      {
         var styleSheet:StyleSheet = new StyleSheet();
         var style:Object;
         style = new Object();
         style.color = "#b3ea97";
         styleSheet.setStyle("current",style);
         style = new Object();
         style.color = "#D79342";
         styleSheet.setStyle("yellow",style);
         style = new Object();
         style.color = "#4A4C4B";
         styleSheet.setStyle("gray",style);
         this.itemAmmoName.textField.styleSheet = styleSheet;
         this.modifiersList.setMaxFit(true);
         this.defaultModifiersList.setMaxFit(true);
         this.criticalModifiersList.setMaxFit(true);
         addChildAt(this._spr,5);
      }
      
      protected function sortModifiers(regularModifiers:Array, defaultModifiers:Array, criticalModifiers:Array) : void
      {
         var allModifiers:Array = this._data.modifiers != null ? this._data.modifiers : [];
         var icons:IconContainer = IconContainer.getInstance("modifiers");
         var i:uint = 0;
         var modifier:Object;
         var modifierIcon:Icon;
         var entry:Object;
         while(i < allModifiers.length)
         {
            modifier = allModifiers[i];
            modifierIcon = icons.getIcon(modifier.icon);
            entry = {
               icon:modifierIcon,
               label:modifier.textValue
            };
            if(modifier.state == ModificationState.DEFECT)
            {
               criticalModifiers.push(entry);
            }
            else if(modifier.isDefault)
            {
               defaultModifiers.push(entry);
            }
            else
            {
               regularModifiers.push(entry);
            }
            i++;
         }
      }
      
      public function displayOrange(show:Boolean) : void
      {
         _displayOrange = show;
      }
      
      public function displaylabel(show:Boolean) : void
      {
         this.textFieldStatus.visible = show;
      }
      
      protected function scopeHintText(item:ItemData) : String
      {
         var scope:Object = null;
         var weaponIds:Array = null;
         var names:Array = null;
         var weaponName:String = null;
         var weaponDescription:Object = null;
         var i:int = 0;
         var hint:String = "";
         if(item.isWeapon)
         {
            scope = UpgradeWindow.attachedScope(item);
            if(Boolean(scope))
            {
               return LocalizationData.getName("scope_slot") + ": " + scope.name;
            }
            if(Boolean(UpgradeWindow.stockScopes[item.dictId]))
            {
               return LocalizationData.getName("scope_slot") + ": " + LocalizationData.getName("default_scope");
            }
            return null;
         }
         if(item.category != ItemCategory.SCOPES)
         {
            return null;
         }
         if(Boolean(item.itemDescription))
         {
            hint = item.itemDescription;
         }
         weaponIds = UpgradeWindow.weaponsFor(item.dictId);
         names = new Array();
         i = 0;
         while(i < weaponIds.length)
         {
            weaponDescription = ItemDescriptor.getItemDescriptor("InventoryItems").getItemDescription(weaponIds[i]);
            weaponName = Boolean(weaponDescription) ? weaponDescription.name : null;
            if(Boolean(weaponName) && names.indexOf(weaponName) == -1)
            {
               names.push(weaponName);
            }
            i = ++i;
         }
         if(names.length > 0)
         {
            hint += (hint.length > 0 ? "\n" : "") + LocalizationData.getName("can_be_installed_on") + ": " + names.join(", ");
         }
         if(Boolean(item.itemId) && Boolean(UpgradeWindow.instance) && item.descriptor != ItemPlace.SHOP)
         {
            names = UpgradeWindow.wearers(item);
            if(Boolean(names) && names.length > 0)
            {
               hint += (hint.length > 0 ? "\n" : "") + LocalizationData.getName("installed_on") + ": " + names.join(", ");
            }
         }
         return hint.length > 0 ? hint : null;
      }
      
      public function set data(value:Object) : void
      {
         var nameColor:uint = 0;
         var criticalModifiers:Array = null;
         var iconFrame:uint = 0;
         this._data = value;
         var item:ItemData = this._data as ItemData;
         if(!item)
         {
            return;
         }
         trace(this.itemName,this.itemAmmoName,this.textFieldDescription,this.topBlackContainer);
         trace("updateIIR::0",item);
         if(Boolean(this._data.rank))
         {
            if(this._data.rank == ItemRank.ONE)
            {
               nameColor = Colors.RANK_1;
            }
            else if(this._data.rank == ItemRank.TWO)
            {
               nameColor = Colors.RANK_2;
            }
            else
            {
               nameColor = Colors.RANK_3;
            }
         }
         else
         {
            nameColor = Colors.LIGHT_GRAY;
         }
         // Skhid: a golden item's name is gold, whatever its rank (golden_weapons.md, premium_armour.md)
         if(ItemFactory.isGolden(item.dictId))
         {
            nameColor = Colors.GOLD;
         }
         this.itemName.textField.textColor = nameColor;
         this.itemName.text = item.name;
         this.sellerIcon = Boolean(item.sellerIcon) ? (item.sellerIcon as Icon).clone() : null;
         this.orangeOverlay.visible = false;
         this.itemName.x = this.itemAmmoName.x = Boolean(item.sellerIcon) ? 30 : 10;
         if(Boolean(item.itemDescription && (item.category == ItemCategory.MEDKITS || item.superCategory == SuperCategory.UPGRADES || item.category == ItemCategory.BUNDLE) && item.category != ItemCategory.GENERIC_MONEY) && Boolean(item.category != ItemCategory.PREMIUM_MONEY) && item.category != ItemCategory.PREMIUM_MONEY)
         {
            this.textFieldDescription.y = item.isStack && item.descriptor < ItemPlace.INVENTORY ? 52 : 32;
            this.textFieldDescription.x = 11;
            this.textFieldDescription.width = 234;
            this.textFieldDescription.styleSheet = MainScene.textStyleSheet;
            this.textFieldDescription.text = item.itemDescription + (item.descriptor == ItemPlace.BUNDLE ? "\n\n" + LocalizationData.getName("bundle_clik_hint") : "");
            this.textFieldDescription.autoSize = TextFieldAutoSize.LEFT;
            this.textFieldDescription.visible = true;
         }
         else
         {
            this.textFieldDescription.visible = false;
         }
         if(item.isWeapon)
         {
            this.itemAmmoName.visible = true;
            trace("isWeapon::",item.ammoText);
            this.itemAmmoName.text = item.ammoText;
         }
         else if(item.isStack && item.descriptor < ItemPlace.INVENTORY && item.category != ItemCategory.PREMIUM_MONEY)
         {
            this.itemAmmoName.text = item.tipText;
            this.itemAmmoName.visible = true;
         }
         else
         {
            this.itemAmmoName.visible = false;
         }
         var scopeText:String = this.scopeHintText(item);
         if(Boolean(scopeText))
         {
            this.textFieldDescription.y = item.isWeapon ? 52 : 32;
            this.textFieldDescription.x = 11;
            this.textFieldDescription.width = 234;
            this.textFieldDescription.styleSheet = MainScene.textStyleSheet;
            this.textFieldDescription.text = scopeText;
            this.textFieldDescription.autoSize = TextFieldAutoSize.LEFT;
            this.textFieldDescription.visible = true;
         }
         this.textFieldItemLevel.text = item.itemLevel > 0 ? LocalizationData.getName("clan_players_level_label") + " " + String(item.itemLevel) : "";
         trace("INFOCAT::",item.isStack,item.descriptor);
         if(item.isWeapon || item.superCategory == SuperCategory.ITEMS)
         {
            iconFrame = item.iconId + (item.isWeapon ? 1 : 100);
            trace("a",item.iconId,iconFrame);
            this.textFieldItemLevel.visible = true;
         }
         else if(item.superCategory == SuperCategory.EQUIPMENT)
         {
            trace("b",item.category);
            this.textFieldItemLevel.visible = true;
         }
         else if(item.superCategory == SuperCategory.AMMO)
         {
            this.textFieldItemLevel.visible = true;
         }
         else
         {
            trace("c");
            this.textFieldItemLevel.visible = false;
         }
         if(this.textFieldDescription.visible)
         {
            this.topBlackContainer.height = this.textFieldDescription.height + this.textFieldDescription.y;
         }
         else
         {
            this.topBlackContainer.height = this.itemAmmoName.visible ? 55 : 37;
         }
         this.propsTopLine.visible = false;
         this.propsTopLine.y = this.topBlackContainer.y + this.topBlackContainer.height;
         this.itemCompare.y = this.propsTopLine.y + 10;
         this.itemCompare.setPrimaryWeapon(item);
         trace("updateIIR::2",this._data.name,this._data.modifiers);
         var regularModifiers:Array = [];
         var defaultModifiers:Array = [];
         criticalModifiers = [];
         this.sortModifiers(regularModifiers,defaultModifiers,criticalModifiers);
         InventoryList.logModifiers(criticalModifiers);
         trace("modifiersList",this.modifiersList,this.defaultModifiersList,this.criticalModifiersList);
         this.modifiersList.dataProvider = new DataProvider(regularModifiers);
         this.modifiersList.validateNow();
         this.defaultModifiersList.dataProvider = new DataProvider(defaultModifiers);
         this.defaultModifiersList.validateNow();
         this.criticalModifiersList.dataProvider = new DataProvider(criticalModifiers);
         this.criticalModifiersList.validateNow();
         var regularHeight:uint = this.modifiersList.container.height * this.modifiersList.scaleY;
         var defaultHeight:uint = this.defaultModifiersList.container.height * this.defaultModifiersList.scaleY;
         var criticalHeight:uint = this.criticalModifiersList.container.height * this.criticalModifiersList.scaleY;
         var hasDefaultModifiers:Boolean = defaultModifiers.length > 0;
         var hasModifiers:Boolean = Boolean(regularHeight) || Boolean(criticalHeight) || Boolean(defaultHeight);
         trace("IIR::FRED",this.itemCompare,this.defaultModfiersBack,this.propsBottomLine,this.bottomBackContainer,this.bottomLine,this.background);
         this.defaultModfiersBack.height = defaultHeight + 11;
         this.propsBottomLine.y = Math.max(this.itemCompare.y + this.itemCompare.height + (this.itemCompare.height > 5 ? 5 : -15),this.propsTopLine.y);
         this.bottomBackContainer.y = this.propsBottomLine.y;
         this.bottomBackContainer.height = regularHeight + criticalHeight + defaultHeight + (hasModifiers ? 14 : 0) + (Boolean(criticalModifiers.length) ? 40 : 0) + (Boolean(regularHeight) || Boolean(criticalHeight) ? 5 : 0);
         this.bottomLine.y = this.bottomBackContainer.y + this.bottomBackContainer.height;
         this.bottomLine.visible = false;
         this.background.height = this.bottomLine.y + 5;
         trace("IIR::FRED2",item.tipText,item.ammoText,item.itemDescription);
         this.criticalSeparator.visible = criticalModifiers.length > 0;
         this.defaultModfiersBack.visible = this.defaultModifiersList.visible = hasDefaultModifiers;
         this.defaultModifiersList.y = this.propsBottomLine.y + 10;
         this.defaultModfiersBack.y = this.propsBottomLine.y + 2;
         this.modifiersList.y = this.defaultModifiersList.y + defaultHeight + (hasDefaultModifiers ? 5 : 0);
         this.criticalSeparator.y = this.modifiersList.y + regularHeight + 15;
         this.criticalModifiersList.y = this.criticalSeparator.visible ? this.criticalSeparator.y + this.criticalSeparator.height - 10 : this.modifiersList.y;
         this.criticalSeparator.textField.text = LocalizationData.getName("critical_damages");
         trace("ItemHint::",item.name,item.descriptor);
         if(item.descriptor == ItemPlace.SHOP)
         {
            this.textFieldStatus.text = LocalizationData.getName("item_hint_label_shop");
         }
         else if(item.descriptor < ItemPlace.INVENTORY)
         {
            this.textFieldStatus.text = LocalizationData.getName("equipped");
         }
         else
         {
            this.textFieldStatus.text = LocalizationData.getName("item_hint_label");
         }
         if(this._displayOrange)
         {
            this.orangeOverlay.y = this.propsTopLine.y;
            this.orangeOverlay.height = this.propsBottomLine.y - this.orangeOverlay.y;
         }
      }
      
      public function get data() : Object
      {
         return this._data;
      }
   }
}

