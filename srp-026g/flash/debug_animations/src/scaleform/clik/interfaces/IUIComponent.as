package scaleform.clik.interfaces
{
   import flash.events.Event;
   import flash.events.IEventDispatcher;
   import flash.geom.Rectangle;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.InputEvent;
   
   public interface IUIComponent extends IEventDispatcher
   {
      
      function get x() : Number;
      
      function set x(value:Number) : void;
      
      function get y() : Number;
      
      function set y(value:Number) : void;
      
      function get width() : Number;
      
      function set width(value:Number) : void;
      
      function get height() : Number;
      
      function set height(value:Number) : void;
      
      function get enabled() : Boolean;
      
      function set enabled(value:Boolean) : void;
      
      function get tabEnabled() : Boolean;
      
      function set tabEnabled(value:Boolean) : void;
      
      function get scale9Grid() : Rectangle;
      
      function set scale9Grid(value:Rectangle) : void;
      
      function get alpha() : Number;
      
      function set alpha(value:Number) : void;
      
      function get doubleClickEnabled() : Boolean;
      
      function set doubleClickEnabled(value:Boolean) : void;
      
      function get focusTarget() : UIComponent;
      
      function set focusTarget(value:UIComponent) : void;
      
      function validateNow(event:Event = null) : void;
      
      function handleInput(event:InputEvent) : void;
   }
}

