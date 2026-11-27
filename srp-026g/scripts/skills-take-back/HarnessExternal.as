package
{
   // The exe's side of ExternalInterface for the skills window's harness: check.sh's test copy of the
   // lobby movie calls this instead (Ruffle has no ExternalInterface; flash.external is the player's
   // and cannot be replaced). Each call is kept as its name and its arguments as they were given.
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
         calls.push({name:functionName,args:args});
         return null;
      }

      public static function addCallback(functionName:String, closure:Function) : void
      {
      }
   }
}
