package survarium.interfaces
{
   import flash.events.Event;
   import flash.events.IEventDispatcher;
   import scaleform.clik.interfaces.IDataProvider;
   
   public interface IAmmoControll extends IEventDispatcher
   {
      
      function updatePaperDollData(param1:IDataProvider, param2:Array) : void;
      
      function update(param1:Event = null, param2:String = "null") : String;
      
      function fillItems(param1:Array) : void;
      
      function openSlots(param1:int, param2:int) : void;
      
      function setSlotTarget(param1:uint, param2:Array = null) : void;
      
      function registerShop(param1:*) : void;
      
      function registerInventory(param1:*) : void;
      
      function reflow(param1:Event = null, param2:String = "null", param3:int = -2) : void;
   }
}

