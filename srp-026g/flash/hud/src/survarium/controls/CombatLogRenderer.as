package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   
   public class CombatLogRenderer extends MovieClip
   {
      
      protected var _data:Object;
      
      protected var _owner:MovieClip;
      
      public var logItem:survarium.controls.CombatLogItem;
      
      public function CombatLogRenderer()
      {
         super();
         addEventListener(Event.ENTER_FRAME,this._handleEvent);
      }
      
      public function setData(data:Object, owner:MovieClip) : void
      {
         this._data = data;
         this._owner = owner;
         this.logItem.setData(data);
      }
      
      private function _handleEvent(e:Event) : void
      {
         if(currentFrame == totalFrames)
         {
            this._owner.delItem();
         }
      }
      
      public function clearEvents() : void
      {
         removeEventListener(Event.ENTER_FRAME,this._handleEvent);
      }
      
      override public function toString() : String
      {
         return "[CombatLogItem " + "]";
      }
   }
}

