package survarium.data
{
   import flash.external.*;
   import flash.utils.Dictionary;
   
   public class ItemDescriptor
   {
      
      protected static var _descriptors:Object = new Object();
      
      protected var _itemDescriptors:Dictionary;
      
      protected var _inventoryItems:Array;
      
      protected var _inventoryItemsDict:Dictionary;
      
      protected var _descriptorName:String;
      
      public function ItemDescriptor(param1:String) : void
      {
         super();
         _descriptorName = param1;
         _descriptors[this._descriptorName] = this;
      }
      
      public static function getItemDescriptor(param1:String) : ItemDescriptor
      {
         return _descriptors[param1];
      }
      
      public function addFakeItemsToDictionary() : void
      {
         if(!this._itemDescriptors)
         {
            _itemDescriptors = new Dictionary();
         }
         var _loc1_:Array = [{
            dictId:0,
            itemLevel:1,
            name:"item_debug_0",
            is_stack:false,
            category:0,
            icon:0
         },{
            dictId:1,
            itemLevel:2,
            name:"item_debug_1",
            is_stack:false,
            category:0,
            icon:1
         },{
            dictId:2,
            itemLevel:3,
            name:"item_debug_2",
            is_stack:true,
            category:1,
            icon:2
         },{
            dictId:3,
            itemLevel:4,
            name:"item_debug_3",
            is_stack:true,
            category:1,
            icon:3
         }];
         var _loc2_:uint = 0;
         while(_loc2_ < _loc1_.length)
         {
            this._itemDescriptors[_loc1_[_loc2_].dictId] = _loc1_[_loc2_];
            _loc2_++;
         }
      }
      
      public function setItemsDictionary(param1:Array) : void
      {
         if(param1 == null)
         {
            return;
         }
         _itemDescriptors = new Dictionary();
         var _loc2_:uint = 0;
         for(; _loc2_ < param1.length; _loc2_++)
         {
            if(param1[_loc2_].is_stack && param1[_loc2_].clip_size == undefined)
            {
               param1[_loc2_].clip_size = 1;
            }
            this._itemDescriptors[param1[_loc2_].dictId] = param1[_loc2_];
            if(param1[_loc2_].item_properties == null)
            {
               continue;
            }
         }
      }
      
      public function getItemDescription(param1:uint) : Object
      {
         if(this._itemDescriptors[param1] == null)
         {
            stackTrace("Wrong dictId [" + param1 + "] in getItemDescription");
            return new Object();
         }
         return this._itemDescriptors[param1];
      }
   }
}

