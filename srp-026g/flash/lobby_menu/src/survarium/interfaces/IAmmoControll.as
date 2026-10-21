package survarium.interfaces
{
   import flash.events.Event;
   import flash.events.IEventDispatcher;
   import survarium.constants.AmmoView;
   import scaleform.clik.interfaces.IDataProvider;
   
   public interface IAmmoControll extends IEventDispatcher
   {
      
      function updatePaperDollData(dataProvider:IDataProvider, slots:Array) : void;
      
      function update(event:Event = null, caller:String = "null") : String;
      
      function fillItems(items:Array) : void;
      
      function openSlots(slotsN:int, slotsA:int) : void;
      
      function setSlotTarget(slotId:uint, targets:Array = null) : void;
      
      function registerShop(shop:*) : void;
      
      function registerInventory(inventory:*) : void;
      
      function reflow(event:Event = null, caller:String = "null", elementIndex:int = AmmoView.USE_PAPERDOLL_SELECTION) : void;
   }
}

