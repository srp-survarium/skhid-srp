package survarium.interfaces
{
   import scaleform.clik.interfaces.IListItemRenderer;
   
   public interface IPaperDollSlot extends IListItemRenderer
   {
      
      function get data() : Object;
      
      function set data(param1:Object) : void;
      
      function hover(param1:Boolean = true) : void;
      
      function dropSlot(param1:Boolean = true) : void;
      
      function reset() : void;
      
      function get highlighted() : Boolean;
      
      function set highlighted(param1:Boolean) : void;
      
      function set artifact(param1:Boolean) : void;
      
      function get artifact() : Boolean;
      
      function set visible(param1:Boolean) : void;
      
      function get visible() : Boolean;
      
      function set label(param1:String) : void;
      
      function invalidateData() : void;
   }
}

