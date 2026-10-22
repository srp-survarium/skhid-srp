package survarium.interfaces
{
   import scaleform.clik.events.DragEvent;
   
   public interface ILocalPlayerProfile extends IPlayerProfile
   {
      
      function set _relocateFunction(value:Function) : void;
      
      function get _relocateFunction() : Function;
      
      function get paperDoll() : ILocalPaperDoll;
      
      function enableSecondWeapon(value:Boolean) : void;
      
      function allowSecondaryTwoHandedWeapon(value:Boolean) : void;
      
      function updateWeight(current:Number, maximum:Number) : void;
      
      function registerRelocateFunction(callback:Function) : void;
      
      function lock(locked:Boolean, slotIndex:uint) : void;
      
      function onDrag(event:DragEvent) : void;
      
      function fillEquippedItems() : void;
      
      function refillItem(itemId:uint) : void;
      
      function handleUnresolvedItems() : void;
      
      function setPlayerSpeed(speed:uint) : void;
      
      function swapQuickSlots(firstIndex:int, secondIndex:int) : void;
      
      function setAutoRepair(value:Boolean) : void;
      
      function setAmmoShare(ammoShare:IAmmoShare) : void;
      
      function get_ammo_share() : IAmmoShare;
   }
}

