package survarium.interfaces
{
   import scaleform.clik.interfaces.IUIComponent;
   
   public interface IPlayerProfile extends IUIComponent
   {
      
      function get profileId() : int;
      
      function registerInventory(param1:IItemFactory) : void;
      
      function updateAmmoBags(param1:uint) : void;
      
      function getAmmoBags() : uint;
      
      function updatePocketSize(param1:uint) : void;
      
      function getPocketSize() : uint;
      
      function setupProfiles(param1:Array) : void;
      
      function updateSlots(param1:uint, param2:uint) : void;
      
      function fillProfileItems(param1:Object) : void;
   }
}

