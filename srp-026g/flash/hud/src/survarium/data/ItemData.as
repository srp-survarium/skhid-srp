package survarium.data
{
   import survarium.core.Icon;
   
   public class ItemData
   {
      
      public var superCategory:uint;
      
      public var category:uint;
      
      public var value:uint;
      
      public var count:uint;
      
      public var dictId:uint;
      
      public var itemId:uint;
      
      public var iconId:uint;
      
      public var factionId:uint;
      
      public var descriptor:uint;
      
      public var cost:uint;
      
      public var premiumCost:uint;
      
      public var price:uint;
      
      public var itemLevel:uint;
      
      public var matchMakingLevel:uint;
      
      public var clipSize:uint;
      
      public var itemSize:Number = 0;
      
      public var quickSlotItemSize:Number = 0;
      
      public var requiredBags:Number = 0;
      
      public var clipsInBag:Number = 0;
      
      public var repairCostRatio:Number = 0;
      
      public var sellerLevel:uint;
      
      public var slotId:uint;
      
      public var slotTarget:uint;
      
      public var weaponTarget:uint;
      
      public var index:uint;
      
      public var type:uint;
      
      public var weight:Number = 0;
      
      public var rank:uint;
      
      public var repairTime:uint;
      
      public var repairTimeDelta:uint;
      
      public var timeDelta:uint;
      
      public var camoAvailable:uint;
      
      public var decalAvailable:uint;
      
      public var upgradesAvailable:uint;
      
      public var modifierUpgraded:uint;
      
      public var rerollTime:uint;
      
      public var dragEnabled:Boolean = false;
      
      public var isWeapon:Boolean = false;
      
      public var usable:Boolean = false;
      
      public var showed:Boolean = false;
      
      public var isNew:Boolean = false;
      
      public var enabled:Boolean;
      
      public var isStack:Boolean = false;
      
      public var oneHanded:Boolean = false;
      
      public var buyable:Boolean = false;
      
      public var buyValue:Boolean = false;
      
      public var canBuy:Boolean = false;
      
      public var equipped:Boolean = false;
      
      public var isPremium:Boolean = false;
      
      public var broken:Boolean = false;
      
      public var fromBundle:Boolean = false;
      
      public var name:String;
      
      public var itemDescription:String;
      
      public var modifiersDescription:String;
      
      public var tipText:String;
      
      public var lockText:String;
      
      public var unlockLevelLabel:String;
      
      public var modifiers:Array;
      
      public var modifiersDefault:Array;
      
      public var properties:Array;
      
      public var sellers:Array;
      
      public var ammoText:String;
      
      protected var _upgrades:Array;
      
      public var camoItem:int;
      
      public var decalItem:Array;
      
      public var additionalSlots:Array;
      
      public var content:Array;
      
      public var icon:Icon;
      
      public var dragIcon:Icon;
      
      public var messIcon:Icon;
      
      public var sellerIcon:Icon;
      
      public function ItemData()
      {
         super();
      }
      
      public function set upgrades(value:Array) : void
      {
         _upgrades = value;
      }
      
      public function get upgrades() : Array
      {
         return this._upgrades;
      }
      
      public function clone() : ItemData
      {
         var itemData:ItemData = new ItemData();
         var origin:ItemData = this as ItemData;
         trace("ItemDataClone::",origin,origin.icon,origin.dragIcon,origin.messIcon,origin.sellerIcon);
         if(origin.icon)
         {
            itemData.icon = origin.icon.clone();
         }
         if(origin.dragIcon)
         {
            itemData.dragIcon = origin.dragIcon.clone();
         }
         if(origin.messIcon)
         {
            itemData.messIcon = origin.messIcon.clone();
         }
         if(origin.sellerIcon)
         {
            itemData.sellerIcon = origin.sellerIcon.clone();
         }
         itemData.name = origin.name;
         itemData.decalItem = origin.decalItem;
         itemData.camoItem = origin.camoItem;
         itemData.upgrades = origin.upgrades;
         itemData.ammoText = origin.ammoText;
         itemData.superCategory = origin.superCategory;
         itemData.category = origin.category;
         itemData.value = origin.value;
         itemData.count = origin.count;
         itemData.dictId = origin.dictId;
         itemData.itemId = origin.itemId;
         itemData.iconId = origin.iconId;
         itemData.factionId = origin.factionId;
         itemData.descriptor = origin.descriptor;
         itemData.cost = origin.cost;
         itemData.price = origin.price;
         itemData.itemLevel = origin.itemLevel;
         itemData.clipSize = origin.clipSize;
         itemData.itemSize = origin.itemSize;
         itemData.quickSlotItemSize = origin.quickSlotItemSize;
         itemData.requiredBags = origin.requiredBags;
         itemData.clipsInBag = origin.clipsInBag;
         itemData.repairCostRatio = origin.repairCostRatio;
         itemData.modifierUpgraded = origin.modifierUpgraded;
         itemData.sellerLevel = origin.sellerLevel;
         itemData.slotId = origin.slotId;
         itemData.slotTarget = origin.slotTarget;
         itemData.weaponTarget = origin.weaponTarget;
         itemData.index = origin.index;
         itemData.type = origin.type;
         itemData.weight = origin.weight;
         itemData.rank = origin.rank;
         itemData.repairTime = origin.repairTime;
         itemData.repairTimeDelta = origin.repairTimeDelta;
         itemData.camoAvailable = origin.camoAvailable;
         itemData.decalAvailable = origin.decalAvailable;
         itemData.upgradesAvailable = origin.upgradesAvailable;
         itemData.isWeapon = origin.isWeapon;
         itemData.usable = origin.usable;
         itemData.showed = origin.showed;
         itemData.isNew = origin.isNew;
         itemData.enabled = origin.enabled;
         itemData.isStack = origin.isStack;
         itemData.oneHanded = origin.oneHanded;
         itemData.buyable = origin.buyable;
         itemData.buyValue = origin.buyValue;
         itemData.canBuy = origin.canBuy;
         itemData.equipped = origin.equipped;
         itemData.isPremium = origin.isPremium;
         itemData.name = origin.name;
         itemData.itemDescription = origin.itemDescription;
         itemData.modifiersDescription = origin.modifiersDescription;
         itemData.tipText = origin.tipText;
         itemData.lockText = origin.lockText;
         itemData.unlockLevelLabel = origin.unlockLevelLabel;
         itemData.modifiers = origin.modifiers;
         itemData.modifiersDefault = origin.modifiersDefault;
         itemData.properties = origin.properties;
         itemData.sellers = origin.sellers;
         itemData.broken = origin.broken;
         itemData.equipped = origin.equipped;
         itemData.fromBundle = origin.fromBundle;
         itemData.additionalSlots = origin.additionalSlots;
         return itemData;
      }
   }
}

