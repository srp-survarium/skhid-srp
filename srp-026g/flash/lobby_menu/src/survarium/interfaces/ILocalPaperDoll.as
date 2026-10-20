package survarium.interfaces
{
   import survarium.data.ItemData;
   
   public interface ILocalPaperDoll extends IPaperDoll
   {
      
      function set ammoControl(param1:IAmmoControll) : void;
      
      function get ammoControl() : IAmmoControll;
      
      function get selectedIndex() : int;
      
      function setSlotsRestrictions(param1:Array) : void;
      
      function registerInventory(param1:IInventoryItemFactory) : void;
      
      function enableSecondWeapon(param1:Boolean) : void;
      
      function allowSecondaryTwoHandedWeapon(param1:Boolean) : void;
      
      function lockSlot(param1:Boolean, param2:uint) : void;
      
      function getSlot(param1:ItemData, param2:Boolean = false) : int;
      
      function hoverItem(param1:uint = 0) : void;
      
      function clearSelection() : void;
      
      function hasAgreements(param1:uint) : uint;
      
      function selectAgeements(param1:int, param2:int, param3:Boolean = false, param4:Boolean = false, param5:int = 0) : void;
      
      function deselectAgeements() : void;
      
      function onItemUpdated(param1:uint = 0) : void;
   }
}

