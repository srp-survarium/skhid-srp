package survarium
{
   import avmplus.getQualifiedClassName;
   import flash.display.DisplayObject;
   import flash.display.Stage;
   import survarium.constants.CharCode;
   
   public class Util
   {
      
      private static const decimalPlaces:Array = [["","I","II","III","IV","V","VI","VII","VIII","IX"],["","X","XX","XXX","XL","L","LX","LXX","LXXX","XC"],["","C","CC","CCC","CD","D","DC","DCC","DCCC","CM"],["","M","MM","MMM"]];
      
      public function Util()
      {
         super();
      }
      
      public static function getTimeString(value:int) : String
      {
         var minutes:int = int(value / 60);
         var seconds:int = value % 60;
         var st_min:String = minutes < 10 ? "0" + minutes.toString() : minutes.toString();
         var st_sec:String = seconds < 10 ? "0" + seconds.toString() : seconds.toString();
         return st_min + ":" + st_sec;
      }
      
      public static function getTimeStringMinutes(value:int) : String
      {
         var hours:int = int(value / 60);
         var minutes:int = value % 60;
         var st_min:String = minutes < 10 ? "0" + minutes.toString() : minutes.toString();
         var st_hours:String = hours < 10 ? "0" + hours.toString() : hours.toString();
         return st_hours + ":" + st_min;
      }
      
      public static function getTimeStringHours(value:int) : String
      {
         var hours:int = int(value / 60);
         hours %= 24;
         var minutes:int = value % 60;
         var st_min:String = minutes < 10 ? "0" + minutes.toString() : minutes.toString();
         var st_hours:String = hours < 10 ? "0" + hours.toString() : hours.toString();
         return st_hours + ":" + st_min;
      }
      
      public static function getColor(r:uint, g:uint, b:uint) : uint
      {
         var color:uint = (r << 16 | g << 8 | b);
         return color;
      }
      
      public static function styledChar(value:String, style:String) : String
      {
         return "<" + style + ">" + value + "</" + style + ">";
      }
      
      public static function getIntString(value:int) : String
      {
         var string:String;
         var number:Number = value;
         if(value < 10000)
         {
            return String(value);
         }
         if(value < 1000000)
         {
            string = (number = number / 1000).toString();
         }
         else
         {
            string = (number = number / 1000000).toString();
         }
         if(number != int(number) && string.indexOf(".") < 3)
         {
            string = string.substr(0,4);
         }
         else
         {
            string = string.substr(0,3);
         }
         if(value < 1000000)
         {
            string += "k";
         }
         else
         {
            string += "m";
         }
         return string;
      }
      
      public static function getNumberString(value:Number) : String
      {
         var string:String;
         var number:Number = value;
         if(value < 10000)
         {
            if(number == int(number))
            {
               return String(number);
            }
            else
            {
               return String(int(number / 0.01) * 0.01);
            }
         }
         else if(value < 1000000)
         {
            string = (number = number / 1000).toString();
         }
         else
         {
            string = (number = number / 1000000).toString();
         }
         if(number != int(number) && string.indexOf(".") < 3)
         {
            string = string.substr(0,4);
         }
         else
         {
            string = string.substr(0,3);
         }
         if(value < 1000000)
         {
            string += "k";
         }
         else
         {
            string += "m";
         }
         return string;
      }
      
      public static function getHexString(value:uint) : String
      {
         var _finalString:String = value.toString(16);
         var _len:uint = (_finalString.length);
         var i:int;
         if(_len < 6)
         {
            i = (_len);
            while(i < 6)
            {
               _finalString = "0" + _finalString;
               i++;
            }
         }
         return _finalString;
      }
      
      public static function stringToNumber(value:String) : Number
      {
         var _finalString:String = "";
         var _len:uint = (value.length);
         var i:int = 0;
         var charCode:int;
         while(i < _len)
         {
            charCode = value.charCodeAt(i);
            if(charCode > CharCode.SLASH && charCode < CharCode.COLON || charCode == CharCode.PERIOD)
            {
               _finalString += value.charAt(i);
            }
            i++;
         }
         return Number(_finalString);
      }
      
      public static function getZeroString(value:Number, length:uint = 2) : String
      {
         var _finalString:String = value.toString();
         var _len:uint = (_finalString.length);
         var i:int;
         if(_len < length)
         {
            i = (_len);
            while(i < length)
            {
               _finalString = "0" + _finalString;
               i++;
            }
         }
         return _finalString;
      }
      
      public static function getMoneyString(value:Number) : String
      {
         var _finalString:String = value.toString();
         var _len:uint = (_finalString.length);
         var i:int = (_len);
         while(i > 0)
         {
            _finalString = _finalString.slice(0,i) + " " + _finalString.slice(i,_finalString.length);
            i -= 3;
         }
         return _finalString;
      }
      
      public static function getParentStack(value:DisplayObject) : String
      {
         if(!value)
         {
            return "null";
         }
         var target:DisplayObject = value;
         var stack:String = value.toString();
         while(target)
         {
            if(!target.parent || target is Stage)
            {
               return stack;
            }
            target = target.parent;
            stack += ":" + target.toString() + "|" + getQualifiedClassName(target);
         }
         return stack;
      }
      
      public static function logObject(value:Object) : void
      {
         var str:String = null;
         for(str in value)
         {
            trace("object prop::",str,value[str]);
         }
         trace("\n");
      }
      
      public static function traceRelocate(value:Array) : void
      {
         trace("traceRelocate",value.length);
         for(var i:int = 0; i < value.length; i++)
         {
            trace("relocateItem:",value[i].profile,value[i].id,value[i].dict_id,"sr:",value[i].sourceSlot,"tg:",value[i].targetSlot,"cnt:",value[i].count);
         }
      }
      
      public static function getRomanNumeral(value:uint) : String
      {
         var arabic:String = value.toString();
         var roman:String = "";
         var l:uint = (arabic.length);
         var i:uint = 0;
         while(i < l)
         {
            roman = decimalPlaces[i][int(arabic.charAt(l - i - 1))] + roman;
            i++;
         }
         return roman;
      }
   }
}

