package scaleform.clik.interfaces
{
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.ListData;
   
   public interface IListItemRenderer extends IUIComponent
   {
      
      function get index() : uint;
      
      function set index(value:uint) : void;
      
      function get owner() : UIComponent;
      
      function set owner(value:UIComponent) : void;
      
      function get selectable() : Boolean;
      
      function set selectable(value:Boolean) : void;
      
      function get selected() : Boolean;
      
      function set selected(value:Boolean) : void;
      
      function get displayFocus() : Boolean;
      
      function set displayFocus(value:Boolean) : void;
      
      function setListData(listData:ListData) : void;
      
      function setData(data:Object) : void;
   }
}

