package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.ScrollBar;
   import scaleform.clik.controls.TileListSmoothR;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.interfaces.IListItemRenderer;
   import survarium.data.ItemDescriptor;
   import survarium.data.LocalizationData;
   import survarium.constants.Cursor;
   import survarium.constants.ItemCategory;
   import survarium.constants.SuperCategory;
   
   public class FiltersWindow extends UIWindow
   {
      
      public var optionControl:OptionFilterControl;
      
      public var inventoryTabPage:ButtonBar;
      
      // the filter lists of the shop's tabs (optionControl.filterList: a tab's SuperCategory - 1)
      private static const EQUIPMENT_FILTERS:int = 0;
      
      private static const WEAPON_FILTERS:int = 1;
      
      private static const AMMO_FILTERS:int = 2;
      
      private static const ITEM_FILTERS:int = 3;
      
      private static const UPGRADE_FILTERS:int = 4;
      
      // the "all" tab's (empty) list
      private static const NO_FILTERS:int = 5;
      
      public var mainTabBar:ButtonBar = new ButtonBar();
      
      public var sellersTabPage:ButtonBar;
      
      public var itemsTileList:TileListSmoothR;
      
      public var background:MovieClip;
      
      public var black_gradient:Sprite;
      
      public var label:TextField;
      
      public var scrollBar:ScrollBar;
      
      protected var _itemDescriptor:ItemDescriptor;
      
      private var _dragListener:Function;
      
      private var lastCategory:int = 0;
      
      private var lastSelectedIndex:int = 0;
      
      private var lastFilterIndexes:Array;
      
      public function FiltersWindow()
      {
         super();
         addEventListener(Event.RENDER,this.init);
      }
      
      override protected function configUI() : void
      {
         removeEventListener(Event.RENDER,this.configUI);
         LocalizationData.addCallback(this.onLocal,"filter_body_armor");
         this.itemsTileList.addEventListener(DragEvent.DRAG_START,this.dragCallback);
         this.itemsTileList.addEventListener(DragEvent.DRAG_END,this.dragCallback);
         this.itemsTileList.addEventListener(ListEvent.INDEX_CHANGE,this.onNewIndex);
         this.black_gradient.mouseEnabled = false;
         addEventListener(Event.RENDER,this.onStart);
      }
      
      public function forEachItem(callback:Function) : void
      {
      }
      
      public function fillSellers(sellers:Array) : void
      {
      }
      
      public function registerItemDescriptor(descriptor:ItemDescriptor) : void
      {
         _itemDescriptor = descriptor;
      }
      
      public function registerDragListener(listener:*) : void
      {
         _dragListener = listener;
      }
      
      public function focusCategory(category:int) : void
      {
         trace("focusCategory::",category,this.lastSelectedIndex);
         this.inventoryTabPage.dataProvider = new DataProvider([{
            name:LocalizationData.getName("filter_all"),
            icon:1,
            index:1,
            enabled:SuperCategory.ALL == category || category == SuperCategory.NO_FOCUS
         },{
            name:LocalizationData.getName("filter_equipment"),
            icon:2,
            index:2,
            enabled:SuperCategory.EQUIPMENT == category || category == SuperCategory.NO_FOCUS
         },{
            name:LocalizationData.getName("filter_weapons"),
            icon:3,
            index:4,
            enabled:SuperCategory.WEAPONS == category || category == SuperCategory.NO_FOCUS
         },{
            name:LocalizationData.getName("filter_ammo"),
            icon:4,
            index:5,
            enabled:SuperCategory.AMMO == category || category == SuperCategory.NO_FOCUS
         },{
            name:LocalizationData.getName("filter_items"),
            icon:5,
            index:3,
            enabled:SuperCategory.ITEMS == category || category == SuperCategory.NO_FOCUS
         },{
            name:LocalizationData.getName("filter_upgrades"),
            icon:6,
            index:6,
            enabled:SuperCategory.UPGRADES == category || category == SuperCategory.NO_FOCUS
         }]);
         if(category != SuperCategory.NO_FOCUS)
         {
            lastCategory = this.inventoryTabPage.selectedIndex;
            lastSelectedIndex = this.itemsTileList.selectedIndex;
            lastFilterIndexes = this.optionControl.filterList.getSelectedPositions();
            this.inventoryTabPage.selectedIndex = category;
            trace("getLastSelectedIndex",this.itemsTileList.selectedIndex,this.lastFilterIndexes);
         }
         else
         {
            this.inventoryTabPage.selectedIndex = this.lastCategory;
            this.itemsTileList.selectedIndex = this.lastSelectedIndex;
            trace("FC_selectPart::",this.itemsTileList.selectedIndex,"|",this.lastFilterIndexes);
            if(this.lastFilterIndexes && this.lastCategory > SuperCategory.ALL)
            {
               this.optionControl.filterList.selectPart(this.lastFilterIndexes,this.lastCategory - 1);
            }
         }
      }
      
      private function onMouseLeave(event:Event) : void
      {
         ExternalInterface.call("set_mouse_cursor",Cursor.ARROW);
      }
      
      private function onLocal(localizedText:String) : void
      {
         this.init(null);
      }
      
      private function init(event:Event) : void
      {
         trace("FW::",this.optionControl,this.inventoryTabPage,this.itemsTileList);
         removeEventListener(Event.RENDER,this.init);
         this.optionControl.filterList.registerDataProvider(EQUIPMENT_FILTERS,new DataProvider([{
            "icon":3,
            "name":"bronya",
            "tip":LocalizationData.getName("filter_body_armor"),
            "sucat":ItemCategory.BODY_ARMOR,
            "selected":false
         },{
            "icon":1,
            "name":"kaska",
            "tip":LocalizationData.getName("filter_helmet"),
            "sucat":ItemCategory.HELMETS,
            "selected":false
         },{
            "icon":6,
            "name":"perchatka",
            "tip":LocalizationData.getName("filter_gloves"),
            "sucat":ItemCategory.GLOVES,
            "selected":false
         },{
            "icon":7,
            "name":"obuv",
            "tip":LocalizationData.getName("filter_boots"),
            "sucat":ItemCategory.BOOTS,
            "selected":false
         },{
            "icon":5,
            "name":"nogi",
            "tip":LocalizationData.getName("filter_pants"),
            "sucat":ItemCategory.PANTS,
            "selected":false
         },{
            "icon":2,
            "name":"maska",
            "tip":LocalizationData.getName("filter_mask"),
            "sucat":ItemCategory.MASKS,
            "selected":false
         },{
            "icon":0,
            "name":"snaryazhenie",
            "tip":LocalizationData.getName("filter_rig"),
            "sucat":ItemCategory.RIGS,
            "selected":false
         }]));
         this.optionControl.filterList.registerDataProvider(WEAPON_FILTERS,new DataProvider([{
            "icon":8,
            "name":"pistolet",
            "tip":LocalizationData.getName("filter_pistol"),
            "sucat":ItemCategory.PISTOLS,
            "selected":false
         },{
            "icon":9,
            "name":"gungun",
            "tip":LocalizationData.getName("filter_submachine_gun"),
            "sucat":ItemCategory.SUBMACHINE_GUNS,
            "selected":false
         },{
            "icon":10,
            "name":"avtomat",
            "tip":LocalizationData.getName("filter_assault_rifle"),
            "sucat":ItemCategory.ASSAULT_RIFLES,
            "selected":false
         },{
            "icon":11,
            "name":"pulemet",
            "tip":LocalizationData.getName("filter_machine_gun"),
            "sucat":ItemCategory.MACHINE_GUNS,
            "selected":false
         },{
            "icon":12,
            "name":"drobovik",
            "tip":LocalizationData.getName("filter_shotgun"),
            "sucat":ItemCategory.SHOTGUNS,
            "selected":false
         },{
            "icon":13,
            "name":"snaiperka",
            "tip":LocalizationData.getName("filter_sniper_rifle"),
            "sucat":ItemCategory.SNIPER_RIFLES,
            "selected":false
         }]));
         this.optionControl.filterList.registerDataProvider(AMMO_FILTERS,new DataProvider([{
            "icon":8,
            "name":"ammo_1",
            "tip":LocalizationData.getName("filter_ammo_pistol"),
            "sucat":ItemCategory.PISTOL_AMMO,
            "selected":false
         },{
            "icon":10,
            "name":"ammo_3",
            "tip":LocalizationData.getName("filter_ammo_assault_rifle"),
            "sucat":ItemCategory.ASSAULT_RIFLE_AMMO,
            "selected":false
         },{
            "icon":11,
            "name":"ammo_4",
            "tip":LocalizationData.getName("filter_ammo_machine_gun"),
            "sucat":ItemCategory.MACHINE_GUN_AMMO,
            "selected":false
         },{
            "icon":12,
            "name":"ammo_5",
            "tip":LocalizationData.getName("filter_ammo_shotgun"),
            "sucat":ItemCategory.SHOTGUN_AMMO,
            "selected":false
         },{
            "icon":13,
            "name":"ammo_6",
            "tip":LocalizationData.getName("filter_ammo_sniper_rifle"),
            "sucat":ItemCategory.SNIPER_RIFLE_AMMO,
            "selected":false
         }]));
         this.optionControl.filterList.registerDataProvider(ITEM_FILTERS,new DataProvider([{
            "icon":14,
            "name":"medkit",
            "tip":LocalizationData.getName("filter_medkit"),
            "sucat":ItemCategory.MEDKITS,
            "selected":false
         },{
            "icon":15,
            "name":"traps",
            "tip":LocalizationData.getName("filter_mine"),
            "sucat":ItemCategory.BOOBY_TRAPS,
            "selected":false
         },{
            "icon":16,
            "name":"grenade",
            "tip":LocalizationData.getName("filter_grenade"),
            "sucat":ItemCategory.GRENADES,
            "selected":false
         }]));
         this.optionControl.filterList.registerDataProvider(UPGRADE_FILTERS,new DataProvider([{
            "icon":18,
            "name":"decal",
            "tip":LocalizationData.getName("filter_decal"),
            "sucat":ItemCategory.DECALS,
            "selected":false
         },{
            "icon":19,
            "name":"camo",
            "tip":LocalizationData.getName("filter_camo"),
            "sucat":ItemCategory.CAMOUFLAGES,
            "selected":false
         },{
            "icon":20,
            "name":"vouchers",
            "tip":LocalizationData.getName("filter_vouchers"),
            "sucat":ItemCategory.VOUCHERS,
            "selected":false
         },{
            "icon":13,
            "name":"scope",
            "tip":LocalizationData.getName("filter_scope"),
            "sucat":ItemCategory.SCOPES,
            "selected":false
         }]));
         this.optionControl.filterList.registerDataProvider(NO_FILTERS,new DataProvider([]));
         if(Boolean(this.mainTabBar))
         {
            this.mainTabBar.dataProvider = new DataProvider([{"name":LocalizationData.getName("shop_label")},{"name":LocalizationData.getName("inventory_label")}]);
         }
         trace("FW::2");
         this.inventoryTabPage.dataProvider = new DataProvider([{
            "name":LocalizationData.getName("filter_all"),
            "icon":1,
            "index":1
         },{
            "name":LocalizationData.getName("filter_equipment"),
            "icon":2,
            "index":2
         },{
            "name":LocalizationData.getName("filter_weapons"),
            "icon":3,
            "index":4
         },{
            "name":LocalizationData.getName("filter_ammo"),
            "icon":4,
            "index":5
         },{
            "name":LocalizationData.getName("filter_items"),
            "icon":5,
            "index":3
         },{
            "name":LocalizationData.getName("filter_upgrades"),
            "icon":6,
            "index":6
         }]);
         this.optionControl.filterList.setDataProvider(NO_FILTERS);
         this.inventoryTabPage.selectedIndex = SuperCategory.ALL;
         this.itemsTileList.selectedIndex = -1;
      }
      
      protected function hideME(event:Event) : void
      {
         visible = false;
      }
      
      protected function onStart(event:Event) : void
      {
         removeEventListener(Event.RENDER,this.onStart);
         this.configureInventoryTab();
      }
      
      protected function configureInventoryTab() : void
      {
         this.inventoryTabPage.selectedIndex = SuperCategory.ALL;
         this.sellersTabPage.selectedIndex = 0;
         this.inventoryTabPage.addEventListener(IndexEvent.INDEX_CHANGE,this.changeInventoryFilter);
         this.optionControl.addEventListener(ListEvent.INDEX_CHANGE,this.changeInventoryFilterFromFilters);
      }
      
      protected function changeInventoryFilter(event:Event) : void
      {
         trace("updateCurrentDatProvider");
         if(this.inventoryTabPage.selectedIndex == SuperCategory.EQUIPMENT)
         {
            this.optionControl.filterList.setDataProvider(EQUIPMENT_FILTERS);
         }
         else if(this.inventoryTabPage.selectedIndex == SuperCategory.WEAPONS)
         {
            this.optionControl.filterList.setDataProvider(WEAPON_FILTERS);
         }
         else if(this.inventoryTabPage.selectedIndex == SuperCategory.AMMO)
         {
            this.optionControl.filterList.setDataProvider(AMMO_FILTERS);
         }
         else if(this.inventoryTabPage.selectedIndex == SuperCategory.ITEMS)
         {
            this.optionControl.filterList.setDataProvider(ITEM_FILTERS);
         }
         else if(this.inventoryTabPage.selectedIndex == SuperCategory.UPGRADES)
         {
            this.optionControl.filterList.setDataProvider(UPGRADE_FILTERS);
         }
         else
         {
            this.optionControl.filterList.setDataProvider(NO_FILTERS);
         }
         if(this.optionControl.filterList)
         {
            this.optionControl.filterList.invalidate(InvalidationType.SIZE);
         }
         trace("FW:changeInventoryFilter");
         this.itemsTileList.selectedIndex = -1;
         this.itemsTileList.scrollBar.position = 0;
      }
      
      protected function dragCallback(event:DragEvent = null) : void
      {
         if(event && event.type == DragEvent.DRAG_START)
         {
            trace("FW:dragCallback");
            this.itemsTileList.selectedIndex = (event.dragTarget as IListItemRenderer).index;
         }
         if(this._dragListener is Function)
         {
            (this._dragListener as Function)(event);
         }
      }
      
      protected function changeInventoryFilterFromFilters(event:Event) : void
      {
      }
      
      protected function onNewIndex(event:ListEvent) : void
      {
      }
   }
}

