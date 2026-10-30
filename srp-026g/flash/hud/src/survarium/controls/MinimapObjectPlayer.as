package survarium.controls
{
   public class MinimapObjectPlayer extends MinimapObject
   {
      
      protected var _type:int = -1;
      
      public function MinimapObjectPlayer()
      {
         super();
         stop();
      }
      
      public function set type(value:int) : void
      {
         gotoAndStop(value);
         _type = value;
      }
   }
}

