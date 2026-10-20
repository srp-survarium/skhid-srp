package survarium.controls
{
   import flash.events.Event;
   import flash.external.*;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import survarium.Util;
   import survarium.data.ItemData;
   import survarium.interfaces.IItemFactory;
   import survarium.interfaces.IPlayerProfile;
   
   public class PlayerProfileBase extends UIComponent implements IPlayerProfile
   {
      
      public var profileItems:*;
      
      protected var _inventory:IItemFactory;
      
      protected var _ammoBags:int;
      
      protected var _pocketSize:int;
      
      protected var _profileId:int;
      
      protected var _profiles:Array;
      
      public function PlayerProfileBase()
      {
         super();
         addEventListener(Event.RENDER,this.init);
      }
      
      public function get profileId() : int
      {
         return this._profileId;
      }
      
      public function registerInventory(param1:IItemFactory) : void
      {
         _inventory = param1;
      }
      
      public function updateAmmoBags(param1:uint) : void
      {
         _ammoBags = param1;
      }
      
      public function getAmmoBags() : uint
      {
         return this._ammoBags;
      }
      
      public function updatePocketSize(param1:uint) : void
      {
         _pocketSize = param1;
      }
      
      public function getPocketSize() : uint
      {
         return this._pocketSize;
      }
      
      public function updateSlots(param1:uint, param2:uint) : void
      {
         this.profileItems.openSlots(param2,param1);
      }
      
      public function setupProfiles(param1:Array) : void
      {
         _profiles = param1;
      }
      
      protected function getItemData(param1:Object) : ItemData
      {
         return this._inventory.createItemData(param1);
      }
      
      protected function handleUnresolvedItem(param1:Object) : void
      {
      }
      
      public function fillProfileItems(param1:Object) : void
      {
         trace("PPB:fillProfileItems",param1.items.length,this.profileItems,this._inventory);
         _profileId = param1.profileId;
         var _loc2_:Array = param1.items;
         var _loc3_:Array = new Array();
         var _loc4_:uint = _loc2_.length;
         var _loc5_:uint = 0;
         var _loc6_:Object;
         var _loc7_:ItemData;
         var _loc8_:ItemData;
         while(_loc5_ < _loc4_)
         {
            _loc6_ = _loc2_[_loc5_];
            Util.logObject(_loc6_);
            _loc7_ = this.getItemData(_loc6_);
            if(!_loc7_)
            {
               this.handleUnresolvedItem(_loc6_);
            }
            else
            {
               _loc8_ = this.createItem(_loc6_,_loc7_);
               _loc3_.push(_loc8_);
               trace("PPB:PlayerProfileAddItem::",_loc7_.name,"count:",_loc8_.value,_loc8_.cost,_loc2_[_loc5_].itemId,_loc2_[_loc5_].slotId,_loc2_[_loc5_].condition_or_stack,"icon::",_loc7_.iconId);
            }
            _loc5_++;
         }
         if(param1.item_slots != undefined)
         {
            this.profileItems.openSlots(param1.item_slots,param1.artifact_slots);
         }
         this.profileItems.dataProvider = new DataProvider(_loc3_);
         trace("PPB:invalidateItems");
      }
      
      protected function createItem(param1:Object, param2:ItemData) : ItemData
      {
         trace("PPB:createItem::",param1,param2);
         var _loc3_:ItemData = param2.clone();
         if(param1.enabled != undefined)
         {
            _loc3_.enabled = param1.enabled;
         }
         else
         {
            _loc3_.enabled = true;
         }
         _loc3_.value = _loc3_.isStack ? param1.condition_or_stack : param2.value;
         _loc3_.slotId = param1.slotId;
         return _loc3_;
      }
      
      protected function init(param1:Event) : void
      {
         removeEventListener(Event.RENDER,this.init);
      }
   }
}

