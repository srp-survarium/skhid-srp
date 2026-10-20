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
      
      public function ItemDescriptor(descriptorName:String) : void
      {
         super();
         _descriptorName = descriptorName;
         _descriptors[this._descriptorName] = this;
      }
      
      public static function getItemDescriptor(descriptorName:String) : ItemDescriptor
      {
         return _descriptors[descriptorName];
      }
      
      public function addFakeItemsToDictionary() : void
      {
         if(!this._itemDescriptors)
         {
            _itemDescriptors = new Dictionary();
         }
         var d:Array = [{
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
         var i:uint = 0;
         while(i < d.length)
         {
            this._itemDescriptors[d[i].dictId] = d[i];
            i++;
         }
      }
      
      public function setItemsDictionary(d:Array) : void
      {
         if(d == null)
         {
            return;
         }
         _itemDescriptors = new Dictionary();
         var i:uint = 0;
         for(; i < d.length; i++)
         {
            if(d[i].is_stack && d[i].clip_size == undefined)
            {
               d[i].clip_size = 1;
            }
            this._itemDescriptors[d[i].dictId] = d[i];
            if(d[i].item_properties == null)
            {
               continue;
            }
         }
      }
      
      public function getItemDescription(dictId:uint) : Object
      {
         if(this._itemDescriptors[dictId] == null)
         {
            stackTrace("Wrong dictId [" + dictId + "] in getItemDescription");
            return new Object();
         }
         return this._itemDescriptors[dictId];
      }
   }
}

