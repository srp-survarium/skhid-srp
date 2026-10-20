package survarium.ammo
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import scaleform.clik.controls.Label;
   import scaleform.clik.controls.NumericStepper;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.events.SliderEvent;
   import survarium.Util;
   import survarium.constants.Colors;
   import survarium.controls.IconRenderer;
   import survarium.controls.NumericStepperSrv;
   import survarium.controls.PlayerInfo;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.data.PropertiesDescriptor;
   
   public class AmmoShareElement extends UIComponent
   {
      
      public var id:int = 0;
      
      public var index:uint = 0;
      
      public var arrayIndex:uint = 0;
      
      public var itemLevel:uint = 999;
      
      public var pair:uint = 0;
      
      public var group:uint = 0;
      
      public var updated:Boolean = false;
      
      public var available:uint = 0;
      
      public var ammoHintArea:ToolTip;
      
      public var weaponSlot:IconRenderer;
      
      public var slider:AmmoSlider;
      
      public var textField:TextField;
      
      public var stepper_backpack:NumericStepper;
      
      public var backpack_label:Label;
      
      public var stepper_storage:NumericStepperSrv;
      
      public var background_tx:TextField;
      
      public var lock:Sprite;
      
      public var textFieldLock:TextField;
      
      public var textFieldLock2:TextField;
      
      public var textFieldEquipped:TextField;
      
      public var textFieldStorage:TextField;
      
      public var lock_mc:Sprite;
      
      public var hintButton:ToolTip;
      
      protected var _ignoreChange:Boolean = false;
      
      protected var _item_price:uint = 0;
      
      protected var _maxWeight:Number = 0;
      
      protected var _max:int = 0;
      
      protected var _data:ItemData;
      
      protected var _owner:Sprite;
      
      protected var _topTarget:AmmoShareElement;
      
      protected var _clipSize:int = 0;
      
      protected var _clipsInBag:Number = 0;
      
      public function AmmoShareElement()
      {
         super();
         this.textFieldEquipped.visible = this.textFieldStorage.visible = false;
         this.backpack_label.visible = this.stepper_storage.visible = this.slider.visible = false;
         this.stepper_backpack.visible = false;
         this.lock_mc.visible = false;
         this.weaponSlot.visible = false;
         gotoAndStop(1);
      }
      
      override public function set x(param1:Number) : void
      {
         super.x = int(param1 * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      public function set owner(param1:Sprite) : void
      {
         _owner = param1;
      }
      
      public function isPremium() : Boolean
      {
         return this._data && !this._data.canBuy && this._data.premiumCost;
      }
      
      override public function get width() : Number
      {
         return 274;
      }
      
      override public function get height() : Number
      {
         return 144;
      }
      
      override public function set height(param1:Number) : void
      {
         super.height = param1;
      }
      
      public function setData(param1:ItemData = null, param2:Boolean = false) : void
      {
         gotoAndStop(2);
         this.textFieldEquipped.visible = this.textFieldStorage.visible = true;
         mouseChildren = true;
         mouseEnabled = true;
         this.backpack_label.visible = this.stepper_storage.visible = this.slider.visible = true;
         this.weaponSlot.visible = true;
         var _loc3_:ItemData = new ItemData();
         _loc3_.name = param1.name;
         _loc3_.dictId = param1.dictId;
         _loc3_.category = param1.category;
         _loc3_.itemId = param1.itemId;
         _loc3_.slotId = param1.slotId;
         _loc3_.icon = param1.icon.clone();
         _loc3_.dragIcon = param1.icon.clone();
         _loc3_.isStack = param1.isStack;
         _loc3_.value = param1.value;
         _loc3_.count = param1.count;
         _loc3_.properties = param1.properties;
         _loc3_.clipSize = param1.clipSize;
         _loc3_.weight = param1.weight;
         _loc3_.slotTarget = param1.slotTarget;
         _loc3_.weaponTarget = param1.weaponTarget;
         _loc3_.cost = param1.cost;
         _loc3_.clipsInBag = param1.clipsInBag;
         _loc3_.canBuy = param1.canBuy;
         _loc3_.unlockLevelLabel = param1.unlockLevelLabel;
         _loc3_.itemLevel = param1.itemLevel;
         _loc3_.premiumCost = param1.premiumCost;
         itemLevel = param1.itemLevel;
         _data = _loc3_;
         var _loc4_:Boolean = !(param1.canBuy == false && this._data.value == 0);
         var _loc5_:Boolean = this.isPremium();
         _item_price = _loc5_ ? this._data.premiumCost : this._data.cost;
         var _loc6_:Object = param1.properties;
         if(_loc6_)
         {
            _loc8_ = "";
            _loc9_ = PropertiesDescriptor.getPropertiesDescriptor("properties");
            for each(_loc10_ in _loc6_)
            {
               _loc11_ = _loc10_;
               _loc12_ = _loc9_.getPropertyDescription(_loc11_.prop_id);
               _loc8_ += _loc12_.prop_name + ": " + Util.styledChar(String(_loc11_.prop_value + (_loc12_.postfix ? _loc12_.postfix : "")),"highlight") + "\n";
            }
            _loc8_ += Util.styledChar(String(_loc3_.itemLevel),"small") + "\n";
            _loc8_ += LocalizationData.getName("item_level") + ": " + Util.styledChar(String(_loc3_.itemLevel),"highlight") + "\n";
            this.ammoHintArea.tipData = {
               tipText:_loc8_,
               isPremium:_loc5_ || _loc3_.isPremium,
               cost:this._item_price,
               lock:!_loc4_ || _loc5_,
               lockText:_loc3_.unlockLevelLabel
            };
         }
         _clipSize = _loc3_.clipSize;
         _clipsInBag = _loc3_.clipsInBag;
         this.textField.text = _loc3_.name;
         this.textField.textColor = _loc5_ ? Colors.GOLD : Colors.SILVER;
         this.slider.shiftInterval = -3;
         this.slider.renderer = "ItemRoundCell";
         this.slider.alpha = 1;
         this.slider.enabled = true;
         this.weaponSlot.content = this._data.icon;
         this.slider.removeEventListener(SliderEvent.VALUE_CHANGE,this.onElementSlider);
         var _loc7_:int = this._data.value;
         var _loc8_:String;
         var _loc9_:*;
         var _loc10_:Object;
         var _loc11_:Object;
         var _loc12_:Object;
         var _loc13_:ColorTransform;
         _max = int(int(this.available * this._clipsInBag) * this._clipSize);
         if(!this._data.canBuy)
         {
            _max = Math.min(this._max,int(int(_loc7_ / this._clipSize) * this._clipSize));
         }
         trace("ASE::max:",this._max,this.available,this._clipsInBag,this._clipSize,this._data.canBuy,this.index,"inv::",_loc7_,this._data.count,this._data.premiumCost);
         focusable = true;
         _ignoreChange = true;
         this.stepper_storage.editable = true;
         this.stepper_storage.minimum = this._data.value;
         this.stepper_storage.maximum = this._item_price == 0 ? PlayerInfo.MAX_FREE_ITEMS_COUNT : uint.MAX_VALUE;
         this.stepper_storage.stepSize = this._clipSize;
         this.stepper_backpack.minimum = 0;
         this.stepper_backpack.maximum = this._max;
         this.stepper_backpack.stepSize = this._clipSize;
         this.stepper_backpack.invalidateData();
         this.stepper_backpack.enabled = _loc4_;
         this.stepper_storage.enabled = _loc4_;
         this.stepper_backpack.focusable = _loc4_;
         this.stepper_storage.focusable = _loc4_;
         this.stepper_storage.mouseEnabled = _loc4_;
         this.stepper_backpack.mouseEnabled = _loc4_;
         this.stepper_storage.mouseChildren = _loc4_;
         this.stepper_backpack.mouseChildren = _loc4_;
         this.slider.enabled = _loc4_;
         this.slider.minimum = 0;
         this.slider.maximum = this._max;
         this.slider.available = Math.min(this.clips(_loc7_),this._max);
         this.slider.total = this._max;
         this.slider.minimum_storage = _loc7_;
         if(this.slider.value_storage != _loc7_)
         {
            this.slider.value_storage = _loc7_;
         }
         this.slider.maxSize = this.available;
         this.slider.snapInterval = this._clipSize;
         if(param2)
         {
            this.slider.value = this._data.count;
         }
         this.updateText();
         this.updateSteppers();
         _ignoreChange = false;
         if(_loc5_)
         {
            _loc13_ = new ColorTransform(0.7,0.7,0.7,1,136,103,15,0);
            this.stepper_storage.nextBtn.transform.colorTransform = _loc13_;
            this.stepper_storage.prevBtn.transform.colorTransform = _loc13_;
            this.textField.x = 133;
            this.lock_mc.y = this.textField.y + 6;
            this.lock_mc.x = this.textField.x - this.lock_mc.width - 4;
         }
         else
         {
            this.textField.x = 123;
            this.lock_mc.y = this.slider.y + 4;
            this.lock_mc.x = this.slider.x + 3;
         }
         this.lock_mc.visible = !_loc4_ || _loc5_;
         if(!_loc4_)
         {
            if(this._data.unlockLevelLabel)
            {
               this.textFieldLock2.text = this._data.unlockLevelLabel;
               this.lock.visible = true;
               if(this.hintButton)
               {
                  this.hintButton.visible = this.lock.visible;
                  this.hintButton.tipData = {name:LocalizationData.getName(param1.lockText) + " \"" + this.styledChar(param1.unlockLevelLabel,"yellow") + "\""};
               }
            }
            this.textFieldLock2.y = this.lock_mc.y - 5;
            this.textFieldLock2.x = this.lock_mc.x + 12;
            if(this.hintButton)
            {
               this.hintButton.y = this.lock_mc.x - 4;
               this.hintButton.y = this.lock_mc.y - 3;
            }
         }
         else
         {
            if(this.hintButton)
            {
               this.hintButton.visible = false;
            }
            this.textFieldLock.text = "";
            this.textFieldLock2.text = "";
            this.lock.visible = false;
         }
         this.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.onElementSlider);
      }
      
      protected function styledChar(param1:String, param2:String) : String
      {
         return "<" + param2 + ">" + param1 + "</" + param2 + ">";
      }
      
      public function ignoreChange() : void
      {
         _ignoreChange = true;
      }
      
      public function getData() : ItemData
      {
         return this._data;
      }
      
      public function update(param1:ItemData = null, param2:Boolean = false) : void
      {
         var _loc3_:int = this.slider.value;
         this.setData(param1);
         _ignoreChange = true;
         if(this.slider.value != _loc3_ && (param2 || _loc3_ <= param1.count))
         {
            this.slider.value = _loc3_;
         }
         _ignoreChange = false;
      }
      
      public function disable() : void
      {
         _data = null;
         focusable = false;
         mouseChildren = false;
         mouseEnabled = false;
      }
      
      public function clear() : void
      {
         updated = false;
         _topTarget = null;
      }
      
      public function getWeight() : Number
      {
         if(!this._data)
         {
            return 0;
         }
         return this._data.weight * this.slider.value / this.slider.snapInterval;
      }
      
      public function getCount() : uint
      {
         if(!this._data)
         {
            return 0;
         }
         var _loc1_:uint = Math.ceil(this.slider.value / this._clipSize / this._clipsInBag);
         return _loc1_;
      }
      
      public function setCount(param1:uint, param2:Boolean = true) : void
      {
         if(!this._data)
         {
            return;
         }
         var _loc3_:Number = int((this.available - param1) * this._clipsInBag) * this._data.weight;
         var _loc4_:int = Math.max(int(_loc3_ / this._data.weight) * this._clipSize,0);
         this.slider.available = Math.min(this.clips(this._data.value),_loc4_);
         var _loc5_:int = this.clips(Math.max(this.slider.available,_loc4_));
         if(this.slider.total != _loc5_)
         {
            this.slider.total = _loc5_;
         }
         if(param2)
         {
            if(this.slider.total < this.slider.value)
            {
               this.slider.value = this.slider.total;
            }
         }
      }
      
      public function get_required_price() : uint
      {
         if(!this._data)
         {
            return 0;
         }
         var _loc1_:Number = Math.max(0,(this.slider.value_storage - this._data.value) / this.slider.snapInterval);
         trace("get_required_price::",_loc1_,this._item_price);
         return _loc1_ * this._item_price;
      }
      
      public function selectNewAmmo(param1:ListEvent) : void
      {
         var _loc2_:AmmoShareLite = this._owner as AmmoShareLite;
         if(!param1.itemData)
         {
            this.disable();
            updated = true;
            _loc2_.reflow(this);
            dispatchEvent(new Event(Event.CHANGE));
            return;
         }
         this.slider.removeEventListener(SliderEvent.VALUE_CHANGE,this.onElementSlider);
         updated = true;
         if(param1.itemData)
         {
            this.setData(param1.itemData as ItemData,true);
         }
         this.updateText();
         this.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.onElementSlider);
         _loc2_.reflow(this);
         dispatchEvent(new Event(Event.CHANGE));
      }
      
      public function updateStorage() : void
      {
         if(!this._data)
         {
            return;
         }
         if(!this._data.canBuy)
         {
            this.stepper_storage.maximum = this._data.value - this.slider.value;
         }
         else if(this._item_price)
         {
            this.stepper_storage.maximum = this._data.value - this.slider.value + this._data.clipSize * int((PlayerInfo.genericMoney - (this._owner as AmmoShareLite).getCost(this)) / this._item_price);
         }
         else
         {
            this.stepper_storage.maximum = PlayerInfo.MAX_FREE_ITEMS_COUNT - this.stepper_backpack.value;
         }
         this.stepper_storage.invalidateData();
      }
      
      override protected function configUI() : void
      {
         if(this.hintButton)
         {
            this.hintButton.tooltipRendererName = "SrvProfileTipRendererStyled";
         }
         this.ammoHintArea.tooltipRendererName = "SrvAmmoHintRenderer";
         this.stepper_backpack.visible = false;
         this.stepper_backpack.addEventListener(IndexEvent.INDEX_CHANGE,this.stepperChange);
         this.stepper_storage.addEventListener(IndexEvent.INDEX_CHANGE,this.stepperChange);
         this.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.updateText);
         this.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.onElementSlider);
         this.backpack_label.visible = this.stepper_storage.visible = this._data != null;
         if(this.background_tx)
         {
            this.background_tx.text = LocalizationData.getName("ammo_label");
         }
         this.textFieldEquipped.text = LocalizationData.getName("ammo_equipped_label");
         this.textFieldStorage.text = LocalizationData.getName("ammo_storage_label");
         this.lock.mouseEnabled = this.textFieldLock2.mouseEnabled = this.textFieldLock.mouseEnabled = false;
      }
      
      protected function onAutoBuyBtnClicked(param1:ButtonEvent) : void
      {
         dispatchEvent(new Event(Event.CHANGE));
      }
      
      protected function stepperChange(param1:Event) : void
      {
         if(param1.target == this.stepper_backpack)
         {
            this.slider.value = param1.target.value;
         }
         else if(param1.target == this.stepper_storage)
         {
            this.slider.value_storage = param1.target.value + this.slider.value;
         }
      }
      
      protected function onElementSlider(param1:SliderEvent) : void
      {
         var _loc2_:AmmoShareLite;
         if(!focusable)
         {
            return;
         }
         if(this._owner is AmmoShareLite)
         {
            trace("ammo_count_updated",uint(this.slider.value),uint(this.slider.available));
            ExternalInterface.call("ammo_count_updated",uint(this.slider.value),uint(this.slider.available));
            _loc2_ = this._owner as AmmoShareLite;
            _loc2_.reflow(this);
            if(this._ignoreChange && this.get_required_price() == 0)
            {
               return;
            }
            updated = true;
            dispatchEvent(new Event(Event.CHANGE));
         }
      }
      
      protected function updateText(param1:SliderEvent = null) : void
      {
         if(!this._data)
         {
            return;
         }
         this.updateStorage();
         this.updateSteppers();
      }
      
      protected function clips(param1:Number) : int
      {
         return int(param1 / this._clipSize) * this._clipSize;
      }
      
      protected function updateSteppers() : void
      {
         this.stepper_backpack.value = this.slider.value;
         this.backpack_label.text = String(this.slider.value);
         this.stepper_storage.minimum = this._data.value - this.slider.value;
         this.stepper_storage.value = this.slider.value_storage - this.slider.value;
      }
   }
}

