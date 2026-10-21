package survarium.interfaces
{
   import scaleform.clik.interfaces.IUIComponent;
   
   public interface IPlayerProfile extends IUIComponent
   {
      
      function get profileId() : int;
      
      function registerInventory(inventory:IItemFactory) : void;
      
      function updateAmmoBags(count:uint) : void;
      
      function getAmmoBags() : uint;
      
      function updatePocketSize(size:uint) : void;
      
      function getPocketSize() : uint;
      
      function setupProfiles(profiles:Array) : void;
      
      function updateSlots(slotsA:uint, slotsN:uint) : void;
      
      function fillProfileItems(profileData:Object) : void;
   }
}

