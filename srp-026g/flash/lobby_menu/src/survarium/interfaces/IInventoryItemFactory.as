package survarium.interfaces
{
   import scaleform.clik.controls.DragSlot;
   import survarium.data.ItemData;
   
   public interface IInventoryItemFactory extends IItemFactory
   {
      
      function invalidateList() : void;
      
      function showContextMenu(param1:DragSlot) : void;
      
      function selectCategory(param1:Array, param2:Boolean = false, param3:Boolean = false) : void;
      
      function setPrimaryItem(param1:ItemData) : void;
      
      function selectItem(param1:int) : void;
      
      function getElelementLocked(param1:uint) : Boolean;
   }
}

