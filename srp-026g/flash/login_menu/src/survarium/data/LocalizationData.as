package survarium.data
{
   import flash.text.TextField;
   import flash.utils.Dictionary;
   import scaleform.clik.controls.Button;
   
   public class LocalizationData
   {
      
      private static var _data:Object;
      
      private static var _dict:Dictionary = new Dictionary(true);
      
      private static var _dict_callback:Dictionary = new Dictionary(true);
      
      protected static var _nullLocals:Dictionary = new Dictionary();
      
      public function LocalizationData()
      {
         super();
      }
      
      public static function setData(param1:Object) : void
      {
         _data = param1;
         _nullLocals = new Dictionary();
         updateFileds();
         updateCallbacks();
      }
      
      public static function getName(param1:String) : String
      {
         if(_data && _data[param1] != undefined)
         {
            return _data[param1];
         }
         _nullLocals[param1] = true;
         return "#" + param1;
      }
      
      public static function logNullLocals() : void
      {
         var _loc1_:String = "";
         var _loc2_:Object;
         for(_loc2_ in _nullLocals)
         {
            _loc1_ += String(_loc2_) + "\n";
         }
         trace("logNullLocals::",_loc1_);
      }
      
      public static function addCallback(param1:Function, param2:String) : void
      {
         if(_data)
         {
            param1(getName(param2));
         }
         else
         {
            _dict_callback[param1] = param2;
         }
      }
      
      public static function addTextField(param1:TextField, param2:String) : void
      {
         _dict[param1] = param2;
         if(_data)
         {
            param1.text = getName(param2);
         }
      }
      
      public static function addButton(param1:Button, param2:String) : void
      {
         _dict[param1] = param2;
         if(_data)
         {
            param1.label = getName(param2);
         }
      }
      
      protected static function updateCallbacks() : void
      {
         var _loc1_:Object;
         var _loc2_:Function;
         for(_loc1_ in _dict_callback)
         {
            _loc2_ = _loc1_ as Function;
            _loc2_(getName(_dict_callback[_loc1_]));
         }
      }
      
      protected static function updateFileds() : void
      {
         var _loc1_:Object;
         for(_loc1_ in _dict)
         {
            if(_loc1_ is TextField)
            {
               _loc1_.text = getName(_dict[_loc1_]);
            }
            else if(_loc1_ is Button)
            {
               _loc1_.label = getName(_dict[_loc1_]);
            }
         }
      }
   }
}

