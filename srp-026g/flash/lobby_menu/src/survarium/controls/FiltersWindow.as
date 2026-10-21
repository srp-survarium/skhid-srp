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
   
   public class FiltersWindow extends UIWindow
   {
      
      public static const CAT_VOUCHER:uint = 27;
      
      public static const CAT_CAMO:uint = 25;
      
      public var optionControl:OptionFilterControl;
      
      public var inventoryTabPage:ButtonBar;
      
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
      
      public function forEachItem(param1:Function) : void
      {
      }
      
      public function fillSellers(param1:Array) : void
      {
      }
      
      public function registerItemDescriptor(param1:ItemDescriptor) : void
      {
         _itemDescriptor = param1;
      }
      
      public function registerDragListener(param1:*) : void
      {
         _dragListener = param1;
      }
      
      public function focusCategory(param1:int) : void
      {
         trace("focusCategory::",param1,this.lastSelectedIndex);
         this.inventoryTabPage.dataProvider = new DataProvider([{
            name:LocalizationData.getName("filter_all"),
            icon:1,
            index:1,
            enabled:0 == param1 || param1 == -1
         },{
            name:LocalizationData.getName("filter_equipment"),
            icon:2,
            index:2,
            enabled:1 == param1 || param1 == -1
         },{
            name:LocalizationData.getName("filter_weapons"),
            icon:3,
            index:4,
            enabled:2 == param1 || param1 == -1
         },{
            name:LocalizationData.getName("filter_ammo"),
            icon:4,
            index:5,
            enabled:3 == param1 || param1 == -1
         },{
            name:LocalizationData.getName("filter_items"),
            icon:5,
            index:3,
            enabled:4 == param1 || param1 == -1
         },{
            name:LocalizationData.getName("filter_upgrades"),
            icon:6,
            index:6,
            enabled:5 == param1 || param1 == -1
         }]);
         if(param1 != -1)
         {
            lastCategory = this.inventoryTabPage.selectedIndex;
            lastSelectedIndex = this.itemsTileList.selectedIndex;
            lastFilterIndexes = this.optionControl.filterList.getSelectedPositions();
            this.inventoryTabPage.selectedIndex = param1;
            trace("getLastSelectedIndex",this.itemsTileList.selectedIndex,this.lastFilterIndexes);
         }
         else
         {
            this.inventoryTabPage.selectedIndex = this.lastCategory;
            this.itemsTileList.selectedIndex = this.lastSelectedIndex;
            trace("FC_selectPart::",this.itemsTileList.selectedIndex,"|",this.lastFilterIndexes);
            if(this.lastFilterIndexes && this.lastCategory > 0)
            {
               this.optionControl.filterList.selectPart(this.lastFilterIndexes,this.lastCategory - 1);
            }
         }
      }
      
      private function onMouseLeave(param1:Event) : void
      {
         ExternalInterface.call("set_mouse_cursor",1);
      }
      
      private function onLocal(param1:String) : void
      {
         this.init(null);
      }
      
      private function init(param1:Event) : void
      {
         trace("FW::",this.optionControl,this.inventoryTabPage,this.itemsTileList);
         removeEventListener(Event.RENDER,this.init);
         this.optionControl.filterList.registerDataProvider(0,new DataProvider([{
            "icon":3,
            "name":"bronya",
            "tip":LocalizationData.getName("filter_body_armor"),
            "sucat":3,
            "selected":false
         },{
            "icon":1,
            "name":"kaska",
            "tip":LocalizationData.getName("filter_helmet"),
            "sucat":1,
            "selected":false
         },{
            "icon":6,
            "name":"perchatka",
            "tip":LocalizationData.getName("filter_gloves"),
            "sucat":6,
            "selected":false
         },{
            "icon":7,
            "name":"obuv",
            "tip":LocalizationData.getName("filter_boots"),
            "sucat":7,
            "selected":false
         },{
            "icon":5,
            "name":"nogi",
            "tip":LocalizationData.getName("filter_pants"),
            "sucat":5,
            "selected":false
         },{
            "icon":2,
            "name":"maska",
            "tip":LocalizationData.getName("filter_mask"),
            "sucat":2,
            "selected":false
         },{
            "icon":0,
            "name":"snaryazhenie",
            "tip":LocalizationData.getName("filter_rig"),
            "sucat":4,
            "selected":false
         }]));
         this.optionControl.filterList.registerDataProvider(1,new DataProvider([{
            "icon":8,
            "name":"pistolet",
            "tip":LocalizationData.getName("filter_pistol"),
            "sucat":17,
            "selected":false
         },{
            "icon":9,
            "name":"gungun",
            "tip":LocalizationData.getName("filter_submachine_gun"),
            "sucat":15,
            "selected":false
         },{
            "icon":10,
            "name":"avtomat",
            "tip":LocalizationData.getName("filter_assault_rifle"),
            "sucat":8,
            "selected":false
         },{
            "icon":11,
            "name":"pulemet",
            "tip":LocalizationData.getName("filter_machine_gun"),
            "sucat":16,
            "selected":false
         },{
            "icon":12,
            "name":"drobovik",
            "tip":LocalizationData.getName("filter_shotgun"),
            "sucat":14,
            "selected":false
         },{
            "icon":13,
            "name":"snaiperka",
            "tip":LocalizationData.getName("filter_sniper_rifle"),
            "sucat":13,
            "selected":false
         }]));
         this.optionControl.filterList.registerDataProvider(2,new DataProvider([{
            "icon":8,
            "name":"ammo_1",
            "tip":LocalizationData.getName("filter_ammo_pistol"),
            "sucat":20,
            "selected":false
         },{
            "icon":10,
            "name":"ammo_3",
            "tip":LocalizationData.getName("filter_ammo_assault_rifle"),
            "sucat":9,
            "selected":false
         },{
            "icon":11,
            "name":"ammo_4",
            "tip":LocalizationData.getName("filter_ammo_machine_gun"),
            "sucat":21,
            "selected":false
         },{
            "icon":12,
            "name":"ammo_5",
            "tip":LocalizationData.getName("filter_ammo_shotgun"),
            "sucat":19,
            "selected":false
         },{
            "icon":13,
            "name":"ammo_6",
            "tip":LocalizationData.getName("filter_ammo_sniper_rifle"),
            "sucat":18,
            "selected":false
         }]));
         this.optionControl.filterList.registerDataProvider(3,new DataProvider([{
            "icon":14,
            "name":"medkit",
            "tip":LocalizationData.getName("filter_medkit"),
            "sucat":10,
            "selected":false
         },{
            "icon":15,
            "name":"traps",
            "tip":LocalizationData.getName("filter_mine"),
            "sucat":11,
            "selected":false
         },{
            "icon":16,
            "name":"grenade",
            "tip":LocalizationData.getName("filter_grenade"),
            "sucat":24,
            "selected":false
         }]));
         this.optionControl.filterList.registerDataProvider(4,new DataProvider([{
            "icon":18,
            "name":"decal",
            "tip":LocalizationData.getName("filter_decal"),
            "sucat":26,
            "selected":false
         },{
            "icon":19,
            "name":"camo",
            "tip":LocalizationData.getName("filter_camo"),
            "sucat":25,
            "selected":false
         },{
            "icon":20,
            "name":"vouchers",
            "tip":LocalizationData.getName("filter_vouchers"),
            "sucat":27,
            "selected":false
         },{
            "icon":13,
            "name":"scope",
            "tip":LocalizationData.getName("filter_scope"),
            "sucat":22,
            "selected":false
         }]));
         this.optionControl.filterList.registerDataProvider(5,new DataProvider([]));
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
         this.optionControl.filterList.setDataProvider(5);
         this.inventoryTabPage.selectedIndex = 0;
         this.itemsTileList.selectedIndex = -1;
      }
      
      protected function hideME(param1:Event) : void
      {
         visible = false;
      }
      
      protected function onStart(param1:Event) : void
      {
         removeEventListener(Event.RENDER,this.onStart);
         this.configureInventoryTab();
      }
      
      protected function configureInventoryTab() : void
      {
         this.inventoryTabPage.selectedIndex = 0;
         this.sellersTabPage.selectedIndex = 0;
         this.inventoryTabPage.addEventListener(IndexEvent.INDEX_CHANGE,this.changeInventoryFilter);
         this.optionControl.addEventListener(ListEvent.INDEX_CHANGE,this.changeInventoryFilterFromFilters);
      }
      
      protected function changeInventoryFilter(param1:Event) : void
      {
         trace("updateCurrentDatProvider");
         if(this.inventoryTabPage.selectedIndex == 1)
         {
            this.optionControl.filterList.setDataProvider(0);
         }
         else if(this.inventoryTabPage.selectedIndex == 2)
         {
            this.optionControl.filterList.setDataProvider(1);
         }
         else if(this.inventoryTabPage.selectedIndex == 3)
         {
            this.optionControl.filterList.setDataProvider(2);
         }
         else if(this.inventoryTabPage.selectedIndex == 4)
         {
            this.optionControl.filterList.setDataProvider(3);
         }
         else if(this.inventoryTabPage.selectedIndex == 5)
         {
            this.optionControl.filterList.setDataProvider(4);
         }
         else
         {
            this.optionControl.filterList.setDataProvider(5);
         }
         if(this.optionControl.filterList)
         {
            this.optionControl.filterList.invalidate(InvalidationType.SIZE);
         }
         trace("FW:changeInventoryFilter");
         this.itemsTileList.selectedIndex = -1;
         this.itemsTileList.scrollBar.position = 0;
      }
      
      protected function dragCallback(param1:DragEvent = null) : void
      {
         if(param1 && param1.type == DragEvent.DRAG_START)
         {
            trace("FW:dragCallback");
            this.itemsTileList.selectedIndex = (param1.dragTarget as IListItemRenderer).index;
         }
         if(this._dragListener is Function)
         {
            (this._dragListener as Function)(param1);
         }
      }
      
      protected function changeInventoryFilterFromFilters(param1:Event) : void
      {
      }
      
      protected function onNewIndex(param1:ListEvent) : void
      {
      }
   }
}

