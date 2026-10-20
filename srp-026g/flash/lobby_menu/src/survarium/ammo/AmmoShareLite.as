package survarium.ammo
{
   import flash.display.DisplayObject;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.SliderEvent;
   import survarium.controls.PlayerInfo;
   import survarium.controls.ScrollContent;
   import survarium.data.ItemData;
   import survarium.interfaces.IAmmoShare;
   
   public class AmmoShareLite extends UIComponent implements IAmmoShare
   {
      
      public static const GROUP_AMMO:uint = 1;
      
      public static const GROUP_ITEMS:uint = 2;
      
      public static const PAIR_AMMO1:uint = 1;
      
      public static const PAIR_AMMO2:uint = 2;
      
      public static const PAIR_ITEMS:uint = 3;
      
      protected static var _id:AmmoShareLite;
      
      protected static var _itemCard:*;
      
      public var tx_weigth:TextField;
      
      public var activeIndex:int = 0;
      
      public var aviableWeight:Number = 0;
      
      protected var _autoBuyCheckBox:Button;
      
      protected var _updateFunction:Function;
      
      protected var _softUpdate:Boolean = false;
      
      protected var _slotsN:int = 0;
      
      protected var _ammo_indexes:Array = [8,9,11,12,19,20,21,22];
      
      protected var _ammoList:Array;
      
      protected var _profileID:uint;
      
      protected var _data:Array;
      
      protected var _dataSlots:Array;
      
      protected var _isShared:Boolean = false;
      
      protected var _relocateArray:Array;
      
      protected var _isReflow:Boolean = false;
      
      protected var _tempAmmoLst:Array = new Array();
      
      protected var _elemetns:Array = new Array();
      
      protected var _elemetnsSlots:Array = new Array();
      
      protected var _ammoBags:uint = 0;
      
      protected var _pocketSize:uint = 0;
      
      protected var _targetElement:AmmoShareElement;
      
      protected var _items_container:ScrollContent = new ScrollContent();
      
      protected var _btn_yes:Button;
      
      protected var _btn_cancel:Button;
      
      protected var _totalPrice:uint = 0;
      
      protected var _totalPricePremium:uint = 0;
      
      protected var _unsetted_slot:int = 0;
      
      protected var _emptyElements:Array = new Array();
      
      protected var _renderer:Class;
      
      public function AmmoShareLite()
      {
         super();
         _id = this;
         addEventListener(Event.ENTER_FRAME,this.config);
         name = "AmmoShareLite instance";
      }
      
      public static function getInstance() : AmmoShareLite
      {
         if(!_id)
         {
            new AmmoShareLite();
         }
         return _id;
      }
      
      public static function setItemCard(param1:*) : void
      {
         _itemCard = param1;
         _itemCard.addEventListener(Event.COMPLETE,getInstance().acceptItemCard);
      }
      
      public function set updateFunction(param1:Function) : void
      {
         _updateFunction = param1;
      }
      
      public function registerWeightTextField(param1:TextField) : void
      {
         tx_weigth = param1;
      }
      
      public function registerRenderer(param1:Class) : void
      {
         _renderer = param1;
         var _loc2_:uint = 0;
         var _loc3_:AmmoShareElement;
         while(_loc2_ < 6)
         {
            _loc3_ = new this._renderer();
            this._emptyElements.push(_loc3_);
            _loc2_++;
         }
      }
      
      protected function getEmptyElement() : AmmoShareElement
      {
         var _loc1_:uint = 0;
         var _loc2_:AmmoShareElement;
         while(_loc1_ < this._emptyElements.length)
         {
            _loc2_ = this._emptyElements[_loc1_];
            if(!_loc2_.parent)
            {
               return _loc2_;
            }
            _loc1_++;
         }
         return null;
      }
      
      public function set autoBuyCheckBox(param1:Button) : void
      {
         _autoBuyCheckBox = param1;
      }
      
      public function get autoBuyCheckBox() : Button
      {
         return this._autoBuyCheckBox;
      }
      
      public function set acceptButton(param1:Button) : void
      {
         _btn_yes = param1;
         this._btn_yes.enabled = false;
         this._btn_yes.addEventListener(ButtonEvent.CLICK,this.setAmmo,false,0,true);
      }
      
      public function set cancelButton(param1:Button) : void
      {
         _btn_cancel = param1;
         this._btn_cancel.enabled = false;
         this._btn_cancel.addEventListener(ButtonEvent.CLICK,this.cancel,false,0,true);
      }
      
      public function set ammoBags(param1:uint) : void
      {
         _ammoBags = param1;
      }
      
      public function set pocketSize(param1:uint) : void
      {
         _pocketSize = param1;
      }
      
      public function getItemsList() : *
      {
         return null;
      }
      
      public function getAmmoRenderer(param1:uint) : DisplayObject
      {
         trace("getAmmoRenderer",param1);
         if(this._elemetns)
         {
            trace("_elemetns",this._elemetns[param1]);
            if(this._elemetns[param1])
            {
               this._elemetns[param1].arrayIndex = param1;
            }
            return this._elemetns[param1];
         }
         trace("return _null");
         return null;
      }
      
      public function highlightAmmoRendererSlider(param1:uint) : DisplayObject
      {
         trace("getAmmoRendererSlider",param1);
         if(this._elemetns)
         {
            trace("_elemetns",this._elemetns[param1]);
            if(this._elemetns[param1])
            {
               this._elemetns[param1].arrayIndex = param1;
            }
            return this._elemetns[param1].slider;
         }
         trace("return _null");
         return null;
      }
      
      public function highlightAmmoRendererCell(param1:uint, param2:Boolean) : void
      {
         trace("highlightAmmoRendererCell",param1);
         if(this._elemetns)
         {
            trace("_elemetns",this._elemetns[param1]);
            if(this._elemetns[param1])
            {
               this._elemetns[param1].slider.hoverCells(param2);
            }
         }
      }
      
      public function swapElements(param1:int, param2:int) : void
      {
         var _loc3_:Object;
         var _loc4_:Object;
         var _loc5_:uint = this._elemetnsSlots.length;
         var _loc6_:int = 0;
         var _loc7_:Object;
         while(_loc6_ < _loc5_)
         {
            _loc7_ = this._elemetnsSlots[_loc6_];
            if(_loc7_.index == param1)
            {
               _loc3_ = _loc7_;
            }
            if(_loc7_.index == param2)
            {
               _loc4_ = _loc7_;
            }
            if(_loc3_ && _loc4_)
            {
               break;
            }
            _loc6_++;
         }
         trace("swapElements",_loc3_,_loc4_,"|",param1,param2);
         if(_loc3_ && _loc4_)
         {
            trace("swapElements",_loc3_.data ? _loc3_.data.name : "null",_loc4_.data ? _loc4_.data.name : "null");
            _loc3_.index = param2;
            if(_loc3_.data)
            {
               _loc3_.data.slotId = param2;
            }
            _loc4_.index = param1;
            if(_loc4_.data)
            {
               _loc4_.data.slotId = param1;
            }
         }
         this.setAmmo();
      }
      
      public function updateElement(param1:Object, param2:ItemData = null) : void
      {
         trace("updateElement::",this._ammoList.length,this._elemetns.length,this._elemetnsSlots.length,this._dataSlots.length,param2 ? param2.count : "no_new_data");
         var _loc3_:ItemData = null;
         var _loc4_:int = 0;
         while(_loc4_ < this._dataSlots.length)
         {
            trace("searchForVO::",param1.dict_id,this._dataSlots[_loc4_].dictId,this._dataSlots[_loc4_].name);
            if(this._dataSlots[_loc4_].dictId == param1.dict_id)
            {
               _loc3_ = this._dataSlots[_loc4_];
               break;
            }
            _loc4_++;
         }
         if(!_loc3_)
         {
            if(param2)
            {
               _loc3_ = param2;
            }
            else
            {
               return;
            }
         }
         else if(param2)
         {
            _loc3_.count += param2.value;
         }
         var _loc5_:int = 0;
         var _loc6_:Object;
         while(_loc5_ < this._elemetnsSlots.length)
         {
            _loc6_ = this._elemetnsSlots[_loc5_];
            if(_loc6_.index == param1.targetSlot)
            {
               trace("update::",_loc3_.slotId,_loc3_.count);
               _loc3_.descriptor = param1.sourceSlot;
               _loc6_.data = _loc3_;
               this.setAmmo();
               break;
            }
            _loc5_++;
         }
      }
      
      protected function acceptItemCard(param1:Event = null) : void
      {
         trace("acceptItemCardTotal::",_itemCard,this._elemetns);
         _relocateArray = new Array();
         trace("ASL::apply_profile_items");
         ExternalInterface.call("apply_profile_items",this._relocateArray);
      }
      
      public function closePopup() : void
      {
         this.prepare_new_data();
      }
      
      public function setProfileId(param1:int) : void
      {
         _profileID = param1;
      }
      
      public function setSlots(param1:int = 0, param2:uint = 0) : void
      {
         _slotsN = param1;
      }
      
      public function setShared(param1:Boolean) : void
      {
         _isShared = param1;
      }
      
      public function getShared() : Boolean
      {
         return this._isShared;
      }
      
      public function reflow(param1:AmmoShareElement) : void
      {
         trace("ammmo_reflow_st",param1,param1 ? param1.index : "no_tg",this._isReflow);
         if(this._isReflow)
         {
            return;
         }
         _isReflow = true;
         var _loc2_:Number = 0;
         var _loc3_:Number = 0;
         var _loc4_:Boolean = true;
         var _loc5_:AmmoShareElement;
         var _loc6_:AmmoShareElement;
         var _loc7_:uint = this._elemetns.length;
         var _loc8_:int = 0;
         var _loc9_:uint;
         while(_loc8_ < _loc7_)
         {
            _loc5_ = this._elemetns[_loc8_];
            if(_loc5_.group != GROUP_ITEMS && (!param1 || _loc5_.pair == param1.pair && _loc5_.index != param1.index))
            {
               _loc2_ = 0;
               _loc9_ = 0;
               while(_loc9_ < _loc7_)
               {
                  _loc6_ = this._elemetns[_loc9_];
                  if(_loc6_.index != _loc5_.index && _loc5_.group == _loc6_.group)
                  {
                     _loc2_ += _loc6_.getCount();
                  }
                  _loc9_++;
               }
               _loc5_.setCount(_loc2_,true);
               if(_loc4_ && _loc5_.getCount() != 0)
               {
                  _loc4_ = false;
               }
            }
            _loc8_++;
         }
         _totalPrice = 0;
         _totalPricePremium = 0;
         for(_loc8_ = 0; _loc8_ < _loc7_; _loc8_++)
         {
            _loc5_ = this._elemetns[_loc8_];
            if(_loc5_.isPremium())
            {
               _totalPricePremium += _loc5_.get_required_price();
            }
            else
            {
               _totalPrice += _loc5_.get_required_price();
            }
            if(_loc5_.group == GROUP_ITEMS)
            {
               continue;
            }
            if(param1 && _loc5_.pair != param1.pair)
            {
               _loc2_ = 0;
               _loc9_ = 0;
               while(_loc9_ < _loc7_)
               {
                  _loc6_ = this._elemetns[_loc9_];
                  if(_loc6_.index != _loc5_.index && _loc5_.group == _loc6_.group)
                  {
                     _loc2_ += _loc6_.getCount();
                  }
                  _loc9_++;
               }
               if(_loc4_)
               {
                  _loc5_.setCount(_loc2_,true);
               }
               else
               {
                  _loc5_.setCount(_loc2_,false);
               }
            }
         }
         if(this._updateFunction is Function)
         {
            this._updateFunction(this._totalPrice,this._totalPricePremium);
         }
         _loc9_ = 0;
         while(_loc9_ < _loc7_)
         {
            _loc5_ = this._elemetns[_loc9_];
            _loc5_.updateStorage();
            _loc9_++;
         }
         _isReflow = false;
         trace("reflow_end");
      }
      
      public function getSlotData(param1:uint) : ItemData
      {
         var _loc2_:int;
         trace("getSlotData::",this._ammoList);
         if(this._ammoList)
         {
            _loc2_ = 0;
            while(_loc2_ < this._ammoList.length)
            {
               if(this._ammoList[_loc2_] && this._ammoList[_loc2_].slot == param1)
               {
                  return this._ammoList[_loc2_];
               }
               _loc2_++;
            }
         }
         return null;
      }
      
      public function getAmmoList(param1:int, param2:Boolean = false, param3:int = -1) : Array
      {
         trace("getAmmoList",param1,this._ammoList,name,param3,param2);
         this._tempAmmoLst.length = 0;
         var _loc4_:int = 0;
         var _loc5_:Boolean;
         var _loc6_:int;
         var _loc7_:AmmoShareElement;
         var _loc8_:ItemData;
         while(_loc4_ < this._ammoList.length)
         {
            _loc5_ = false;
            if(this._ammoList[_loc4_].weaponTarget != param1)
            {
               _loc5_ = true;
            }
            else
            {
               this._ammoList[_loc4_].enabled = true;
               if(param3 == -1 || this._ammoList[_loc4_].slotId != param3)
               {
                  _loc6_ = 0;
                  while(_loc6_ < this._elemetns.length)
                  {
                     _loc7_ = this._elemetns[_loc6_];
                     _loc8_ = _loc7_.getData();
                     if(_loc8_)
                     {
                        if(this._ammoList[_loc4_].dictId == _loc8_.dictId)
                        {
                           if(param2)
                           {
                              _loc5_ = true;
                              break;
                           }
                           else
                           {
                              this._ammoList[_loc4_].enabled = false;
                           }
                        }
                     }
                     _loc6_++;
                  }
               }
            }
            if(!_loc5_)
            {
               this._tempAmmoLst.push(this._ammoList[_loc4_]);
            }
            _loc4_++;
         }
         trace("tempAmmoList:",this._tempAmmoLst);
         return this._tempAmmoLst;
      }
      
      public function setAmmo(param1:ButtonEvent = null) : void
      {
         _relocateArray = new Array();
         var _loc2_:int = 0;
         var _loc3_:AmmoShareElement;
         while(_loc2_ < this._elemetns.length)
         {
            _loc3_ = this._elemetns[_loc2_];
            _loc3_.updated = false;
            this.relocateAmmo(_loc3_);
            _loc2_++;
         }
         _loc2_ = 0;
         while(_loc2_ < this._elemetnsSlots.length)
         {
            this.relocateItem(this._elemetnsSlots[_loc2_].data);
            _loc2_++;
         }
         trace("ASL::apply_profile_items");
         ExternalInterface.call("apply_profile_items",this._relocateArray);
         this.clear();
         if(this._updateFunction is Function)
         {
            this._updateFunction(0,0);
         }
         this._btn_yes.enabled = this._btn_cancel.enabled = false;
      }
      
      public function reflowElements() : void
      {
         activeIndex = 4;
         var _loc1_:int = 144 + 5;
         var _loc2_:int = 275 + 5;
         var _loc3_:int = 0;
         var _loc4_:int = 0;
         var _loc5_:AmmoShareElement;
         trace("reflowElemnts",this.activeIndex);
         for(var _loc6_:uint = 0; _loc6_ < this._elemetns.length; _loc6_++)
         {
            _loc5_ = this._elemetns[_loc6_];
            if(!_loc5_)
            {
               continue;
            }
            if(_loc5_.pair == PAIR_AMMO1 || _loc5_.pair == PAIR_AMMO2)
            {
               if(this.activeIndex == 0 && _loc5_.pair == PAIR_AMMO1 || this.activeIndex == 1 && _loc5_.pair == PAIR_AMMO2 || this.activeIndex == 4)
               {
                  if(!_loc5_.parent)
                  {
                     _loc5_.y = _loc3_;
                     this._items_container.addContent(_loc5_);
                  }
               }
               else if(_loc5_.parent)
               {
                  this._items_container.removeContent(_loc5_);
               }
            }
            else if(this.activeIndex == -1)
            {
               _loc5_.y = _loc3_;
               this._items_container.addContent(_loc5_);
            }
            else
            {
               this._items_container.removeContent(_loc5_);
            }
            if(!_loc5_.parent)
            {
               continue;
            }
            _loc5_.y = _loc3_;
            _loc5_.x = _loc4_;
            _loc3_ += _loc1_;
            if(_loc3_ / _loc1_ == 3)
            {
               _loc3_ = 0;
               _loc4_ += _loc2_;
            }
         }
      }
      
      public function setData(param1:Array, param2:Array, param3:Boolean = false, param4:Boolean = false) : void
      {
         _data = param1;
         _dataSlots = param2;
         _softUpdate = param4;
         _ammoList = this._data;
         trace("invalidateASLData",this._data,this._softUpdate);
         invalidateData();
      }
      
      internal function checkApplyButton() : void
      {
         trace("checkApplyButton",this._totalPrice);
         if(this._totalPrice > PlayerInfo.genericMoney)
         {
            this._btn_yes.enabled = false;
            this._btn_cancel.enabled = false;
         }
         else
         {
            this._btn_yes.enabled = true;
            this._btn_cancel.enabled = true;
         }
      }
      
      internal function getCount(param1:AmmoShareElement) : Number
      {
         var _loc2_:Number = 0;
         var _loc3_:int = 0;
         var _loc4_:AmmoShareElement;
         for(; _loc3_ < this._elemetns.length; _loc3_++)
         {
            _loc4_ = this._elemetns[_loc3_];
            if(!_loc4_)
            {
               continue;
            }
            if(_loc4_.group == GROUP_ITEMS)
            {
               continue;
            }
            if(_loc4_.index != param1.index)
            {
               _loc2_ += _loc4_.getCount();
            }
         }
         return _loc2_;
      }
      
      internal function getCost(param1:AmmoShareElement) : Number
      {
         var _loc2_:Number = 0;
         var _loc3_:int = 0;
         var _loc4_:AmmoShareElement;
         while(_loc3_ < this._elemetns.length)
         {
            _loc4_ = this._elemetns[_loc3_];
            if(_loc4_.index != param1.index)
            {
               _loc2_ += _loc4_.isPremium() ? 0 : _loc4_.get_required_price();
            }
            _loc3_++;
         }
         return _loc2_;
      }
      
      protected function config(param1:Event) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.config);
         addEventListener(Event.CHANGE,this.onChange);
         addEventListener(SliderEvent.VALUE_CHANGE,this.onChange);
         this._items_container.name = "items_container";
         this._items_container.elemntShift = 15;
         addChild(this._items_container);
         this._items_container.width = 575;
         this._items_container.height = 500;
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            if(this._data)
            {
               this._setData(this._data);
            }
         }
      }
      
      protected function updateAutoBuy(param1:Event) : void
      {
         trace("ASl::updateAutoBuy",this._autoBuyCheckBox.selected);
         ExternalInterface.call("ammo_autobuy_changed",this._autoBuyCheckBox.selected);
      }
      
      protected function onChange(param1:Event) : void
      {
         var _loc2_:Number = 0;
         var _loc3_:int = (this._elemetns.length);
         var _loc4_:int = 0;
         var _loc5_:AmmoShareElement;
         var _loc6_:Number;
         var _loc7_:ItemData;
         while(_loc4_ < _loc3_)
         {
            _loc5_ = this._elemetns[_loc4_];
            _loc6_ = 0;
            if((_loc5_.getData()))
            {
               _loc6_ = _loc5_.getWeight();
               _loc2_ += _loc6_;
            }
            _loc4_++;
         }
         _loc3_ = (this._elemetnsSlots.length);
         _loc4_ = 0;
         while(_loc4_ < this._elemetnsSlots.length)
         {
            _loc7_ = this._elemetnsSlots[_loc4_].data;
            if(_loc7_)
            {
               _loc6_ = _loc7_.value * _loc7_.weight;
               _loc2_ += _loc6_;
            }
            _loc4_++;
         }
         trace("updateWeightOnChange::",this.tx_weigth,PlayerInfo.curWeight,this.aviableWeight,_loc2_,PlayerInfo.maxWeight,"calculated",Number(PlayerInfo.maxWeight - this.aviableWeight + _loc2_).toPrecision(2));
         if(this.tx_weigth)
         {
         }
      }
      
      protected function onControlChanges(param1:Event) : void
      {
         this.checkApplyButton();
      }
      
      protected function clear() : void
      {
         dispatchEvent(new Event(Event.CLOSE));
      }
      
      protected function trace_relocate(param1:Array) : void
      {
         trace("trace_relocate",param1.length);
         var _loc2_:int = 0;
         while(_loc2_ < param1.length)
         {
            trace("items is:",param1[_loc2_].profile,param1[_loc2_].id,param1[_loc2_].dict_id,"sr:",param1[_loc2_].sourceSlot,"tg:",param1[_loc2_].targetSlot,param1[_loc2_].count);
            _loc2_++;
         }
      }
      
      protected function prepare_new_data() : void
      {
         var _loc1_:int = 0;
         var _loc2_:AmmoShareElement;
         while(_loc1_ < this._elemetns.length)
         {
            _loc2_ = this._elemetns[_loc1_];
            _loc2_.disable();
            _loc1_++;
         }
         this.clear();
      }
      
      protected function cancel(param1:ButtonEvent = null) : void
      {
         this.prepare_new_data();
         this._setData(this._ammoList);
         this._btn_cancel.enabled = this._btn_yes.enabled = false;
      }
      
      protected function clearElements() : void
      {
         var _loc1_:int = 0;
         var _loc2_:AmmoShareElement;
         while(_loc1_ < this._elemetns.length)
         {
            _loc2_ = this._elemetns[_loc1_];
            _loc2_.disable();
            this._items_container.removeContent(_loc2_);
            _loc1_++;
         }
         this._elemetns.length = 0;
      }
      
      protected function relocateItem(param1:ItemData) : void
      {
         var _loc2_:ItemData = param1;
         var _loc3_:uint;
         var _loc4_:uint;
         var _loc5_:uint;
         var _loc6_:uint;
         var _loc7_:uint;
         var _loc8_:uint;
         var _loc9_:uint;
         if(_loc2_)
         {
            _loc3_ = _loc2_.itemId == -1 ? 0 : _loc2_.itemId;
            _loc4_ = this._profileID;
            _loc5_ = _loc2_.dictId;
            _loc6_ = _loc2_.slotId;
            _loc7_ = _loc2_.descriptor != 0 ? _loc2_.descriptor : 100;
            _loc8_ = _loc2_.count;
            _loc9_ = _loc2_.value;
            trace("prepare relocate item:",param1.name,"target:",_loc6_,"count:",_loc9_,_loc8_,"sr:",_loc7_,"name",_loc2_.name,_loc3_,_loc2_.itemId,"ds:",_loc2_.descriptor);
            if(_loc8_)
            {
               this._relocateArray.push({
                  profile:_loc4_,
                  id:_loc3_,
                  dict_id:_loc5_,
                  targetSlot:_loc6_,
                  count:_loc9_,
                  sourceSlot:_loc7_,
                  count_in_inventory:_loc8_,
                  auto_buy:this.autoBuyCheckBox.selected
               });
            }
         }
      }
      
      protected function relocateAmmo(param1:AmmoShareElement) : void
      {
         var _loc2_:Object = param1.getData();
         var _loc3_:uint;
         var _loc4_:uint;
         var _loc5_:uint;
         var _loc6_:uint;
         var _loc7_:uint;
         var _loc8_:uint;
         var _loc9_:uint;
         if(_loc2_)
         {
            _loc3_ = _loc2_.itemId == -1 ? 0 : (_loc2_.itemId);
            _loc4_ = this._profileID;
            _loc5_ = (_loc2_.dictId);
            _loc6_ = param1.index;
            _loc7_ = _loc2_.source_slot != undefined ? (_loc2_.source_slot) : 100;
            _loc8_ = param1.slider.position;
            _loc9_ = (param1.slider.value_storage);
            trace("prepare relocate:",param1.name,"target:",_loc6_,"source:",_loc7_,"count:",_loc8_,_loc9_,"name",_loc2_.name,param1.pair,_loc3_,_loc2_.itemId,this.autoBuyCheckBox.selected);
            if(_loc9_)
            {
               this._relocateArray.push({
                  profile:_loc4_,
                  id:_loc3_,
                  dict_id:_loc5_,
                  targetSlot:_loc6_,
                  count:_loc8_,
                  sourceSlot:_loc7_,
                  count_in_inventory:_loc9_,
                  auto_buy:this.autoBuyCheckBox.selected
               });
            }
         }
      }
      
      protected function clearData(param1:Event) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.clearData);
         this.prepare_new_data();
      }
      
      protected function updateData(param1:Event) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.updateData);
         this._setData(this._data);
      }
      
      protected function _setData(value:Array) : void
      {
         trace("AmmoShareSetData:",this.activeIndex,this._softUpdate,"|",value,"|",this._elemetns,"|",this._items_container);
         _ammoList = value;
         if(value.length)
         {
            value.sortOn(["weapon_target","slot"],Array.NUMERIC);
         }
         var _ammoSlots:uint = 6;
         var _shift:uint = 0;
         var _element:AmmoShareElement;
         var current_index:uint = 0;
         var st_index:uint = 0;
         var _slotToIterate:uint = _ammoSlots;
         var ln:int;
         var ln2:int;
         var ammoL:int;
         var slotsL:int;
         var itemsL:int;
         trace("updateA");
         ln = (this._elemetns.length);
         i = 0;
         while(i < ln)
         {
            _element = this._elemetns[i];
            _element.index = this._unsetted_slot;
            _element.slider.removeEventListener(SliderEvent.VALUE_CHANGE,this.onChange);
            _element.removeEventListener(Event.CHANGE,this.onControlChanges);
            if(_element.parent)
            {
               this._items_container.removeContent(_element);
            }
            i++;
         }
         trace("updateB");
         var _start_elements:Array = this._elemetns.concat();
         this._elemetns.length = 0;
         var ammo1_slots:int = 0;
         var ammo2_slots:int = 0;
         var i:int;
         ln = (value.length);
         i = 0;
         while(i < ln)
         {
            itemData = value[i] as ItemData;
            if(itemData.weaponTarget != PAIR_ITEMS)
            {
               _element = null;
               ln2 = (_start_elements.length);
               for(k = 0; k < ln2; k++)
               {
                  if(!_start_elements[k])
                  {
                     continue;
                  }
                  elData = _start_elements[k].getData();
                  if(elData && elData.dictId == itemData.dictId)
                  {
                     _element = _start_elements[k];
                     trace("use_old_element",_element.index,_element.updated);
                     _start_elements.splice(k,1);
                  }
               }
               if(!_element)
               {
                  trace("create_new_element",itemData.slotId);
                  _element = new this._renderer();
                  _element.owner = this;
               }
               _element.index = itemData.slotId;
               this._elemetns[current_index] = _element;
               current_index += 1;
               _element.ignoreChange();
               _element.slider.renderer = "ItemRoundCell";
               _element.available = this._ammoBags;
               _element.group = GROUP_AMMO;
               st_index += 1;
               _element.pair = itemData.weaponTarget;
               if(_element.pair == PAIR_AMMO1)
               {
                  ammo1_slots++;
               }
               else
               {
                  ammo2_slots++;
               }
               if(this.activeIndex == 0 || this.activeIndex == 1 || this.activeIndex == 4)
               {
                  if(!_element.parent)
                  {
                     this._items_container.addContent(_element);
                  }
               }
               else if(_element.parent)
               {
                  this._items_container.removeContent(_element);
               }
            }
            i++;
         }
         ln = (this._elemetns.length);
         i = 0;
         while(i < ln)
         {
            _element = this._elemetns[i];
            if(_element.pair != PAIR_ITEMS && _element.index == this._unsetted_slot)
            {
               function getIndexOwned(param1:int):Boolean
               {
                  var _loc2_:int = 0;
                  var _loc3_:*;
                  while(_loc2_ < ln)
                  {
                     _loc3_ = _elemetns[_loc2_];
                     if(_loc3_.pair != PAIR_ITEMS && param1 == _loc3_.index)
                     {
                        return true;
                     }
                     _loc2_++;
                  }
                  return false;
               }
               var _elementSlot:Object;
               var pos_of_last_ammo:uint;
               var j:int;
               var list:Array;
               index_iterator = 0;
               new_index = (this._ammo_indexes[0]);
               while(getIndexOwned(new_index))
               {
                  index_iterator++;
                  new_index = (this._ammo_indexes[index_iterator]);
               }
               _element.index = new_index;
            }
            i++;
         }
         ln = this._elemetns.length - 1;
         trace("removeRestedAmmoSlots");
         i = ln;
         while(i > st_index)
         {
            _element = this._elemetns[i];
            if(_element.pair != PAIR_ITEMS)
            {
               if(_element.parent)
               {
                  this._items_container.removeContent(_element);
                  _element.clear();
               }
               this._elemetns.splice(i,1);
            }
            i--;
         }
         ln = (_start_elements.length);
         i = 0;
         while(i < ln)
         {
            _element = _start_elements[i];
            if(_element && _element.pair != PAIR_ITEMS)
            {
               if(_element.parent)
               {
                  this._items_container.removeContent(_element);
               }
               _start_elements.splice(i,1);
               i--;
            }
            i++;
         }
         trace("addIgnorentAmmoSlots",_slotToIterate);
         i = 0;
         while(i < _slotToIterate)
         {
            if(!this._elemetns[i])
            {
               _element = this.getEmptyElement();
               _element.group = GROUP_AMMO;
               _element.index = 999;
               if(ammo1_slots < 3)
               {
                  _element.pair = PAIR_AMMO1;
                  ammo1_slots++;
               }
               else
               {
                  _element.pair = PAIR_AMMO2;
                  ammo2_slots++;
               }
               if(!_element.parent)
               {
                  this._items_container.addContent(_element);
               }
               this._elemetns[current_index] = _element;
               current_index += 1;
               st_index += 1;
            }
            i++;
         }
         i = 0;
         while(i < this._slotsN)
         {
            _elementSlot = null;
            ln = (this._elemetnsSlots.length);
            k = 0;
            while(k < ln)
            {
               if(this._elemetnsSlots[k].index == i + 13)
               {
                  _elementSlot = this._elemetnsSlots[k];
                  break;
               }
               k++;
            }
            if(!_elementSlot)
            {
               _elementSlot = new Object();
               this._elemetnsSlots.push(_elementSlot);
            }
            _elementSlot.index = i + 13;
            i++;
         }
         this._elemetnsSlots.length = this._slotsN;
         ln = (this._elemetns.length);
         st_index += i;
         i = ln;
         while(i > st_index)
         {
            _element = this._elemetns.pop();
            _element.removeEventListener(Event.CHANGE,this.onControlChanges);
            _element.slider.removeEventListener(SliderEvent.VALUE_CHANGE,this.onChange);
            if(_element.parent)
            {
               this._items_container.removeContent(_element);
            }
            i--;
         }
         _start_elements.length = 0;
         k = 0;
         while(i < ln)
         {
            if(_element.getData() == null)
            {
               (this._elemetns[k] as AmmoShareElement).setData(this._elemetns[k]);
            }
            else
            {
               _element.disable();
            }
            k++;
         }
         if(this._ammoBags == this._slotsN)
         {
            pos_of_last_ammo = (this._elemetns.indexOf(_element));
         }
         this._elemetns.sortOn(["pair","index"],Array.NUMERIC);
         ln2 = (value.length);
         i = 0;
         while(i < ln2)
         {
            itemData = value[i];
            j = 0;
            while(j < ln)
            {
               _element = this._elemetns[j];
               if(_element && (itemData.slotId != 0 && itemData.slotId == _element.index || (itemData.slotId == 0 && _element.getData() && itemData.dictId == _element.getData().dictId)))
               {
                  if((this._softUpdate || _element.updated) && _element.getData() && itemData.dictId == _element.getData().dictId)
                  {
                     _element.name = value[i].name;
                     _element.update(value[i],!this._softUpdate);
                     break;
                  }
                  else
                  {
                     _element.name = value[i].name;
                     _element.setData(value[i],true);
                     break;
                  }
               }
               j++;
            }
            i++;
         }
         i = 0;
         while(i < this._slotsN)
         {
            _elementSlot = this._elemetnsSlots[i];
            _elementSlot.data = null;
            i++;
         }
         itemsL = (this._dataSlots.length);
         trace("fill_dataSlots",itemsL,"|",this._slotsN);
         i = 0;
         while(i < itemsL)
         {
            itemData = this._dataSlots[i];
            trace("itemData",itemData.name,itemData.slotId);
            j = 0;
            while(j < this._slotsN)
            {
               _elementSlot = this._elemetnsSlots[j];
               trace("_elementSlot",_elementSlot.index,itemData.slotId);
               if(_elementSlot.index == itemData.slotId)
               {
                  _elementSlot.data = itemData;
                  break;
               }
               j++;
            }
            i++;
         }
         i = 0;
         while(i < ln)
         {
            _element = this._elemetns[i];
            if(!_element.getData() && _element.pair != PAIR_ITEMS && _element.index != 999)
            {
               list = this.getAmmoList(_element.pair,true);
               if(list.length && list[0])
               {
                  _element.name = "[]" + list[0].name;
                  _element.setData(list[0],true);
               }
            }
            i++;
         }
         i = 0;
         while(i < ln)
         {
            _element = this._elemetns[i];
            if(!_element.getData())
            {
               trace("clean_element::",_element.name);
               _element.disable();
            }
            i++;
         }
         this.reflow(null);
         this.onChange(null);
         i = 0;
         while(i < ln)
         {
            _element = this._elemetns[i];
            _element.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.onChange,false,0,true);
            _element.addEventListener(Event.CHANGE,this.onControlChanges,false,0,true);
            i++;
         }
         this._elemetns.sortOn(["pair","itemLevel","index"],Array.NUMERIC);
         var step_y:int = 144 + 5;
         var step_x:int = 275 + 5;
         var shift_y:int = 0;
         var shift_x:int = 0;
         var itemData:ItemData;
         var k:int;
         var elData:ItemData;
         var index_iterator:int;
         var new_index:uint;
         for(i = 0; i < ln; i++)
         {
            _element = this._elemetns[i];
            if(!_element || !_element.parent)
            {
               continue;
            }
            _element.x = shift_x;
            _element.y = shift_y;
            shift_y += step_y;
            if(shift_y / step_y == 3)
            {
               shift_y = 0;
               shift_x += step_x;
            }
         }
         trace("ASL::updated");
      }
   }
}

