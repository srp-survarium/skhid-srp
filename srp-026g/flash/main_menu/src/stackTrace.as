package
{
   public function stackTrace(param1:String) : void
   {
      if(param1 != null)
      {
         trace(param1);
      }
      var _loc2_:Error = new Error();
      var _loc3_:String = _loc2_.getStackTrace();
      trace(_loc3_);
   }
}

