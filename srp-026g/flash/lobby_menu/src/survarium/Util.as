package survarium
{
   import avmplus.getQualifiedClassName;
   import flash.display.DisplayObject;
   import flash.display.Stage;
   
   public class Util
   {
      
      private static const decimalPlaces:Array = [["","I","II","III","IV","V","VI","VII","VIII","IX"],["","X","XX","XXX","XL","L","LX","LXX","LXXX","XC"],["","C","CC","CCC","CD","D","DC","DCC","DCCC","CM"],["","M","MM","MMM"]];
      
      public function Util()
      {
         super();
      }
      
      public static function getTimeString(param1:int) : String
      {
         var _loc2_:int = int(param1 / 60);
         var _loc3_:int = param1 % 60;
         var _loc4_:String = _loc2_ < 10 ? "0" + _loc2_.toString() : _loc2_.toString();
         var _loc5_:String = _loc3_ < 10 ? "0" + _loc3_.toString() : _loc3_.toString();
         return _loc4_ + ":" + _loc5_;
      }
      
      public static function getTimeStringMinutes(param1:int) : String
      {
         var _loc2_:int = int(param1 / 60);
         var _loc3_:int = param1 % 60;
         var _loc4_:String = _loc3_ < 10 ? "0" + _loc3_.toString() : _loc3_.toString();
         var _loc5_:String = _loc2_ < 10 ? "0" + _loc2_.toString() : _loc2_.toString();
         return _loc5_ + ":" + _loc4_;
      }
      
      public static function getTimeStringHours(param1:int) : String
      {
         var _loc2_:int = int(param1 / 60);
         _loc2_ %= 24;
         var _loc3_:int = param1 % 60;
         var _loc4_:String = _loc3_ < 10 ? "0" + _loc3_.toString() : _loc3_.toString();
         var _loc5_:String = _loc2_ < 10 ? "0" + _loc2_.toString() : _loc2_.toString();
         return _loc5_ + ":" + _loc4_;
      }
      
      public static function getColor(param1:uint, param2:uint, param3:uint) : uint
      {
         var _loc4_:uint = (param1 << 16 | param2 << 8 | param3);
         return _loc4_;
      }
      
      public static function styledChar(param1:String, param2:String) : String
      {
         return "<" + param2 + ">" + param1 + "</" + param2 + ">";
      }
      
      public static function getIntString(param1:int) : String
      {
         var _loc2_:String;
         var _loc3_:Number = param1;
         if(param1 < 10000)
         {
            return String(param1);
         }
         if(param1 < 1000000)
         {
            _loc2_ = (_loc3_ = _loc3_ / 1000).toString();
         }
         else
         {
            _loc2_ = (_loc3_ = _loc3_ / 1000000).toString();
         }
         if(_loc3_ != int(_loc3_) && _loc2_.indexOf(".") < 3)
         {
            _loc2_ = _loc2_.substr(0,4);
         }
         else
         {
            _loc2_ = _loc2_.substr(0,3);
         }
         if(param1 < 1000000)
         {
            _loc2_ += "k";
         }
         else
         {
            _loc2_ += "m";
         }
         return _loc2_;
      }
      
      public static function getNumberString(param1:Number) : String
      {
         var _loc2_:String;
         var _loc3_:Number = param1;
         if(param1 < 10000)
         {
            if(_loc3_ == int(_loc3_))
            {
               return String(_loc3_);
            }
            else
            {
               return String(int(_loc3_ / 0.01) * 0.01);
            }
         }
         else if(param1 < 1000000)
         {
            _loc2_ = (_loc3_ = _loc3_ / 1000).toString();
         }
         else
         {
            _loc2_ = (_loc3_ = _loc3_ / 1000000).toString();
         }
         if(_loc3_ != int(_loc3_) && _loc2_.indexOf(".") < 3)
         {
            _loc2_ = _loc2_.substr(0,4);
         }
         else
         {
            _loc2_ = _loc2_.substr(0,3);
         }
         if(param1 < 1000000)
         {
            _loc2_ += "k";
         }
         else
         {
            _loc2_ += "m";
         }
         return _loc2_;
      }
      
      public static function getHexString(param1:uint) : String
      {
         var _loc2_:String = param1.toString(16);
         var _loc3_:uint = (_loc2_.length);
         var _loc4_:int;
         if(_loc3_ < 6)
         {
            _loc4_ = (_loc3_);
            while(_loc4_ < 6)
            {
               _loc2_ = "0" + _loc2_;
               _loc4_++;
            }
         }
         return _loc2_;
      }
      
      public static function stringToNumber(param1:String) : Number
      {
         var _loc2_:String = "";
         var _loc3_:uint = (param1.length);
         var _loc4_:int = 0;
         var _loc5_:int;
         while(_loc4_ < _loc3_)
         {
            _loc5_ = param1.charCodeAt(_loc4_);
            if(_loc5_ > 47 && _loc5_ < 58 || _loc5_ == 46)
            {
               _loc2_ += param1.charAt(_loc4_);
            }
            _loc4_++;
         }
         return Number(_loc2_);
      }
      
      public static function getZeroString(param1:Number, param2:uint = 2) : String
      {
         var _loc3_:String = param1.toString();
         var _loc4_:uint = (_loc3_.length);
         var _loc5_:int;
         if(_loc4_ < param2)
         {
            _loc5_ = (_loc4_);
            while(_loc5_ < param2)
            {
               _loc3_ = "0" + _loc3_;
               _loc5_++;
            }
         }
         return _loc3_;
      }
      
      public static function getMoneyString(param1:Number) : String
      {
         var _loc2_:String = param1.toString();
         var _loc3_:uint = (_loc2_.length);
         var _loc4_:int = (_loc3_);
         while(_loc4_ > 0)
         {
            _loc2_ = _loc2_.slice(0,_loc4_) + " " + _loc2_.slice(_loc4_,_loc2_.length);
            _loc4_ -= 3;
         }
         return _loc2_;
      }
      
      public static function getParentStack(param1:DisplayObject) : String
      {
         if(!param1)
         {
            return "null";
         }
         var _loc2_:DisplayObject = param1;
         var _loc3_:String = param1.toString();
         while(_loc2_)
         {
            if(!_loc2_.parent || _loc2_ is Stage)
            {
               return _loc3_;
            }
            _loc2_ = _loc2_.parent;
            _loc3_ += ":" + _loc2_.toString() + "|" + getQualifiedClassName(_loc2_);
         }
         return _loc3_;
      }
      
      public static function logObject(param1:Object) : void
      {
         var _loc2_:String;
         for(_loc2_ in param1)
         {
            trace("object prop::",_loc2_,param1[_loc2_]);
         }
         trace("\n");
      }
      
      public static function traceRelocate(param1:Array) : void
      {
         trace("traceRelocate",param1.length);
         var _loc2_:int = 0;
         while(_loc2_ < param1.length)
         {
            trace("relocateItem:",param1[_loc2_].profile,param1[_loc2_].id,param1[_loc2_].dict_id,"sr:",param1[_loc2_].sourceSlot,"tg:",param1[_loc2_].targetSlot,"cnt:",param1[_loc2_].count);
            _loc2_++;
         }
      }
      
      public static function getRomanNumeral(param1:uint) : String
      {
         var _loc2_:String = param1.toString();
         var _loc3_:String = "";
         var _loc4_:uint = (_loc2_.length);
         var _loc5_:uint = 0;
         while(_loc5_ < _loc4_)
         {
            _loc3_ = decimalPlaces[_loc5_][int(_loc2_.charAt(_loc4_ - _loc5_ - 1))] + _loc3_;
            _loc5_++;
         }
         return _loc3_;
      }
   }
}

