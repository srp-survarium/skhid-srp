package survarium.interfaces
{
   import scaleform.clik.interfaces.IListItemRenderer;
   
   public interface IPaperDollSlot extends IListItemRenderer
   {
      
      function get data() : Object;
      
      function set data(value:Object) : void;
      
      function hover(isHovered:Boolean = true) : void;
      
      function dropSlot(manual:Boolean = true) : void;
      
      function reset() : void;
      
      function get highlighted() : Boolean;
      
      function set highlighted(value:Boolean) : void;
      
      function set artifact(value:Boolean) : void;
      
      function get artifact() : Boolean;
      
      function set visible(value:Boolean) : void;
      
      function get visible() : Boolean;
      
      function set label(value:String) : void;
      
      function invalidateData() : void;
   }
}

