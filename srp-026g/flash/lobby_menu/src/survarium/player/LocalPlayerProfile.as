package survarium.player
{
   import fl.transitions.easing.Strong;
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.*;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.controls.Button;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.motion.Tween;
   import survarium.Util;
   import survarium.controls.PlayerProfileBase;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IAmmoShare;
   import survarium.interfaces.IInventoryItemFactory;
   import survarium.interfaces.IItemFactory;
   import survarium.interfaces.ILocalPaperDoll;
   import survarium.interfaces.ILocalPlayerProfile;
   
   public class LocalPlayerProfile extends PlayerProfileBase implements ILocalPlayerProfile
   {
      
      public var labelText:TextField;
      
      public var btnShowAmmo:Button;
      
      public var playerWeight:MovieClip;
      
      public var glassSprite:MovieClip;
      
      public var checkbox_autobuy:Button;
      
      public var checkbox_autorepair:Button;
      
      public var ammoContainer:MovieClip;
      
      public var buyPremium:MovieClip;
      
      private var _weightC:Number = 0;
      
      private var _weightM:Number = 0;
      
      private var _relocateFunctionRef:Function;
      
      private var _ammo_share:IAmmoShare;
      
      private var _tween:Tween;
      
      private var _playerPaperDoll:ILocalPaperDoll;
      
      private var _playerInventory:IInventoryItemFactory;
      
      protected var _unresolvedItems:Array = new Array();
      
      protected var _convertedItems:Array;
      
      public function LocalPlayerProfile()
      {
         super();
         var rendererPremium:Class = getDefinitionByName("ProfileBuyPremium") as Class;
         buyPremium = new rendererPremium();
         _playerPaperDoll = profileItems as ILocalPaperDoll;
         addChild(this.buyPremium);
         this.buyPremium.visible = false;
         this.buyPremium.x = 17;
         this.buyPremium.y = 113;
         var rendererBack:Class = getDefinitionByName("BackBlack") as Class;
         var bb:MovieClip = new rendererBack();
         bb.y = -10;
         bb.x = -10;
         this.buyPremium.addChildAt(bb,0);
         bb.height = this.height - this.buyPremium.y - bb.y - 4;
         bb.width = this.width - this.buyPremium.x - bb.x - 4;
      }
      
      override public function get height() : Number
      {
         return 902;
      }
      
      override public function get width() : Number
      {
         return 580;
      }
      
      public function set _relocateFunction(value:Function) : void
      {
         trace("LocalPlayerProfile::set",value);
         _relocateFunctionRef = value;
      }
      
      public function get _relocateFunction() : Function
      {
         trace("LocalPlayerProfile::get",this._relocateFunctionRef);
         return this._relocateFunctionRef;
      }
      
      public function get paperDoll() : ILocalPaperDoll
      {
         return this._playerPaperDoll;
      }
      
      public function get btnSetAmmo() : Button
      {
         return this.ammoContainer.btn_apply;
      }
      
      public function setAmmoShare(value:IAmmoShare) : void
      {
         _ammo_share = value;
         this._ammo_share.registerWeightTextField(this.playerWeight.textFieldValue);
      }
      
      public function lockTutorial(value:Boolean) : void
      {
      }
      
      public function getRelativeData(value:Object) : Object
      {
         if(value.superCategory == 3)
         {
            return null;
         }
         var equippedItems:DataProvider = profileItems.dataProvider as DataProvider;
         if(equippedItems)
         {
            var l:uint = equippedItems.length;
            for(var j:uint = 0; j < l; j++)
            {
               var itemData:ItemData = equippedItems[j];
               if(itemData.category == value.category || itemData.isWeapon && value.isWeapon && itemData.oneHanded == value.oneHanded)
               {
                  if(itemData.itemId == value.itemId)
                  {
                     return null;
                  }
                  return itemData;
               }
            }
         }
         return null;
      }
      
      override public function set enabled(value:Boolean) : void
      {
         profileItems.enabled = value;
         this.btnShowAmmo.enabled = value;
         if(this.btnShowAmmo.selected && value == false)
         {
            this.onAmmoButton();
         }
      }
      
      override public function set x(value:Number) : void
      {
         super.x = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(value:Number) : void
      {
         super.y = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      public function get_ammo_share() : IAmmoShare
      {
         return this._ammo_share;
      }
      
      override public function registerInventory(link:IItemFactory) : void
      {
         _playerInventory = link as IInventoryItemFactory;
         _inventory = link;
         this._playerPaperDoll.registerInventory(this._playerInventory);
      }
      
      public function registerRelocateFunction(link:Function) : void
      {
         trace("PP:registerRelocateFunction");
         _relocateFunction = link;
      }
      
      public function updateWeight(cur:Number, max:Number) : void
      {
         _weightC = cur;
         _weightM = max;
         this.playerWeight.textFieldValue.styleSheet = MainScene.textStyleSheet;
         var style:String = this._weightC < this._weightM ? "highlight" : "red";
         this.playerWeight.textFieldValue.text = Util.styledChar(cur.toFixed(2),style) + " / " + Util.getNumberString(max);
         this.playerWeight.progress_indicator.maximum = this._weightM;
         this.playerWeight.progress_indicator.value = this._weightC;
         this._playerPaperDoll.ammoControl.update(null,"player_profile");
      }
      
      public function enableSecondWeapon(value:Boolean) : void
      {
         this._playerPaperDoll.enableSecondWeapon(value);
      }
      
      public function allowSecondaryTwoHandedWeapon(value:Boolean) : void
      {
         this._playerPaperDoll.allowSecondaryTwoHandedWeapon(value);
      }
      
      public function lock(state:Boolean, arg:uint) : void
      {
         if(state)
         {
            this._playerPaperDoll.lockSlot(state,arg);
            profileItems.enabled = false;
         }
         else
         {
            this._playerPaperDoll.lockSlot(state,0);
            profileItems.enabled = true;
         }
         this._ammo_share.enabled = !state;
         this.btnShowAmmo.enabled = !state;
      }
      
      public function onDrag(e:DragEvent) : void
      {
         if(e == null || e.dragData == null)
         {
            return;
         }
         if(e.type == "dragStart")
         {
            this._playerPaperDoll.selectAgeements(e.dragData.category,e.dragData.dictId,e.dragData.oneHanded,false,e.dragData.itemId);
         }
         else
         {
            this._playerPaperDoll.deselectAgeements();
         }
      }
      
      public function fillAutoBuyItems(value:Array) : void
      {
         this._playerPaperDoll.ammoControl.fillItems(value);
      }
      
      public function fillEquippedItems() : void
      {
         var equippedItems:DataProvider = profileItems.dataProvider as DataProvider;
         trace("fillEquippedItems",equippedItems);
         if(equippedItems)
         {
            var l:uint = equippedItems.length;
            for(var j:uint = 0; j < l; j++)
            {
               var itemData:ItemData = equippedItems[j];
               var item:ItemData = this._playerInventory.getInventoryItem(itemData.itemId);
               if(!item)
               {
                  trace("failedToCreateItem");
               }
               else
               {
                  trace("fillItem");
                  itemData.name = itemData.name;
                  var new_value:int = item.isStack ? (itemData.value) : (item.value);
                  var itemUpdated:Boolean = false;
                  if(itemData.value != new_value)
                  {
                     itemData.value = new_value;
                     itemUpdated = true;
                  }
                  if(itemData.modifiers != item.modifiers)
                  {
                     itemData.modifiers = item.modifiers;
                     itemData.modifiersDescription = item.modifiersDescription;
                     itemUpdated = true;
                  }
                  if(itemData.rank != item.rank)
                  {
                     itemData.rank = item.rank;
                     itemUpdated = true;
                  }
                  if(itemData.broken != item.broken)
                  {
                     itemData.broken = item.broken;
                     itemUpdated = true;
                  }
                  if(itemUpdated)
                  {
                     this._playerPaperDoll.onItemUpdated(itemData.slotId);
                  }
                  if(item && !item.isStack)
                  {
                     item.equipped = true;
                  }
               }
            }
            trace("invalidateOnEquipped");
            equippedItems.invalidate();
         }
      }
      
      public function refillItem(value:uint) : void
      {
         trace("pp:refillItem");
         var currentItems:DataProvider = profileItems.dataProvider as DataProvider;
         var l:uint = currentItems.length;
         for(var i:uint = 0; i < l; i++)
         {
            var item:ItemData = currentItems[i];
            if(item && item.itemId == value)
            {
               var currentItemDescr:ItemData = this._playerInventory.getInventoryItem(value);
               var newItem:ItemData = currentItemDescr.clone();
               newItem.value = newItem.isStack ? item.value : currentItemDescr.value;
               newItem.slotId = item.slotId;
               currentItems[i] = newItem;
               currentItems.invalidate();
               this._playerPaperDoll.onItemUpdated(item.slotId);
               currentItemDescr.equipped = true;
               return;
            }
         }
      }
      
      public function setPlayerSpeed(value:uint) : void
      {
      }
      
      public function handleUnresolvedItems() : void
      {
         var l:uint = this._unresolvedItems.length;
         if(!l)
         {
            return;
         }
         for(var i:uint = 0; i < l; i++)
         {
            var profileItemData:Object = this._unresolvedItems[i];
            var currentItemDescr:ItemData = this._playerInventory.getInventoryItem(profileItemData.itemId);
            if(currentItemDescr)
            {
               var item:ItemData = this.createItem(profileItemData,currentItemDescr);
               this._convertedItems.push(item);
               profileItems.dataProvider = new DataProvider(this._convertedItems);
            }
         }
         for(var j:uint = 0; j < this._convertedItems.length; j++)
         {
            item = this._playerInventory.getInventoryItem(this._convertedItems[j].itemId);
            if(item && !item.isStack)
            {
               item.equipped = true;
            }
         }
      }
      
      override protected function handleUnresolvedItem(value:Object) : void
      {
         this._unresolvedItems.push(value);
      }
      
      override protected function getItemData(value:Object) : ItemData
      {
         return this._playerInventory.getInventoryItem(value.itemId);
      }
      
      override public function fillProfileItems(data:Object) : void
      {
         trace("fillProfileItems",data.items.length);
         var oldData:DataProvider = profileItems.dataProvider as DataProvider;
         if(oldData)
         {
            for(var j:uint = 0; j < oldData.length; j++)
            {
               var item:ItemData = this._playerInventory.getInventoryItem(oldData[j].itemId);
               if(item && !item.isStack)
               {
                  item.equipped = false;
               }
            }
         }
         this._unresolvedItems.length = 0;
         super.fillProfileItems(data);
         trace("PP:invalidateItems");
         this._playerInventory.invalidateList();
      }
      
      override protected function createItem(profileItem:Object, inventoryData:ItemData) : ItemData
      {
         if(!inventoryData.isStack)
         {
            inventoryData.equipped = true;
         }
         var itemData:ItemData = super.createItem(profileItem,inventoryData);
         itemData.descriptor = itemData.slotId;
         return itemData;
      }
      
      public function swapQuickSlots(value_1:int, value_2:int) : void
      {
         this._ammo_share.swapElements(value_1,value_2);
      }
      
      protected function onLocal(value:String = "") : void
      {
         this.playerWeight.textFieldLabel.text = LocalizationData.getName("equipment_weight");
         this.ammoContainer.tx_total_label.text = LocalizationData.getName("equipment_cost");
         this.btnShowAmmo.label = LocalizationData.getName("show_ammo");
         this.labelText.text = LocalizationData.getName("paperdoll_label");
         this.checkbox_autobuy.tipData = {name:LocalizationData.getName("auto_button_hint")};
         this.checkbox_autorepair.tipData = {name:LocalizationData.getName("auto_repair_hint")};
      }
      
      override protected function init(e:Event) : void
      {
         super.init(e);
         this.btnShowAmmo.toggle = true;
         trace("PlayerProfileInit");
         LocalizationData.addCallback(this.onLocal,"equipment_weight");
         LocalizationData.addButton(this.ammoContainer.btn_cancel,"cancel");
         LocalizationData.addButton(this.ammoContainer.btn_apply,"refill_ammo_btn");
         LocalizationData.addButton(this.checkbox_autobuy,"autobuy_button_label");
         LocalizationData.addButton(this.checkbox_autorepair,"auto_repair_button_label");
         this.checkbox_autobuy.tooltipRendererName = "SrvProfileTipRendererStyled";
         this.checkbox_autorepair.tooltipRendererName = "SrvProfileTipRendererStyled";
         this.ammoContainer.labelCostP.moneyType = 1;
         this.checkbox_autorepair.visible = true;
         this.checkbox_autorepair.enabled = true;
         this.checkbox_autorepair.addEventListener(ButtonEvent.CLICK,this.onAutoRepair);
         this.checkbox_autobuy.addEventListener(ButtonEvent.CLICK,this.onAutoRepair);
         trace("PlayerProfileInit2",this._ammo_share,this.ammoContainer,this.ammoContainer.labelCostR,this.ammoContainer.labelCostP,this.btnShowAmmo);
         this._ammo_share.autoBuyCheckBox = this.checkbox_autobuy;
         this.ammoContainer.addChildAt(this._ammo_share as DisplayObject,4);
         this._ammo_share.x = 10;
         this._ammo_share.y = 10;
         this._ammo_share.acceptButton = this.ammoContainer.btn_apply;
         this._ammo_share.cancelButton = this.ammoContainer.btn_cancel;
         this.ammoContainer.labelCostR.moveIcon = false;
         this.ammoContainer.labelCostP.moveIcon = false;
         this._ammo_share.updateFunction = this.onTotalUpdate;
         this.btnShowAmmo.addEventListener(ButtonEvent.CLICK,this.onAmmoButton);
         this.ammoContainer.shift = -575;
         this.ammoContainer.addEventListener(Event.CLOSE,this.onAmmoButtonSwitch);
      }
      
      private function onAutoRepair(e:Event) : void
      {
         trace("PP::updateAutoRepair",this.checkbox_autorepair.selected,this.getAutoBuyState());
         ExternalInterface.call("autobuy_changed",this.getAutoBuyState());
      }
      
      private function getAutoBuyState() : uint
      {
         if(this.checkbox_autorepair.selected == false && this.checkbox_autobuy.selected == false)
         {
            return 0;
         }
         if(this.checkbox_autorepair.selected == true && this.checkbox_autobuy.selected == true)
         {
            return 3;
         }
         if(this.checkbox_autorepair.selected == true)
         {
            return 2;
         }
         return 1;
      }
      
      public function setAutoRepair(value:Boolean) : void
      {
         this.checkbox_autorepair.selected = value;
      }
      
      private function onAmmoButtonSwitch(e:Event) : void
      {
         if(this.btnShowAmmo.selected == true)
         {
            this.btnShowAmmo.selected = false;
            this.onAmmoButton();
         }
      }
      
      private function onAmmoButton(e:Event = null) : void
      {
         trace("onAmmoButton",this.btnShowAmmo.selected,this.ammoContainer);
         if(this.btnShowAmmo.selected)
         {
            ExternalInterface.call("sound_play",10);
            this.btnShowAmmo.label = LocalizationData.getName("hide_ammo");
            if(this._tween)
            {
               this._tween.reset();
               this._tween.paused = true;
            }
            stage.focus = this.ammoContainer;
            trace("ammo_container_showed");
            ExternalInterface.call("ammo_container_showed");
            _tween = new Tween(250,this.ammoContainer,{shift:0},{ease:Strong.easeOut});
         }
         else
         {
            ExternalInterface.call("sound_play",9);
            this.btnShowAmmo.label = LocalizationData.getName("show_ammo");
            if(this._tween)
            {
               this._tween.reset();
               this._tween.paused = true;
            }
            stage.focus = this;
            if(this.ammoContainer.btn_apply.enabled)
            {
               this._ammo_share.setAmmo();
            }
            _tween = new Tween(250,this.ammoContainer,{shift:-575},{ease:Strong.easeOut});
         }
      }
      
      private function onTotalUpdate(eRegular:Number, ePremium:Number) : void
      {
         trace("onTotalUpdate",eRegular,ePremium);
         this.ammoContainer.labelCostR.cost = eRegular;
         this.ammoContainer.labelCostP.visible = ePremium != 0;
         this.ammoContainer.plusSprite.visible = this.ammoContainer.labelCostP.visible;
         this.ammoContainer.labelCostP.cost = ePremium;
      }
      
      private function changeIndexByFilter(e:IndexEvent) : void
      {
         if(e.index == 0)
         {
            this._playerPaperDoll.ammoControl.reflow(null,"playerProfile",4);
         }
         else
         {
            this._playerPaperDoll.ammoControl.reflow(null,"playerProfile",-1);
         }
      }
   }
}

