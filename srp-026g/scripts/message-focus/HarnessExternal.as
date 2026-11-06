package
{
   // The exe's side of ExternalInterface for the harness: check.sh's test copies of the movies call
   // this instead (Ruffle has no ExternalInterface; flash.external is the player's and cannot be
   // replaced). Each call is recorded by its name.
   public class HarnessExternal
   {
      public static var calls:Array = [];

      public function HarnessExternal()
      {
         super();
      }

      public static function get available() : Boolean
      {
         return true;
      }

      public static function call(functionName:String, ... args) : *
      {
         calls.push(functionName);
         return null;
      }

      public static function addCallback(functionName:String, closure:Function) : void
      {
      }
   }
}
