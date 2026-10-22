package survarium.interfaces
{
   import survarium.data.ItemData;
   
   public interface ILocalPaperDoll extends IPaperDoll
   {
      
      function set ammoControl(value:IAmmoControll) : void;
      
      function get ammoControl() : IAmmoControll;
      
      function get selectedIndex() : int;
      
      function setSlotsRestrictions(restrictions:Array) : void;
      
      function registerInventory(inventory:IInventoryItemFactory) : void;
      
      function enableSecondWeapon(value:Boolean) : void;
      
      function allowSecondaryTwoHandedWeapon(value:Boolean) : void;
      
      function lockSlot(locked:Boolean, slotIndex:uint) : void;
      
      function getSlot(item:ItemData, preferEmpty:Boolean = false) : int;
      
      function hoverItem(itemId:uint = 0) : void;
      
      function hoverWeaponsFor(scopeDictId:int = -1) : void;
      
      function clearSelection() : void;
      
      function hasAgreements(category:uint) : uint;
      
      function selectAgeements(category:int, dictId:int, oneHanded:Boolean = false, fromSlot:Boolean = false, itemId:int = 0) : void;
      
      function deselectAgeements() : void;
      
      function onItemUpdated(slotId:uint = 0) : void;
   }
}

