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
      
      public function setMessageData(id:int, messageData:Object) : void
      {
         trace("setMessageData::",id,messageData);
         _id = id;
         _data = messageData;
         invalidateData();
      }
      
      public function get scale() : Number
      {
         return actualScaleX;
      }
      
      public function set scale(value:Number) : void
      {
         setActualScale(value,value);
      }
      
      protected function cleanUp(event:Event = null) : void
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

