package survarium.interfaces
{
   import scaleform.clik.interfaces.IDataProvider;
   import scaleform.clik.interfaces.IUIComponent;
   
   public interface IPaperDoll extends IUIComponent
   {
      
      function get dataProvider() : IDataProvider;
      
      function set dataProvider(param1:IDataProvider) : void;
      
      function openSlots(param1:int, param2:int) : void;
      
      function getSlotAt(param1:int) : IPaperDollSlot;
   }
}

