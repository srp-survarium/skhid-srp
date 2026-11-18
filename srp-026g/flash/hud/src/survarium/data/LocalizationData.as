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
      
      public static function setData(value:Object) : void
      {
         _data = value;
         _nullLocals = new Dictionary();
         updateFileds();
         updateCallbacks();
      }
      
      public static function getName(value:String) : String
      {
         if(_data && _data[value] != undefined)
         {
            return _data[value];
         }
         _nullLocals[value] = true;
         return "#" + value;
      }

      // A text with values: %1$s, %2$s, ... in it, in any order a language needs.
      public static function format(value:String, ... values) : String
      {
         var text:String = getName(value);
         for(var i:int = 0; i < values.length; i++)
         {
            text = text.split("%" + (i + 1) + "$s").join(String(values[i]));
         }
         return text;
      }

      public static function logNullLocals() : void
      {
         var str:String = "";
         var tx:Object;
         for(tx in _nullLocals)
         {
            str += String(tx) + "\n";
         }
         trace("logNullLocals::",str);
      }
      
      public static function addCallback(fn:Function, value:String) : void
      {
         if(_data)
         {
            fn(getName(value));
         }
         else
         {
            _dict_callback[fn] = value;
         }
      }
      
      public static function addTextField(tx:TextField, value:String) : void
      {
         _dict[tx] = value;
         if(_data)
         {
            tx.text = getName(value);
         }
      }
      
      public static function addButton(bt:Button, value:String) : void
      {
         _dict[bt] = value;
         if(_data)
         {
            bt.label = getName(value);
         }
      }
      
      protected static function updateCallbacks() : void
      {
         var fn:Object;
         var fnc:Function;
         for(fn in _dict_callback)
         {
            fnc = fn as Function;
            fnc(getName(_dict_callback[fn]));
         }
      }
      
      protected static function updateFileds() : void
      {
         var tx:Object;
         for(tx in _dict)
         {
            if(tx is TextField)
            {
               tx.text = getName(_dict[tx]);
            }
            else if(tx is Button)
            {
               tx.label = getName(_dict[tx]);
            }
         }
      }
   }
}

