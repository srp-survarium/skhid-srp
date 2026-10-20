package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.text.StyleSheet;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.interfaces.IDataRenderer;
   import survarium.constants.Category;
   import survarium.constants.Colors;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.ItemData;
   import survarium.data.ItemDescriptor;
   import survarium.data.LocalizationData;
   import survarium.upgrade.UpgradeWindow;
   
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
      
      override public function set width(param1:Number) : void
      {
         super.width = param1;
      }
      
      protected function set sellerIcon(param1:Sprite) : void
      {
         if(this.manufacturerIcon.numChildren)
         {
            this.manufacturerIcon.removeChildAt(0);
         }
         if(this.manufacturerIcon && param1)
         {
            this.manufacturerIcon.addChild(param1);
         }
      }
      
      protected function init() : void
      {
         var _loc1_:StyleSheet = new StyleSheet();
         var _loc2_:Object;
         _loc2_ = new Object();
         _loc2_.color = "#b3ea97";
         _loc1_.setStyle("current",_loc2_);
         _loc2_ = new Object();
         _loc2_.color = "#D79342";
         _loc1_.setStyle("yellow",_loc2_);
         _loc2_ = new Object();
         _loc2_.color = "#4A4C4B";
         _loc1_.setStyle("gray",_loc2_);
         this.itemAmmoName.textField.styleSheet = _loc1_;
         this.modifiersList.setMaxFit(true);
         this.defaultModifiersList.setMaxFit(true);
         this.criticalModifiersList.setMaxFit(true);
         addChildAt(this._spr,5);
      }
      
      protected function sortModifiers(param1:Array, param2:Array, param3:Array) : void
      {
         var _loc4_:Array = this._data.modifiers != null ? this._data.modifiers : [];
         var _loc5_:IconContainer = IconContainer.getInstance("modifiers");
         var _loc6_:uint = 0;
         var _loc7_:Object;
         var _loc8_:Icon;
         var _loc9_:Object;
         while(_loc6_ < _loc4_.length)
         {
            _loc7_ = _loc4_[_loc6_];
            _loc8_ = _loc5_.getIcon(_loc7_.icon);
            _loc9_ = {
               icon:_loc8_,
               label:_loc7_.textValue
            };
            if(_loc7_.state == 1)
            {
               param3.push(_loc9_);
            }
            else if(_loc7_.isDefault)
            {
               param2.push(_loc9_);
            }
            else
            {
               param1.push(_loc9_);
            }
            _loc6_++;
         }
      }
      
      public function displayOrange(param1:Boolean) : void
      {
         _displayOrange = param1;
      }
      
      public function displaylabel(param1:Boolean) : void
      {
         this.textFieldStatus.visible = param1;
      }
      
      protected function scopeHintText(param1:ItemData) : String
      {
         var _loc2_:Object = null;
         var _loc3_:Array = null;
         var _loc4_:Array = null;
         var _loc5_:String = null;
         var _loc6_:Object = null;
         var _loc7_:int = 0;
         var _loc8_:String = "";
         if(param1.isWeapon)
         {
            _loc2_ = UpgradeWindow.attachedScope(param1);
            if(Boolean(_loc2_))
            {
               return LocalizationData.getName("scope_slot") + ": " + _loc2_.name;
            }
            if(Boolean(UpgradeWindow.stockScopes[param1.dictId]))
            {
               return LocalizationData.getName("scope_slot") + ": " + LocalizationData.getName("default_scope");
            }
            return null;
         }
         if(param1.category != 22)
         {
            return null;
         }
         if(Boolean(param1.itemDescription))
         {
            _loc8_ = param1.itemDescription;
         }
         _loc3_ = UpgradeWindow.weaponsFor(param1.dictId);
         _loc4_ = new Array();
         _loc7_ = 0;
         while(_loc7_ < _loc3_.length)
         {
            _loc6_ = ItemDescriptor.getItemDescriptor("InventoryItems").getItemDescription(_loc3_[_loc7_]);
            _loc5_ = Boolean(_loc6_) ? _loc6_.name : null;
            if(Boolean(_loc5_) && _loc4_.indexOf(_loc5_) == -1)
            {
               _loc4_.push(_loc5_);
            }
            _loc7_ = ++_loc7_;
         }
         if(_loc4_.length > 0)
         {
            _loc8_ += (_loc8_.length > 0 ? "\n" : "") + LocalizationData.getName("can_be_installed_on") + ": " + _loc4_.join(", ");
         }
         if(Boolean(param1.itemId) && Boolean(UpgradeWindow.instance) && param1.descriptor != 200)
         {
            _loc4_ = UpgradeWindow.wearers(param1);
            if(Boolean(_loc4_) && _loc4_.length > 0)
            {
               _loc8_ += (_loc8_.length > 0 ? "\n" : "") + LocalizationData.getName("installed_on") + ": " + _loc4_.join(", ");
            }
         }
         return _loc8_.length > 0 ? _loc8_ : null;
      }
      
      public function set data(param1:Object) : void
      {
         var _loc3_:uint = 0;
         var _loc6_:Array = null;
         var _loc12_:uint = 0;
         this._data = param1;
         var _loc2_:ItemData = this._data as ItemData;
         if(!_loc2_)
         {
            return;
         }
         trace(this.itemName,this.itemAmmoName,this.textFieldDescription,this.topBlackContainer);
         trace("updateIIR::0",_loc2_);
         if(Boolean(this._data.rank))
         {
            if(this._data.rank == 1)
            {
               _loc3_ = Colors.RANK_1;
            }
            else if(this._data.rank == 2)
            {
               _loc3_ = Colors.RANK_2;
            }
            else
            {
               _loc3_ = Colors.RANK_3;
            }
         }
         else
         {
            _loc3_ = Colors.LIGHT_GRAY;
         }
         this.itemName.textField.textColor = _loc3_;
         this.itemName.text = _loc2_.name;
         this.sellerIcon = Boolean(_loc2_.sellerIcon) ? (_loc2_.sellerIcon as Icon).clone() : null;
         this.orangeOverlay.visible = false;
         this.itemName.x = this.itemAmmoName.x = Boolean(_loc2_.sellerIcon) ? 30 : 10;
         if(Boolean(_loc2_.itemDescription && (_loc2_.category == 10 || _loc2_.superCategory == 5 || _loc2_.category == 100) && _loc2_.category != 28) && Boolean(_loc2_.category != 29) && _loc2_.category != Category.GOLD)
         {
            this.textFieldDescription.y = _loc2_.isStack && _loc2_.descriptor < 100 ? 52 : 32;
            this.textFieldDescription.x = 11;
            this.textFieldDescription.width = 234;
            this.textFieldDescription.styleSheet = MainScene.textStyleSheet;
            this.textFieldDescription.text = _loc2_.itemDescription + (_loc2_.descriptor == 300 ? "\n\n" + LocalizationData.getName("bundle_clik_hint") : "");
            this.textFieldDescription.autoSize = TextFieldAutoSize.LEFT;
            this.textFieldDescription.visible = true;
         }
         else
         {
            this.textFieldDescription.visible = false;
         }
         if(_loc2_.isWeapon)
         {
            this.itemAmmoName.visible = true;
            trace("isWeapon::",_loc2_.ammoText);
            this.itemAmmoName.text = _loc2_.ammoText;
         }
         else if(_loc2_.isStack && _loc2_.descriptor < 100 && _loc2_.category != Category.GOLD)
         {
            this.itemAmmoName.text = _loc2_.tipText;
            this.itemAmmoName.visible = true;
         }
         else
         {
            this.itemAmmoName.visible = false;
         }
         var scopeText:String = this.scopeHintText(_loc2_);
         if(Boolean(scopeText))
         {
            this.textFieldDescription.y = _loc2_.isWeapon ? 52 : 32;
            this.textFieldDescription.x = 11;
            this.textFieldDescription.width = 234;
            this.textFieldDescription.styleSheet = MainScene.textStyleSheet;
            this.textFieldDescription.text = scopeText;
            this.textFieldDescription.autoSize = TextFieldAutoSize.LEFT;
            this.textFieldDescription.visible = true;
         }
         this.textFieldItemLevel.text = _loc2_.itemLevel > 0 ? LocalizationData.getName("clan_players_level_label") + " " + String(_loc2_.itemLevel) : "";
         trace("INFOCAT::",_loc2_.isStack,_loc2_.descriptor);
         if(_loc2_.isWeapon || _loc2_.superCategory == 4)
         {
            _loc12_ = _loc2_.iconId + (_loc2_.isWeapon ? 1 : 100);
            trace("a",_loc2_.iconId,_loc12_);
            this.textFieldItemLevel.visible = true;
         }
         else if(_loc2_.superCategory == 1)
         {
            trace("b",_loc2_.category);
            this.textFieldItemLevel.visible = true;
         }
         else if(_loc2_.superCategory == 3)
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
         this.itemCompare.setPrimaryWeapon(_loc2_);
         trace("updateIIR::2",this._data.name,this._data.modifiers);
         var _loc4_:Array = [];
         var _loc5_:Array = [];
         _loc6_ = [];
         this.sortModifiers(_loc4_,_loc5_,_loc6_);
         InventoryList.logModifiers(_loc6_);
         trace("modifiersList",this.modifiersList,this.defaultModifiersList,this.criticalModifiersList);
         this.modifiersList.dataProvider = new DataProvider(_loc4_);
         this.modifiersList.validateNow();
         this.defaultModifiersList.dataProvider = new DataProvider(_loc5_);
         this.defaultModifiersList.validateNow();
         this.criticalModifiersList.dataProvider = new DataProvider(_loc6_);
         this.criticalModifiersList.validateNow();
         var _loc7_:uint = this.modifiersList.container.height * this.modifiersList.scaleY;
         var _loc8_:uint = this.defaultModifiersList.container.height * this.defaultModifiersList.scaleY;
         var _loc9_:uint = this.criticalModifiersList.container.height * this.criticalModifiersList.scaleY;
         var _loc10_:Boolean = _loc5_.length > 0;
         var _loc11_:Boolean = Boolean(_loc7_) || Boolean(_loc9_) || Boolean(_loc8_);
         trace("IIR::FRED",this.itemCompare,this.defaultModfiersBack,this.propsBottomLine,this.bottomBackContainer,this.bottomLine,this.background);
         this.defaultModfiersBack.height = _loc8_ + 11;
         this.propsBottomLine.y = Math.max(this.itemCompare.y + this.itemCompare.height + (this.itemCompare.height > 5 ? 5 : -15),this.propsTopLine.y);
         this.bottomBackContainer.y = this.propsBottomLine.y;
         this.bottomBackContainer.height = _loc7_ + _loc9_ + _loc8_ + (_loc11_ ? 14 : 0) + (Boolean(_loc6_.length) ? 40 : 0) + (Boolean(_loc7_) || Boolean(_loc9_) ? 5 : 0);
         this.bottomLine.y = this.bottomBackContainer.y + this.bottomBackContainer.height;
         this.bottomLine.visible = false;
         this.background.height = this.bottomLine.y + 5;
         trace("IIR::FRED2",_loc2_.tipText,_loc2_.ammoText,_loc2_.itemDescription);
         this.criticalSeparator.visible = _loc6_.length > 0;
         this.defaultModfiersBack.visible = this.defaultModifiersList.visible = _loc10_;
         this.defaultModifiersList.y = this.propsBottomLine.y + 10;
         this.defaultModfiersBack.y = this.propsBottomLine.y + 2;
         this.modifiersList.y = this.defaultModifiersList.y + _loc8_ + (_loc10_ ? 5 : 0);
         this.criticalSeparator.y = this.modifiersList.y + _loc7_ + 15;
         this.criticalModifiersList.y = this.criticalSeparator.visible ? this.criticalSeparator.y + this.criticalSeparator.height - 10 : this.modifiersList.y;
         this.criticalSeparator.textField.text = LocalizationData.getName("critical_damages");
         trace("ItemHint::",_loc2_.name,_loc2_.descriptor);
         if(_loc2_.descriptor == 200)
         {
            this.textFieldStatus.text = LocalizationData.getName("item_hint_label_shop");
         }
         else if(_loc2_.descriptor < 100)
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

