package survarium.interfaces
{
   import scaleform.clik.events.DragEvent;
   
   public interface ILocalPlayerProfile extends IPlayerProfile
   {
      
      function set _relocateFunction(param1:Function) : void;
      
      function get _relocateFunction() : Function;
      
      function get paperDoll() : ILocalPaperDoll;
      
      function enableSecondWeapon(param1:Boolean) : void;
      
      function allowSecondaryTwoHandedWeapon(param1:Boolean) : void;
      
      function updateWeight(param1:Number, param2:Number) : void;
      
      function registerRelocateFunction(param1:Function) : void;
      
      function lock(param1:Boolean, param2:uint) : void;
      
      function onDrag(param1:DragEvent) : void;
      
      function fillEquippedItems() : void;
      
      function refillItem(param1:uint) : void;
      
      function handleUnresolvedItems() : void;
      
      function setPlayerSpeed(param1:uint) : void;
      
      function swapQuickSlots(param1:int, param2:int) : void;
      
      function setAutoRepair(param1:Boolean) : void;
      
      function setAmmoShare(param1:IAmmoShare) : void;
      
      function get_ammo_share() : IAmmoShare;
   }
}

