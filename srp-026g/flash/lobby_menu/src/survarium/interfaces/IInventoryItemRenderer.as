package survarium.interfaces
{
   import scaleform.clik.interfaces.IUIComponent;
   
   public interface IInventoryItemRenderer extends IUIComponent
   {
      
      function hover(param1:Boolean = true) : void;
      
      function startItemDrag() : void;
   }
}

