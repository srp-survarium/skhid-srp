package survarium.controls
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.ListEvent;
   import survarium.data.LocalizationData;
   import survarium.constants.ShopFilter;
   import survarium.constants.WeaponHands;
   
   public class OptionFilterControl extends Sprite
   {
      
      public var checkbox_all_traders:Button;
      
      public var selectAll:Button;
      
      public var filterList:FilterList;
      
      public var sortTab:ButtonBar;
      
      public var textField:TextField;
      
      protected var _checkbox_enabled_items:Button;
      
      protected var _mask_step:int = 160;
      
      protected var sortTypes:Array = ["itemLevel","price"];
      
      protected var _startPosX:int;
      
      protected var _startPosY:int;
      
      protected var _startX:int;
      
      protected var _startY:int;
      
      protected var _shopTab:ButtonBar;
      
      protected var _checkbox_shop_items:Button;

      // The row's width, and the room at its right end kept for the shop's owned toggle
      // (ShopOwnedToggle, placed by InventoryList): the filters are centred in the rest.
      public static const ROW_WIDTH:int = 573;

      public static const TOGGLE_ROOM:int = 40;

      public function OptionFilterControl()
      {
         super();
         _startPosX = x;
         _startPosY = this.y;
         _startX = x;
         _startY = this.y;
         this.filterList.addEventListener(FilterList.NEW_DATA_PROVIDER,this.onNewFilters);
         this.filterList.addEventListener(ListEvent.INDEX_CHANGE,this.checkAllButton);
         LocalizationData.addCallback(this.onLocal,"sort_level");
      }
      
      public function getSelectedIndexes() : Array
      {
         return this.filterList.getSelectedIndexes();
      }
      
      override public function get y() : Number
      {
         return super.y;
      }
      
      override public function set y(value:Number) : void
      {
         super.y = value;
         dispatchEvent(new Event(Event.RESIZE));
      }
      
      public function set shopTab(value:ButtonBar) : void
      {
         _shopTab = value;
         this._shopTab.selectedIndex = ShopFilter.SHOP_AND_INVENTORY;
         this._shopTab.addEventListener(IndexEvent.INDEX_CHANGE,this.onSortTypeChange);
      }
      
      public function set checkbox_enabled_items(value:Button) : void
      {
         _checkbox_enabled_items = value;
         this._checkbox_enabled_items.addEventListener(ButtonEvent.CLICK,this.onBtnClick);
      }
      
      public function get checkbox_enabled_items() : Button
      {
         return this._checkbox_enabled_items;
      }
      
      public function set checkbox_shop_items(value:Button) : void
      {
         _checkbox_shop_items = value;
         this._checkbox_shop_items.addEventListener(ButtonEvent.CLICK,this.onBtnClick);
      }
      
      public function get checkbox_shop_items() : Button
      {
         return this._checkbox_shop_items;
      }
      
      private function updatePos() : void
      {
         _startX = int(this._startPosX * MainScene.scaleFactor) / MainScene.scaleFactor;
         _startY = int(this._startPosY * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      private function onNewFilters(event:Event) : void
      {
         var filterCount:int;
         var cellWidth:uint;
         var listWidth:uint;
         trace("newFilters");
         if(this.filterList.dataProvider == null || this.filterList.dataProvider.length == 0)
         {
            if(this.enabled)
            {
               this.hide();
            }
            this.filterList.visible = false;
            this.selectAll.visible = false;
         }
         else
         {
            this.filterList.visible = true;
            this.selectAll.visible = true;
            filterCount = this.filterList.dataProvider.length;
            cellWidth = 64;
            listWidth = filterCount * cellWidth;
            this.filterList.width = listWidth;
            // Centred in the row but its right end, the shop's owned toggle's (InventoryList).
            this.filterList.x = int((ROW_WIDTH - TOGGLE_ROOM - (listWidth + cellWidth)) / 2 + cellWidth);
            trace("filterspos:",filterCount,this.filterList.x);
            this.selectAll.x = this.filterList.x - cellWidth;
            this.checkAllButton();
         }
      }
      
      private function checkAllButton(event:Event = null) : void
      {
         var selectedCount:uint = this.filterList.getSelectedIndexes().length;
         trace("checkAllButton",selectedCount);
         this.selectAll.selected = selectedCount == 0 || selectedCount == this.filterList.dataProvider.length;
      }
      
      private function get maxOffset() : int
      {
         var offset:int = (this.filterList.y - 16);
         return offset;
      }
      
      public function get enabled() : Boolean
      {
         return true;
      }
      
      public function getFilterOptions() : Object
      {
         return {
            weapons:WeaponHands.ALL,
            sortFirst:(this.sortTab.selectedIndex == 0 ? this.sortTypes[0] : this.sortTypes[1]),
            sortSecond:(this.sortTab.selectedIndex == 0 ? this.sortTypes[1] : this.sortTypes[0]),
            onlyEnabled:false,
            allTraders:this.checkbox_all_traders.selected,
            shopItems:this._shopTab.selectedIndex
         };
      }
      
      public function showWeaponsSort(show:Boolean) : void
      {
      }
      
      public function setSortTab(tabBar:ButtonBar) : void
      {
         if(!tabBar)
         {
            return;
         }
         sortTab = tabBar;
         this.sortTab.selectedIndex = 0;
         this.sortTab.addEventListener(IndexEvent.INDEX_CHANGE,this.onSortTypeChange);
      }
      
      public function setShowBtn(button:Button) : void
      {
         if(!button)
         {
            return;
         }
      }
      
      public function setCheckBox(checkbox:Button) : void
      {
         checkbox_all_traders = checkbox;
         this.checkbox_all_traders.addEventListener(ButtonEvent.CLICK,this.onBtnClick);
         LocalizationData.addButton(this.checkbox_all_traders,"all_factions");
      }
      
      public function selectCategoty(subCategory:int) : void
      {
         var i:uint;
         while(i < this.filterList.dataProvider.length)
         {
            if(this.filterList.dataProvider[i].sucat == subCategory)
            {
               this.filterList.selectedIndex = i;
               return;
            }
            i++;
         }
      }
      
      public function show() : void
      {
         this.updatePos();
      }
      
      public function hide() : void
      {
         trace("hide",this._mask_step);
      }
      
      public function onBtnSelectAllClick(event:Event) : void
      {
         trace("onBtnSelectAllClick");
         this.filterList.deselectAll();
         this.selectAll.selected = true;
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE));
      }
      
      public function reset(resetTraders:Boolean = true) : void
      {
         this.filterList.deselectAll();
         if(resetTraders)
         {
            this.checkbox_all_traders.selected = true;
         }
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE));
      }
      
      protected function onSortTypeChange(event:Event) : void
      {
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE));
      }
      
      protected function onBtnClick(event:Event) : void
      {
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE));
      }
      
      protected function onLocal(value:String) : void
      {
         if(this.sortTab)
         {
            this.sortTab.dataProvider = new DataProvider([LocalizationData.getName("sort_level"),LocalizationData.getName("sort_cost")]);
         }
         if(this.sortTab)
         {
            this.sortTab.selectedIndex = 0;
         }
         this.selectAll.data = {
            icon:17,
            name:"all",
            tip:LocalizationData.getName("filter_all"),
            sucat:-1,
            selected:false
         };
         this.selectAll.enabled = true;
         this.selectAll.addEventListener(ButtonEvent.CLICK,this.onBtnSelectAllClick);
         this.filterList.columnWidth = 64;
      }
      
      protected function onBtnResetClick(event:Event) : void
      {
         this.reset();
      }
      
      protected function onBtnShowClick(event:Event) : void
      {
         if(this.enabled)
         {
            this.hide();
         }
         else
         {
            this.show();
         }
      }
   }
}

