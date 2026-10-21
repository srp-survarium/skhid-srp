package scaleform.clik.interfaces
{
   import flash.display.Sprite;
   import scaleform.clik.events.DragEvent;
   
   public interface IDragSlot extends IUIComponent
   {
      
      function get data() : Object;
      
      function set data(value:Object) : void;
      
      function get content() : Sprite;
      
      function set content(value:Sprite) : void;
      
      function get descriptor() : uint;
      
      function set descriptor(value:uint) : void;
      
      function handleDropEvent(e:DragEvent) : Boolean;
      
      function handleDragStartEvent(e:DragEvent) : void;
      
      function handleDragEndEvent(e:DragEvent, wasValidDrop:Boolean) : void;
   }
}

