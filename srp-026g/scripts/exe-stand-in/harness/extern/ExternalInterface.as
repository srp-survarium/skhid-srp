package harness.extern
{
   // Test fixture, in the harnesses' swfs only: the exe a movie calls. ruffle_movie.py --exe renames
   // the movie's flash.external package to this one in a copy for Ruffle (which has no exe), so the
   // movie's own ExternalInterface.call lands here and a harness reads what it was told to call,
   // with no public hook in the players' movies. The loading harness's ApplicationDomain is the
   // parent of the movie's, so this class resolves. calls: [name, args...] oldest first.
   public class ExternalInterface
   {
      public static var calls:Array = [];

      public function ExternalInterface()
      {
         super();
      }

      public static function call(name:String, ... args) : *
      {
         calls.push([name].concat(args));
         return null;
      }

      // The last call of `name`, null none.
      public static function last(name:String) : Array
      {
         for(var i:int = calls.length - 1; i >= 0; i--)
         {
            if(calls[i][0] == name)
            {
               return calls[i];
            }
         }
         return null;
      }

      public static function count(name:String) : int
      {
         var n:int = 0;
         for each(var c:Array in calls)
         {
            if(c[0] == name)
            {
               n++;
            }
         }
         return n;
      }
   }
}
