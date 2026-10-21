package scaleform.clik.interfaces
{
   import flash.events.Event;
   
   public interface IDataProvider
   {
      
      function get length() : uint;
      
      function requestItemAt(index:uint, callBack:Function = null) : Object;
      
      function requestItemRange(startIndex:int, endIndex:int, callBack:Function = null) : Array;
      
      function indexOf(item:Object, callBack:Function = null) : int;
      
      function cleanUp() : void;
      
      function invalidate(length:uint = 0) : void;
      
      function addEventListener(type:String, listener:Function, useCapture:Boolean = false, priority:int = 0, useWeakReference:Boolean = false) : void;
      
      function removeEventListener(type:String, listener:Function, useCapture:Boolean = false) : void;
      
      function dispatchEvent(event:Event) : Boolean;
      
      function hasEventListener(type:String) : Boolean;
      
      function willTrigger(type:String) : Boolean;
   }
}

