package survarium.interfaces
{
   import survarium.data.ItemData;
   import survarium.data.ItemDescriptor;
   
   public interface IItemFactory
   {
      
      function createBaseItem(param1:uint) : ItemData;
      
      function createItem(param1:uint, param2:* = -1) : ItemData;
      
      function createItemData(param1:Object) : ItemData;
      
      function getInventoryItem(param1:uint) : ItemData;
      
      function registerItemDescriptor(param1:ItemDescriptor) : void;
      
      function fillRewardItems(param1:Array) : void;
   }
}

