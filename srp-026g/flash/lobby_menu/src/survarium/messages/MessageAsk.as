package survarium.messages
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.filters.DropShadowFilter;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import survarium.constants.Colors;
   import survarium.controls.InventoryList;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;
   import survarium.constants.ItemCategory;
   import survarium.constants.ItemPlace;
   import survarium.constants.MoneyType;
   import survarium.constants.ProfileSlot;
   import survarium.constants.QuickSlotSize;
   
   public class MessageAsk extends MessageAccept
   {
      
      // The most steps the count slider offers.
      private static const MAX_STEPS:int = 100;
      
      // The count a buy of a consumable starts at (startCount). The owner, 2026-11-01: "when buying
      // consumables, it should always try to buy at least 30 if enough money (this is for new
      // players who might buy less and not understand why they no longer have consumables in the
      // game)"; "this shouldn't take into account energy drinks and similar items which give
      // passive bonuses per round".
      private static const CONSUMABLE_START_COUNT:int = 30;
      
      protected var _money_type:Boolean = false;
      
      protected var _step:uint = 1;
      
      protected var _price:Number = 1;
      
      public var slider:OptionStepperBtn = new OptionStepperBtn();
      
      public var _windowLabel:TextField = new TextField();
      
      public var _ItemLabel:TextField = new TextField();
      
      public var _CountLabel:TextField = new TextField();
      
      public var _costLabel:TextField = new TextField();
      
      public var _CountPrice:MovieClip = new SrvCostLabelShared();
      
      public var _priceLabel:MovieClip = new SrvCostLabelShared();
      
      public var content:Sprite = new Sprite();
      
      public var _CountLabel2:TextField = new TextField();
      
      public var background:MovieClip = new MessageSmallBackground();
      
      protected var _fakeEnabled:Boolean = false;
      
      public function MessageAsk()
      {
         super();
         addChild(this.background);
         this.background.width = 287;
         this.background.height = 215;
         this.content.x = 0;
         this.content.y = 10;
         btnClose = new CloseButtonDef();
         btnClose.setActualSize(33,33);
         btnClose.x = this.background.width - btnClose.width - 5;
         btnClose.y = 5;
         this.slider.x = (this.background.width - this.slider.width) / 2 + 2;
         this.slider.y = this.background.height - 93;
         addChild(this.content);
         this.slider.editable = true;
         var filters:Array = [new DropShadowFilter(2,45,0,1,3,3)];
         var frLabel:TextFormat = new TextFormat("Calibri",16,10794161,true);
         var frLabelCentered:TextFormat = new TextFormat("Calibri",16,10794161,true);
         frLabelCentered.align = TextFormatAlign.CENTER;
         var frLabelBold:TextFormat = new TextFormat("Calibri",20,Colors.SILVER,true,null,null,null,null,TextFormatAlign.CENTER);
         var offset:int = 10;
         this._ItemLabel.x = 0;
         this._ItemLabel.y = 90;
         this._ItemLabel.width = this.background.width;
         this._ItemLabel.defaultTextFormat = frLabelCentered;
         this._ItemLabel.filters = filters;
         this._ItemLabel.selectable = false;
         addChild(this._ItemLabel);
         this._windowLabel.x = 0;
         this._windowLabel.y = 4;
         this._windowLabel.width = this.background.width;
         this._windowLabel.defaultTextFormat = frLabelBold;
         this._windowLabel.filters = filters;
         this._windowLabel.selectable = false;
         var txF:TextFormat = new TextFormat("Calibri",14,Colors.GRAY,true);
         txF.align = TextFormatAlign.RIGHT;
         this._costLabel.x = 4;
         this._costLabel.width = this.background.width / 2 - 8;
         this._costLabel.defaultTextFormat = txF;
         addChild(this._costLabel);
         txF = new TextFormat("Calibri",21,10794161,true);
         txF.align = TextFormatAlign.LEFT;
         this._CountPrice.y = this.slider.y + 2;
         this._costLabel.y = this._CountPrice.y + 2;
         this._costLabel.selectable = false;
         addChild(this._CountPrice);
         txF = new TextFormat("Calibri",16,16777215);
         this._CountLabel.x = 67;
         this._CountLabel.y = 31;
         this._CountLabel.width = 108;
         this._CountLabel.height = 24;
         this._CountLabel.defaultTextFormat = txF;
         this._CountLabel.filters = filters;
         this._CountLabel.selectable = false;
         txF = new TextFormat("Calibri",11,16777215);
         this._CountLabel2.x = 67;
         this._CountLabel2.y = 153;
         this._CountLabel2.width = 108;
         this._CountLabel2.height = 18;
         this._CountLabel2.defaultTextFormat = txF;
         this._CountLabel2.filters = filters;
         this._CountLabel2.selectable = false;
         addChild(this._CountLabel2);
         this.slider.icon.visible = false;
         addChild(this.slider);
         this._CountPrice.moveIcon = true;
         this._priceLabel.moveIcon = true;
         btnAccept.width = btnCancel.width = (this.background.width - offset * 3 + 6) / 2;
         btnAccept.x = offset;
         btnAccept.y = this.background.height - btnAccept.height - offset - 2;
         addChild(btnAccept);
         btnCancel.x = this.background.width - btnCancel.width - offset;
         btnCancel.y = btnAccept.y;
         addChild(btnCancel);
      }
      
      public function hideSlider() : void
      {
         this.slider.visible = false;
         this._CountLabel.visible = false;
      }
      
      protected function setSliderProp(_max:Number = 100, _step:uint = 1, _default:uint = 0) : void
      {
         if(!_default || _default > _max)
         {
            _default = _step;
         }
         trace("setSliderProp:",_max,_step,_default);
         this.slider.maximum = _max;
         this.slider.minimum = _step > _max ? _max : _step;
         this.slider.stepSize = _step;
         this.slider.value = _default;
         this.onSliderUpdate();
      }
      
      protected function onSliderUpdate(event:IndexEvent = null) : void
      {
         var sliderpos:int = this.slider.value;
         var ammount:int = sliderpos / this._step * this._price;
         trace("onSliderUpdate::",ammount);
         trace("ammount::",sliderpos,this._step,this._price);
         this._CountPrice.cost = ammount;
         if(this._step > 1)
         {
            this._CountLabel.text = "x" + this._step.toString();
         }
         else
         {
            this._CountLabel.text = LocalizationData.getName("item_piece_postfix");
         }
         this._CountLabel.visible = false;
         if(sliderpos == 0)
         {
            btnAccept.enabled = false;
         }
         else
         {
            btnAccept.enabled = true;
         }
         if(this.slider.visible)
         {
            this.updateCostPos();
            this._costLabel.visible = false;
         }
      }
      
      protected function updateSlider(event:ButtonEvent) : void
      {
         var toSet:uint = 0;
         if(event.target.name == "btnMax")
         {
            toSet = this.slider.maximum;
         }
         this.slider.value = toSet;
         this.slider.validateNow();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         LocalizationData.addButton(btnAccept,"accept");
         LocalizationData.addTextField(this._windowLabel,"buy");
         btnAccept.label = LocalizationData.getName("buy").toUpperCase();
         this._costLabel.text = LocalizationData.getName("cost") + ":";
         this.slider.addEventListener(IndexEvent.INDEX_CHANGE,this.onSliderUpdate);
         btnClose.addEventListener(ButtonEvent.CLICK,onMessageCancel,false,0,true);
         this._CountLabel.visible = false;
         if(_data)
         {
            _money_type = _data.isPremium;
            trace("MA::configUI",this._money_type,_data.isPremium);
            if(this._money_type == true)
            {
               this.slider.icon.gotoAndStop(2);
            }
            else
            {
               this.slider.icon.gotoAndStop(1);
            }
            if(_data.clip_size)
            {
               _price = _data.price;
            }
            else
            {
               _price = _data.price;
            }
            if(_data.count > 1 || _data.is_stack)
            {
               this.update_slider();
               this.background.gotoAndStop(1);
            }
            else
            {
               this.hideSlider();
               this._priceLabel.visible = false;
            }
            this._ItemLabel.text = _data.name;
            if(_data.icon)
            {
               var icon:Sprite = _data.icon;
               icon.x = Math.max(0,(this.background.width - icon.width) / 2);
               this.content.addChild(icon);
            }
         }
         trace("configMA::",_data.price,this._step);
         var sliderpos:int = this.slider.value;
         this._priceLabel.cost = _data.price;
         var ammount:int = Math.ceil(sliderpos / this._step * this._price);
         var totalW:int;
         trace("ammount::",sliderpos,this._step,this._price);
         this._CountPrice.moneyType = this._money_type;
         this._CountPrice.textField.textColor = this._money_type ? Colors.GOLD : Colors.PRICE_REG;
         if(this.slider.visible)
         {
            this.updateCostPos();
            this._costLabel.visible = false;
            this._CountPrice.cost = ammount;
         }
         else
         {
            this._CountPrice.cost = int(this._price);
            totalW = this._CountPrice.textField.x + this._CountPrice.textField.textWidth + this._costLabel.textWidth + 8;
            this._costLabel.x = (this.background.width - totalW) / 2 - this._costLabel.getLineMetrics(0).x;
            this._CountPrice.x = this._costLabel.x + this._costLabel.width + 5 - this._CountPrice.textField.getLineMetrics(0).x + (this._money_type ? 14 : 0);
         }
         if(_data.price != undefined && _data.sell == undefined)
         {
            if(this.slider.visible)
            {
               btnAccept.enabled = PlayerInfo.genericMoney >= ammount && sliderpos != 0;
            }
            else if(this._money_type == MoneyType.PREMIUM)
            {
               if(PlayerInfo.premiumMoney < this._price)
               {
                  _fakeEnabled = true;
               }
               btnAccept.enabled = true;
            }
            else
            {
               btnAccept.enabled = PlayerInfo.genericMoney >= 1 * this._price;
            }
         }
      }
      
      protected function updateCostPos() : void
      {
         this._CountPrice.x = this.background.width / 2 + 10 - this._CountPrice.textField.getLineMetrics(0).x + (this._money_type ? 14 : 0);
      }
      
      protected function update_slider() : void
      {
         var _maxpos:Number;
         var _money_tot:uint;
         var _maxTot:Number;
         if(_data.clip_size)
         {
            _step = _data.clip_size;
         }
         if(!this._money_type)
         {
            _money_tot = PlayerInfo.genericMoney;
         }
         else
         {
            _money_tot = PlayerInfo.premiumMoney;
         }
         if(_data.price != undefined && _data.sell == undefined)
         {
            _maxTot = this._price < 1 ? Math.min(Math.max(0,int(int(InventoryList.MAX_FREE_ITEMS_COUNT - _data.currentCount) / this._step) * this._step),MAX_STEPS * this._step) : int(int(_money_tot / this._price)) * this._step;
         }
         else
         {
            _maxTot = Math.ceil(_data.count / this._step) * this._step;
         }
         trace("race1::",this._price,_data.price,_maxTot,_data.count,this._step,_data.sell,PlayerInfo.pocketSize,_data.quick_slot_item_size,_money_tot);
         var fullPocket:Boolean = _data.quick_slot_item_size == QuickSlotSize.WHOLE;
         if(_data.targetSlot && _data.targetSlot != ItemPlace.INVENTORY)
         {
            if(_data.price != undefined && _data.sell == undefined && ProfileSlot.isPocket(_data.targetSlot))
            {
               // a buy dropped on a pocket: what the pocket does not take stays in the inventory
               // (LobbyMenu.relocate_to_paperdoll), so the pocket's size does not cap it
               _maxpos = Math.min(MAX_STEPS * this._step,_maxTot);
            }
            else if(_data.targetSlot < ItemPlace.INVENTORY)
            {
               _maxpos = Math.min(MAX_STEPS * this._step,_maxTot,fullPocket ? 1 : int(PlayerInfo.pocketSize));
            }
            else if(!_data.originData)
            {
               _maxpos = Math.min(MAX_STEPS * this._step,_maxTot,int((PlayerInfo.maxWeight - PlayerInfo.curWeight) / _data.weight));
            }
            else
            {
               _maxpos = Math.min(MAX_STEPS * this._step,_maxTot,int((PlayerInfo.maxWeight - PlayerInfo.curWeight + _data.originData.weight * _data.originData.condition_or_stack) / _data.weight));
            }
         }
         else
         {
            _maxpos = Math.min(_maxTot,MAX_STEPS * this._step);
         }
         if(this._money_type == MoneyType.PREMIUM)
         {
            if(_maxpos == 0)
            {
               _fakeEnabled = true;
            }
            _maxpos = Math.max(1,_maxpos);
         }
         else
         {
            _maxpos = Math.max(0,_maxpos);
         }
         trace("CreateAskSellWindow::",_maxpos,_maxTot,_data.originData,_data.targetSlot,this._step,_data.currentCount,this._price,this._fakeEnabled);
         if(_data.sell != undefined)
         {
            this.setSliderProp(Math.min(_maxpos,_data.count != undefined ? _data.count : _maxpos),this._step);
         }
         else if(!_data.originData)
         {
            this.setSliderProp(_maxpos,this._step,this.startCount(_maxpos));
         }
         else
         {
            this.setSliderProp(_maxpos,this._step,_data.originData.condition_or_stack);
         }
         var sliderpos:int = this.slider.value;
         var ammount:int = sliderpos / this._step * this._price;
         trace("ammount::",sliderpos,this._step,this._price);
         if(this.slider.visible)
         {
            this._CountPrice.cost = ammount;
         }
         else
         {
            this._CountPrice.cost = this._price;
         }
         if(_data.price != undefined && _data.sell == undefined)
         {
            if(this.slider.visible)
            {
               btnAccept.enabled = (_money_tot >= ammount || this._money_type == MoneyType.PREMIUM && _maxpos == 1) && sliderpos != 0;
            }
            else
            {
               btnAccept.enabled = _money_tot >= 1 * this._price || this._money_type == MoneyType.PREMIUM;
            }
         }
      }
      
      // Where a buy's slider starts, up to max: a consumable a match uses up (a drug, a trap, a
      // grenade: the pockets' categories) at CONSUMABLE_START_COUNT, or max when less; 0 (one step,
      // setSliderProp) for anything else: ammo, a booster (it fills a whole pocket and works while
      // carried, items.md §8), an item no stack, a relocation (no price).
      private function startCount(max:Number) : uint
      {
         var category:* = _data.category;
         var consumable:Boolean = category == ItemCategory.MEDKITS || category == ItemCategory.BOOBY_TRAPS || category == ItemCategory.GRENADES;
         if(_data.price == undefined || !_data.is_stack || !consumable || _data.quick_slot_item_size == QuickSlotSize.WHOLE)
         {
            return 0;
         }
         return uint(Math.min(CONSUMABLE_START_COUNT,max));
      }
      
      protected function changeMoneyType(event:ButtonEvent = null) : void
      {
         _money_type = !this._money_type;
         if(this._money_type == true)
         {
         }
         this.update_slider();
      }
      
      override protected function cleanUp(e:Event = null) : void
      {
         super.cleanUp(e);
         this.content.removeChild(this.content.getChildAt(0));
      }
      
      protected function onNoMoneyEnought() : void
      {
         var object:Object = {
            text:LocalizationData.getName("cant_buy_premium_message"),
            onYes:this.goToStore,
            label_ok:LocalizationData.getName("btn_add_premium_message"),
            label_cancel:LocalizationData.getName("cancel")
         };
         Message.showMessage("apply",object,true);
      }
      
      protected function goToStore() : void
      {
         MessageMoneyExchange.buyGold();
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         super.onMessageAccept();
         if(this._fakeEnabled)
         {
            this.onNoMoneyEnought();
            return;
         }
         if(this.slider.visible)
         {
            _data.count = uint(uint(this.slider.value / this._step));
         }
         else
         {
            _data.count = uint(1);
         }
         trace("buy_ok_clicked",_data.uid,_data.count,_data.seller,_data.targetSlot,_data.sourceSlot,_data.id);
         if(_data.sourceSlot == ItemPlace.BUNDLE)
         {
            trace("buy_shop_item",uint(_data.uid));
            ExternalInterface.call("buy_shop_item",uint(_data.uid));
         }
         else if(_data.targetSlot == ItemPlace.INVENTORY)
         {
            ExternalInterface.call("buy_ok_clicked",_data.uid,uint(_data.count),uint(_data.seller),this._money_type);
         }
         if(_data.onYes)
         {
            _data.onYes(_data);
            _data.onYes = null;
            _data.icon = null;
         }
      }
   }
}

