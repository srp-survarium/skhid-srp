package survarium.controls
{
   public class MinimapObjectPlayer extends MinimapObject
   {
      
      protected var _type:int = -1;
      
      protected var _isCarryingItem:Boolean = false;
      
      public function MinimapObjectPlayer()
      {
         super();
         stop();
      }
      
      public function set isCarryingItem(value:Boolean) : void
      {
         _isCarryingItem = value;
      }
      
      public function get isCarryingItem() : Boolean
      {
         return this._isCarryingItem;
      }
      
      public function set type(value:int) : void
      {
         gotoAndStop(value);
         _type = value;
      }
   }
}

