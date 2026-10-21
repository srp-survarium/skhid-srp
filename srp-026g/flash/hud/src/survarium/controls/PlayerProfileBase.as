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
      
      public function registerInventory(inventory:IItemFactory) : void
      {
         _inventory = inventory;
      }
      
      public function updateAmmoBags(ammoBags:uint) : void
      {
         _ammoBags = ammoBags;
      }
      
      public function getAmmoBags() : uint
      {
         return this._ammoBags;
      }
      
      public function updatePocketSize(pocketSize:uint) : void
      {
         _pocketSize = pocketSize;
      }
      
      public function getPocketSize() : uint
      {
         return this._pocketSize;
      }
      
      public function updateSlots(artifactSlots:uint, itemSlots:uint) : void
      {
         this.profileItems.openSlots(itemSlots,artifactSlots);
      }
      
      public function setupProfiles(profiles:Array) : void
      {
         _profiles = profiles;
      }
      
      protected function getItemData(item:Object) : ItemData
      {
         return this._inventory.createItemData(item);
      }
      
      protected function handleUnresolvedItem(item:Object) : void
      {
      }
      
      public function fillProfileItems(profile:Object) : void
      {
         trace("PPB:fillProfileItems",profile.items.length,this.profileItems,this._inventory);
         _profileId = profile.profileId;
         var items:Array = profile.items;
         var profileItemList:Array = new Array();
         var count:uint = items.length;
         var i:uint = 0;
         var item:Object;
         var baseData:ItemData;
         var itemData:ItemData;
         while(i < count)
         {
            item = items[i];
            Util.logObject(item);
            baseData = this.getItemData(item);
            if(!baseData)
            {
               this.handleUnresolvedItem(item);
            }
            else
            {
               itemData = this.createItem(item,baseData);
               profileItemList.push(itemData);
               trace("PPB:PlayerProfileAddItem::",baseData.name,"count:",itemData.value,itemData.cost,items[i].itemId,items[i].slotId,items[i].condition_or_stack,"icon::",baseData.iconId);
            }
            i++;
         }
         if(profile.item_slots != undefined)
         {
            this.profileItems.openSlots(profile.item_slots,profile.artifact_slots);
         }
         this.profileItems.dataProvider = new DataProvider(profileItemList);
         trace("PPB:invalidateItems");
      }
      
      protected function createItem(item:Object, baseData:ItemData) : ItemData
      {
         trace("PPB:createItem::",item,baseData);
         var itemData:ItemData = baseData.clone();
         if(item.enabled != undefined)
         {
            itemData.enabled = item.enabled;
         }
         else
         {
            itemData.enabled = true;
         }
         itemData.value = itemData.isStack ? item.condition_or_stack : baseData.value;
         itemData.slotId = item.slotId;
         return itemData;
      }
      
      protected function init(event:Event) : void
      {
         removeEventListener(Event.RENDER,this.init);
      }
   }
}

