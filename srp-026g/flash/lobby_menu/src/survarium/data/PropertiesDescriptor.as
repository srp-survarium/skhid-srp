package survarium.data
{
   import flash.external.*;
   import flash.utils.Dictionary;
   
   public class PropertiesDescriptor
   {
      
      protected static var _descriptors:Object = new Object();
      
      protected var _propertyDescriptors:Dictionary;
      
      protected var _descriptorName:String;
      
      public function PropertiesDescriptor(param1:String) : void
      {
         super();
         _descriptorName = param1;
         _descriptors[this._descriptorName] = this;
      }
      
      public static function getPropertiesDescriptor(param1:String) : PropertiesDescriptor
      {
         return _descriptors[param1];
      }
      
      public function setPropertiesDictionary(param1:Array) : void
      {
         trace("setPropertiesDictionary::",param1.length);
         if(param1 == null)
         {
            return;
         }
         _propertyDescriptors = new Dictionary();
         var _loc2_:uint = 0;
         while(_loc2_ < param1.length)
         {
            this._propertyDescriptors[param1[_loc2_].prop_id] = param1[_loc2_];
            _loc2_++;
         }
      }
      
      public function getPropertyDescription(param1:uint) : Object
      {
         if(this._propertyDescriptors[param1] == null)
         {
            stackTrace("Wrong dictId [" + param1 + "] in getPropertyDescription");
            return new Object();
         }
         return this._propertyDescriptors[param1];
      }
   }
}

