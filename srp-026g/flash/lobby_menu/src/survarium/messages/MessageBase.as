package survarium.messages
{
   import flash.events.Event;
   import scaleform.clik.core.UIComponent;
   
   public class MessageBase extends UIComponent
   {
      
      public var closable:Boolean = true;
      
      protected var _id:int;
      
      protected var _data:Object;
      
      public function MessageBase()
      {
         super();
         addEventListener(Event.REMOVED_FROM_STAGE,this.cleanUp,false,0,true);
      }
      
      public function getMessageId() : int
      {
         return this._id;
      }
      
      public function setMessageData(param1:int, param2:Object) : void
      {
         trace("setMessageData::",param1,param2);
         _id = param1;
         _data = param2;
         invalidateData();
      }
      
      public function getMessageData() : Object
      {
         return this._data;
      }
      
      public function get scale() : Number
      {
         return actualScaleX;
      }
      
      public function set scale(param1:Number) : void
      {
         setActualScale(param1,param1);
      }
      
      protected function cleanUp(param1:Event = null) : void
      {
         removeEventListener(Event.REMOVED_FROM_STAGE,this.cleanUp);
         _data = null;
      }
      
      override public function toString() : String
      {
         return "MessageWidnow_" + this._id + " " + name;
      }
   }
}

