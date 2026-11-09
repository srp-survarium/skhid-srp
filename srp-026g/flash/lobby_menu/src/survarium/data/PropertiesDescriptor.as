package survarium.data
{
   import flash.external.*;
   import flash.utils.Dictionary;
   
   public class PropertiesDescriptor
   {
      
      protected static var _descriptors:Object = new Object();
      
      protected var _propertyDescriptors:Dictionary;
      
      protected var _descriptorName:String;
      
      public function PropertiesDescriptor(descriptorName:String) : void
      {
         super();
         _descriptorName = descriptorName;
         _descriptors[this._descriptorName] = this;
      }
      
      public static function getPropertiesDescriptor(descriptorName:String) : PropertiesDescriptor
      {
         return _descriptors[descriptorName];
      }
      
      public function setPropertiesDictionary(d:Array) : void
      {
         trace("setPropertiesDictionary::",d.length);
         if(d == null)
         {
            return;
         }
         _propertyDescriptors = new Dictionary();
         var i:uint = 0;
         while(i < d.length)
         {
            this._propertyDescriptors[d[i].prop_id] = d[i];
            i++;
         }
      }
      
      // every property's description (the item search's bonus words: InventoryList.searchBonuses)
      public function descriptions() : Array
      {
         var all:Array = [];
         for each(var description:Object in this._propertyDescriptors)
         {
            all.push(description);
         }
         return all;
      }

      // whether a property is known (getPropertyDescription logs an unknown one)
      public function knows(propId:uint) : Boolean
      {
         return this._propertyDescriptors != null && this._propertyDescriptors[propId] != null;
      }

      public function getPropertyDescription(propId:uint) : Object
      {
         if(this._propertyDescriptors[propId] == null)
         {
            stackTrace("Wrong dictId [" + propId + "] in getPropertyDescription");
            return new Object();
         }
         return this._propertyDescriptors[propId];
      }
   }
}

