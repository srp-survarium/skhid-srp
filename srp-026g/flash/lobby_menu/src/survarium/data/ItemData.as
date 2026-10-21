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
      
      public function set upgrades(param1:Array) : void
      {
         _upgrades = param1;
      }
      
      public function get upgrades() : Array
      {
         return this._upgrades;
      }
      
      public function clone() : ItemData
      {
         var _loc1_:ItemData = new ItemData();
         var _loc2_:ItemData = this as ItemData;
         trace("ItemDataClone::",_loc2_,_loc2_.icon,_loc2_.dragIcon,_loc2_.messIcon,_loc2_.sellerIcon);
         if(_loc2_.icon)
         {
            _loc1_.icon = _loc2_.icon.clone();
         }
         if(_loc2_.dragIcon)
         {
            _loc1_.dragIcon = _loc2_.dragIcon.clone();
         }
         if(_loc2_.messIcon)
         {
            _loc1_.messIcon = _loc2_.messIcon.clone();
         }
         if(_loc2_.sellerIcon)
         {
            _loc1_.sellerIcon = _loc2_.sellerIcon.clone();
         }
         _loc1_.name = _loc2_.name;
         _loc1_.decalItem = _loc2_.decalItem;
         _loc1_.camoItem = _loc2_.camoItem;
         _loc1_.upgrades = _loc2_.upgrades;
         _loc1_.ammoText = _loc2_.ammoText;
         _loc1_.superCategory = _loc2_.superCategory;
         _loc1_.category = _loc2_.category;
         _loc1_.value = _loc2_.value;
         _loc1_.count = _loc2_.count;
         _loc1_.dictId = _loc2_.dictId;
         _loc1_.itemId = _loc2_.itemId;
         _loc1_.iconId = _loc2_.iconId;
         _loc1_.factionId = _loc2_.factionId;
         _loc1_.descriptor = _loc2_.descriptor;
         _loc1_.cost = _loc2_.cost;
         _loc1_.price = _loc2_.price;
         _loc1_.itemLevel = _loc2_.itemLevel;
         _loc1_.clipSize = _loc2_.clipSize;
         _loc1_.itemSize = _loc2_.itemSize;
         _loc1_.quickSlotItemSize = _loc2_.quickSlotItemSize;
         _loc1_.requiredBags = _loc2_.requiredBags;
         _loc1_.clipsInBag = _loc2_.clipsInBag;
         _loc1_.repairCostRatio = _loc2_.repairCostRatio;
         _loc1_.modifierUpgraded = _loc2_.modifierUpgraded;
         _loc1_.sellerLevel = _loc2_.sellerLevel;
         _loc1_.slotId = _loc2_.slotId;
         _loc1_.slotTarget = _loc2_.slotTarget;
         _loc1_.weaponTarget = _loc2_.weaponTarget;
         _loc1_.index = _loc2_.index;
         _loc1_.type = _loc2_.type;
         _loc1_.weight = _loc2_.weight;
         _loc1_.rank = _loc2_.rank;
         _loc1_.repairTime = _loc2_.repairTime;
         _loc1_.repairTimeDelta = _loc2_.repairTimeDelta;
         _loc1_.camoAvailable = _loc2_.camoAvailable;
         _loc1_.decalAvailable = _loc2_.decalAvailable;
         _loc1_.upgradesAvailable = _loc2_.upgradesAvailable;
         _loc1_.isWeapon = _loc2_.isWeapon;
         _loc1_.usable = _loc2_.usable;
         _loc1_.showed = _loc2_.showed;
         _loc1_.isNew = _loc2_.isNew;
         _loc1_.enabled = _loc2_.enabled;
         _loc1_.isStack = _loc2_.isStack;
         _loc1_.oneHanded = _loc2_.oneHanded;
         _loc1_.buyable = _loc2_.buyable;
         _loc1_.buyValue = _loc2_.buyValue;
         _loc1_.canBuy = _loc2_.canBuy;
         _loc1_.equipped = _loc2_.equipped;
         _loc1_.isPremium = _loc2_.isPremium;
         _loc1_.name = _loc2_.name;
         _loc1_.itemDescription = _loc2_.itemDescription;
         _loc1_.modifiersDescription = _loc2_.modifiersDescription;
         _loc1_.tipText = _loc2_.tipText;
         _loc1_.lockText = _loc2_.lockText;
         _loc1_.unlockLevelLabel = _loc2_.unlockLevelLabel;
         _loc1_.modifiers = _loc2_.modifiers;
         _loc1_.modifiersDefault = _loc2_.modifiersDefault;
         _loc1_.properties = _loc2_.properties;
         _loc1_.sellers = _loc2_.sellers;
         _loc1_.broken = _loc2_.broken;
         _loc1_.equipped = _loc2_.equipped;
         _loc1_.fromBundle = _loc2_.fromBundle;
         _loc1_.additionalSlots = _loc2_.additionalSlots;
         return _loc1_;
      }
   }
}

