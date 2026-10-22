package survarium.interfaces
{
   import scaleform.clik.controls.DragSlot;
   import survarium.data.ItemData;
   
   public interface IInventoryItemFactory extends IItemFactory
   {
      
      function invalidateList() : void;
      
      function showContextMenu(activeItem:DragSlot) : void;
      
      function selectCategory(categories:Array, deselect:Boolean = false, lockCategory:Boolean = false) : void;
      
      function setPrimaryItem(item:ItemData) : void;
      
      function selectItem(itemId:int) : void;
      
      function getElelementLocked(elementId:uint) : Boolean;
   }
}

