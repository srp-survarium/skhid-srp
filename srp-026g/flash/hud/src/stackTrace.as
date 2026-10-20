package
{
   public function stackTrace(v:String) : void
   {
      if(v != null)
      {
         trace(v);
      }
      var tempError:Error = new Error();
      var stackTrace:String = tempError.getStackTrace();
      trace(stackTrace);
   }
}

