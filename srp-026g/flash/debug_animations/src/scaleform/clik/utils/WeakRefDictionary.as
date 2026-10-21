package scaleform.clik.utils
{
   import flash.utils.Dictionary;
   
   public class WeakRefDictionary
   {
      
      protected var _dictionary:Dictionary;
      
      public function WeakRefDictionary()
      {
         super();
         _dictionary = new Dictionary(true);
      }
      
      public function setValue(key:Object, value:Object) : void
      {
         var dvalue:Object;
         var dkey:Object;
         for(dvalue in this._dictionary)
         {
            dkey = this._dictionary[dvalue];
            if(key == dkey)
            {
               delete this._dictionary[dvalue];
            }
         }
         this._dictionary[value] = key;
      }
      
      public function getValue(key:Object) : Object
      {
         var dvalue:Object;
         var dkey:Object;
         for(dvalue in this._dictionary)
         {
            dkey = this._dictionary[dvalue];
            if(key == dkey)
            {
               return dvalue;
            }
         }
         return null;
      }
   }
}

