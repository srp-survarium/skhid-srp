package survarium.interfaces
{
   import survarium.data.ItemData;
   import survarium.data.ItemDescriptor;
   import survarium.constants.Faction;
   
   public interface IItemFactory
   {
      
      function createBaseItem(dictId:uint) : ItemData;
      
      function createItem(dictId:uint, seller:* = Faction.ANY) : ItemData;
      
      function createItemData(value:Object) : ItemData;
      
      function getInventoryItem(itemId:uint) : ItemData;
      
      function registerItemDescriptor(link:ItemDescriptor) : void;
      
      function fillRewardItems(value:Array) : void;
   }
}

