package scaleform.clik.utils
{
   import flash.utils.Dictionary;
   
   public class WeakRefDictionary
   {
      
      protected var _dictionary:Dictionary;
      
      public function WeakRefDictionary()
      {
         super();
         this._dictionary = new Dictionary(true);
      }
      
      public function setValue(param1:Object, param2:Object) : void
      {
         var _loc3_:Object = null;
         var _loc4_:Object = null;
         for(_loc3_ in this._dictionary)
         {
            _loc4_ = this._dictionary[_loc3_];
            if(param1 == _loc4_)
            {
               delete this._dictionary[_loc3_];
            }
         }
         this._dictionary[param2] = param1;
      }
      
      public function getValue(param1:Object) : Object
      {
         var _loc2_:Object = null;
         var _loc3_:Object = null;
         for(_loc2_ in this._dictionary)
         {
            _loc3_ = this._dictionary[_loc2_];
            if(param1 == _loc3_)
            {
               return _loc2_;
            }
         }
         return null;
      }
   }
}

