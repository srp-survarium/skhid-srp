package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.text.StyleSheet;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.interfaces.IDataRenderer;
   import survarium.Util;
   import survarium.constants.Colors;
   import survarium.item.ItemFactory;
   import survarium.item.WeaponDetails;
   import survarium.item.WeaponDetailsView;
   import survarium.player.LocalPlayerProfile;
   import survarium.player.MatchEffects;
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

      private var _weaponDetails:WeaponDetails;
      private var _detailsText:WeaponDetailsView;
      private var _detailsMode:String = "none";
      
      public function ItemInfoRenderer()
      {
         super();
         this.init();
         addEventListener(Event.ADDED_TO_STAGE,detailsAdded,false,0,true);
         addEventListener(Event.REMOVED_FROM_STAGE,detailsRemoved,false,0,true);
      }

      private function detailsAdded(e:Event) : void
      {
         addEventListener(Event.ENTER_FRAME,detailsFrame,false,0,true);
      }

      private function detailsRemoved(e:Event) : void
      {
         removeEventListener(Event.ENTER_FRAME,detailsFrame);
      }

      private function detailsFrame(e:Event) : void
      {
         var mode:String = detailsMode();
         if(mode == _detailsMode) return;
         itemCompare.refreshComparison();
         updateDetailsText();
         layoutDetails();
         dispatchEvent(new Event(Event.RESIZE));
      }

      // What the card shows: its rows (none, compact or expanded with Shift), neutral colours
      // (Ctrl), a weapon's stats in a match (Alt, docs/notes/alt-weapon-stats.md).
      private function detailsMode() : String
      {
         if(_data == null) return "none";
         var rows:String = _weaponDetails == null ? "none" : MainScene.isShift ? "expanded" : "compact";
         return rows + (MainScene.isCtrl ? "-neutral" : "") + (MainScene.isAlt && _data.isWeapon ? "-match" : "");
      }

      private function updateDetailsText() : void
      {
         _detailsMode = detailsMode();
         if(_detailsText == null && _weaponDetails != null)
         {
            _detailsText = new WeaponDetailsView();
            addChild(_detailsText);
         }
         if(_detailsText != null)
         {
            _detailsText.visible = _weaponDetails != null;
            var effects:MatchEffects = LocalPlayerProfile.heldMatchEffects(_data);
            _detailsText.show(effects == null ? _weaponDetails : WeaponDetails.read(_data,effects),MainScene.isShift,itemCompare,effects);
         }
      }

      // Transfer the native warning graphics without leaving the full item's
      // deferred comparison layout attached to them.
      public static function warningSkin() : ItemInfoRenderer
      {
         var skin:ItemInfoRenderer = new InventoryItemHint();
         skin.itemCompare.removeEventListener(Event.RESIZE,skin.layoutDetails);
         return skin;
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
         this.itemCompare.addEventListener(Event.RESIZE,this.layoutDetails);
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
         // the chosen kit's weapons (slot 1 or 2) in gold (owner 2026-10-22: "it should in gold
         // highlight your weapon, if it is in the list"; weapon_scopes.md)
         var kit:Array = UpgradeWindow.kitWeapons();
         var owned:Object = new Object();
         i = 0;
         while(i < weaponIds.length)
         {
            weaponDescription = ItemDescriptor.getItemDescriptor("InventoryItems").getItemDescription(weaponIds[i]);
            weaponName = Boolean(weaponDescription) ? weaponDescription.name : null;
            if(Boolean(weaponName) && names.indexOf(weaponName) == -1)
            {
               names.push(weaponName);
            }
            if(Boolean(weaponName) && kit.indexOf(weaponIds[i]) != -1)
            {
               owned[weaponName] = true;
            }
            i = ++i;
         }
         i = 0;
         while(i < names.length)
         {
            if(owned[names[i]])
            {
               names[i] = Util.styledChar(names[i],"gold");
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
         _weaponDetails = WeaponDetails.read(item);
         updateDetailsText();
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
         this.layoutHeader(this.naturalHeaderHeight());
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
         this.layoutDetails();
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

      }

      private function naturalHeaderHeight() : Number
      {
         return textFieldDescription.visible ? textFieldDescription.y + textFieldDescription.height : (itemAmmoName.visible ? 55 : 37);
      }

      private function layoutHeader(headerHeight:Number) : void
      {
         topBlackContainer.height = headerHeight;
         propsTopLine.visible = false;
         propsTopLine.y = topBlackContainer.y + topBlackContainer.height;
         itemCompare.y = propsTopLine.y + 10;
      }

      // Optics descriptions can wrap. Reserve the taller natural header on
      // both weapon cards so every comparison row starts at the same height.
      public function alignComparisonHeader(peer:ItemInfoRenderer) : void
      {
         var ownHeight:Number = naturalHeaderHeight();
         var peerHeight:Number = peer == null ? 0 : peer.naturalHeaderHeight();
         if(peer != null && _data != null && peer._data != null && _data.isWeapon && peer._data.isWeapon)
            ownHeight = peerHeight = Math.max(ownHeight,peerHeight);
         layoutHeader(ownHeight);
         layoutDetails();
         if(peer != null)
         {
            peer.layoutHeader(peerHeight);
            peer.layoutDetails();
         }
      }

      // The comparison can add/remove rows after the item has been assigned.
      // One layout owner sizes the card and every section for either callback.
      private function layoutDetails(e:Event = null) : void
      {
         // Native timeline children can dispatch RESIZE before this card has
         // data/providers. The final data setter lays it out once all exist.
         if(_data == null || modifiersList.dataProvider == null || defaultModifiersList.dataProvider == null || criticalModifiersList.dataProvider == null) return;
         if(modifiersList.container == null || defaultModifiersList.container == null || criticalModifiersList.container == null) return;
         if(e != null) updateDetailsText();
         var regularHeight:uint = this.modifiersList.container.height * this.modifiersList.scaleY;
         var defaultHeight:uint = this.defaultModifiersList.container.height * this.defaultModifiersList.scaleY;
         var criticalHeight:uint = this.criticalModifiersList.container.height * this.criticalModifiersList.scaleY;
         var hasDefaultModifiers:Boolean = this.defaultModifiersList.dataProvider.length > 0;
         var hasCriticalModifiers:Boolean = this.criticalModifiersList.dataProvider.length > 0;
         var hasModifiers:Boolean = Boolean(regularHeight) || Boolean(criticalHeight) || Boolean(defaultHeight);
         trace("IIR::FRED",this.itemCompare,this.defaultModfiersBack,this.propsBottomLine,this.bottomBackContainer,this.bottomLine,this.background);
         this.defaultModfiersBack.height = defaultHeight + 11;
         this.propsBottomLine.y = Math.max(this.itemCompare.y + this.itemCompare.height + (this.itemCompare.height > 5 ? 5 : -15),this.propsTopLine.y);
         if(_detailsText != null && _detailsText.visible)
         {
            _detailsText.y = this.propsBottomLine.y + 2;
            this.propsBottomLine.y = _detailsText.y + _detailsText.height + 6;
         }
         this.bottomBackContainer.y = this.propsBottomLine.y;
         this.bottomBackContainer.height = regularHeight + criticalHeight + defaultHeight + (hasModifiers ? 14 : 0) + (hasCriticalModifiers ? 40 : 0) + (Boolean(regularHeight) || Boolean(criticalHeight) ? 5 : 0);
         this.bottomLine.y = this.bottomBackContainer.y + this.bottomBackContainer.height;
         this.bottomLine.visible = false;
         this.background.height = this.bottomLine.y + 5;
         this.criticalSeparator.visible = hasCriticalModifiers;
         this.defaultModfiersBack.visible = this.defaultModifiersList.visible = hasDefaultModifiers;
         this.defaultModifiersList.y = this.propsBottomLine.y + 10;
         this.defaultModfiersBack.y = this.propsBottomLine.y + 2;
         this.modifiersList.y = this.defaultModifiersList.y + defaultHeight + (hasDefaultModifiers ? 5 : 0);
         this.criticalSeparator.y = this.modifiersList.y + regularHeight + 15;
         this.criticalModifiersList.y = this.criticalSeparator.visible ? this.criticalSeparator.y + this.criticalSeparator.height - 10 : this.modifiersList.y;
         this.criticalSeparator.textField.text = LocalizationData.getName("critical_damages");
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

