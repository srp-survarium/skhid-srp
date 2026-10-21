package survarium.interfaces
{
   import scaleform.clik.interfaces.IDataProvider;
   import scaleform.clik.interfaces.IUIComponent;
   
   public interface IPaperDoll extends IUIComponent
   {
      
      function get dataProvider() : IDataProvider;
      
      function set dataProvider(value:IDataProvider) : void;
      
      function openSlots(slotsN:int, slotsA:int) : void;
      
      function getSlotAt(slotIndex:int) : IPaperDollSlot;
   }
}

