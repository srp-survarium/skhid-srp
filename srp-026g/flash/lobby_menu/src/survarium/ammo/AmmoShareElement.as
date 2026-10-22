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
      
      override public function set x(value:Number) : void
      {
         super.x = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      public function set owner(value:Sprite) : void
      {
         _owner = value;
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
      
      override public function set height(value:Number) : void
      {
         super.height = value;
      }
      
      public function setData(item:ItemData = null, applyCount:Boolean = false) : void
      {
         gotoAndStop(2);
         this.textFieldEquipped.visible = this.textFieldStorage.visible = true;
         mouseChildren = true;
         mouseEnabled = true;
         this.backpack_label.visible = this.stepper_storage.visible = this.slider.visible = true;
         this.weaponSlot.visible = true;
         var copy:ItemData = new ItemData();
         copy.name = item.name;
         copy.dictId = item.dictId;
         copy.category = item.category;
         copy.itemId = item.itemId;
         copy.slotId = item.slotId;
         copy.icon = item.icon.clone();
         copy.dragIcon = item.icon.clone();
         copy.isStack = item.isStack;
         copy.value = item.value;
         copy.count = item.count;
         copy.properties = item.properties;
         copy.clipSize = item.clipSize;
         copy.weight = item.weight;
         copy.slotTarget = item.slotTarget;
         copy.weaponTarget = item.weaponTarget;
         copy.cost = item.cost;
         copy.clipsInBag = item.clipsInBag;
         copy.canBuy = item.canBuy;
         copy.unlockLevelLabel = item.unlockLevelLabel;
         copy.itemLevel = item.itemLevel;
         copy.premiumCost = item.premiumCost;
         itemLevel = item.itemLevel;
         _data = copy;
         var unlocked:Boolean = !(item.canBuy == false && this._data.value == 0);
         var premium:Boolean = this.isPremium();
         _item_price = premium ? this._data.premiumCost : this._data.cost;
         var properties:Object = item.properties;
         if(properties)
         {
            hint = "";
            descriptor = PropertiesDescriptor.getPropertiesDescriptor("properties");
            for each(entry in properties)
            {
               prop = entry;
               propInfo = descriptor.getPropertyDescription(prop.prop_id);
               hint += propInfo.prop_name + ": " + Util.styledChar(String(prop.prop_value + (propInfo.postfix ? propInfo.postfix : "")),"highlight") + "\n";
            }
            hint += Util.styledChar(String(copy.itemLevel),"small") + "\n";
            hint += LocalizationData.getName("item_level") + ": " + Util.styledChar(String(copy.itemLevel),"highlight") + "\n";
            this.ammoHintArea.tipData = {
               tipText:hint,
               isPremium:premium || copy.isPremium,
               cost:this._item_price,
               lock:!unlocked || premium,
               lockText:copy.unlockLevelLabel
            };
         }
         _clipSize = copy.clipSize;
         _clipsInBag = copy.clipsInBag;
         this.textField.text = copy.name;
         this.textField.textColor = premium ? Colors.GOLD : Colors.SILVER;
         this.slider.shiftInterval = -3;
         this.slider.renderer = "ItemRoundCell";
         this.slider.alpha = 1;
         this.slider.enabled = true;
         this.weaponSlot.content = this._data.icon;
         this.slider.removeEventListener(SliderEvent.VALUE_CHANGE,this.onElementSlider);
         var ownedCount:int = this._data.value;
         var hint:String;
         var descriptor:*;
         var entry:Object;
         var prop:Object;
         var propInfo:Object;
         var goldTint:ColorTransform;
         _max = int(int(this.available * this._clipsInBag) * this._clipSize);
         if(!this._data.canBuy)
         {
            _max = Math.min(this._max,int(int(ownedCount / this._clipSize) * this._clipSize));
         }
         trace("ASE::max:",this._max,this.available,this._clipsInBag,this._clipSize,this._data.canBuy,this.index,"inv::",ownedCount,this._data.count,this._data.premiumCost);
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
         this.stepper_backpack.enabled = unlocked;
         this.stepper_storage.enabled = unlocked;
         this.stepper_backpack.focusable = unlocked;
         this.stepper_storage.focusable = unlocked;
         this.stepper_storage.mouseEnabled = unlocked;
         this.stepper_backpack.mouseEnabled = unlocked;
         this.stepper_storage.mouseChildren = unlocked;
         this.stepper_backpack.mouseChildren = unlocked;
         this.slider.enabled = unlocked;
         this.slider.minimum = 0;
         this.slider.maximum = this._max;
         this.slider.available = Math.min(this.clips(ownedCount),this._max);
         this.slider.total = this._max;
         this.slider.minimum_storage = ownedCount;
         if(this.slider.value_storage != ownedCount)
         {
            this.slider.value_storage = ownedCount;
         }
         this.slider.maxSize = this.available;
         this.slider.snapInterval = this._clipSize;
         if(applyCount)
         {
            this.slider.value = this._data.count;
         }
         this.updateText();
         this.updateSteppers();
         _ignoreChange = false;
         if(premium)
         {
            goldTint = new ColorTransform(0.7,0.7,0.7,1,136,103,15,0);
            this.stepper_storage.nextBtn.transform.colorTransform = goldTint;
            this.stepper_storage.prevBtn.transform.colorTransform = goldTint;
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
         this.lock_mc.visible = !unlocked || premium;
         if(!unlocked)
         {
            if(this._data.unlockLevelLabel)
            {
               this.textFieldLock2.text = this._data.unlockLevelLabel;
               this.lock.visible = true;
               if(this.hintButton)
               {
                  this.hintButton.visible = this.lock.visible;
                  this.hintButton.tipData = {name:LocalizationData.getName(item.lockText) + " \"" + this.styledChar(item.unlockLevelLabel,"yellow") + "\""};
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
      
      protected function styledChar(content:String, style:String) : String
      {
         return "<" + style + ">" + content + "</" + style + ">";
      }
      
      public function ignoreChange() : void
      {
         _ignoreChange = true;
      }
      
      public function getData() : ItemData
      {
         return this._data;
      }
      
      public function update(item:ItemData = null, force:Boolean = false) : void
      {
         var prevValue:int = this.slider.value;
         this.setData(item);
         _ignoreChange = true;
         if(this.slider.value != prevValue && (force || prevValue <= item.count))
         {
            this.slider.value = prevValue;
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
         var count:uint = Math.ceil(this.slider.value / this._clipSize / this._clipsInBag);
         return count;
      }
      
      public function setCount(usedSlots:uint, clamp:Boolean = true) : void
      {
         if(!this._data)
         {
            return;
         }
         var freeWeight:Number = int((this.available - usedSlots) * this._clipsInBag) * this._data.weight;
         var capacity:int = Math.max(int(freeWeight / this._data.weight) * this._clipSize,0);
         this.slider.available = Math.min(this.clips(this._data.value),capacity);
         var newTotal:int = this.clips(Math.max(this.slider.available,capacity));
         if(this.slider.total != newTotal)
         {
            this.slider.total = newTotal;
         }
         if(clamp)
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
         var clipsToBuy:Number = Math.max(0,(this.slider.value_storage - this._data.value) / this.slider.snapInterval);
         trace("get_required_price::",clipsToBuy,this._item_price);
         return clipsToBuy * this._item_price;
      }
      
      public function selectNewAmmo(event:ListEvent) : void
      {
         var ammoShare:AmmoShareLite = this._owner as AmmoShareLite;
         if(!event.itemData)
         {
            this.disable();
            updated = true;
            ammoShare.reflow(this);
            dispatchEvent(new Event(Event.CHANGE));
            return;
         }
         this.slider.removeEventListener(SliderEvent.VALUE_CHANGE,this.onElementSlider);
         updated = true;
         if(event.itemData)
         {
            this.setData(event.itemData as ItemData,true);
         }
         this.updateText();
         this.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.onElementSlider);
         ammoShare.reflow(this);
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
      
      protected function onAutoBuyBtnClicked(event:ButtonEvent) : void
      {
         dispatchEvent(new Event(Event.CHANGE));
      }
      
      protected function stepperChange(event:Event) : void
      {
         if(event.target == this.stepper_backpack)
         {
            this.slider.value = event.target.value;
         }
         else if(event.target == this.stepper_storage)
         {
            this.slider.value_storage = event.target.value + this.slider.value;
         }
      }
      
      protected function onElementSlider(event:SliderEvent) : void
      {
         var ammoShare:AmmoShareLite;
         if(!focusable)
         {
            return;
         }
         if(this._owner is AmmoShareLite)
         {
            trace("ammo_count_updated",uint(this.slider.value),uint(this.slider.available));
            ExternalInterface.call("ammo_count_updated",uint(this.slider.value),uint(this.slider.available));
            ammoShare = this._owner as AmmoShareLite;
            ammoShare.reflow(this);
            if(this._ignoreChange && this.get_required_price() == 0)
            {
               return;
            }
            updated = true;
            dispatchEvent(new Event(Event.CHANGE));
         }
      }
      
      protected function updateText(event:SliderEvent = null) : void
      {
         if(!this._data)
         {
            return;
         }
         this.updateStorage();
         this.updateSteppers();
      }
      
      protected function clips(amount:Number) : int
      {
         return int(amount / this._clipSize) * this._clipSize;
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

