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
      
      override public function set y(param1:Number) : void
      {
         super.y = param1;
         dispatchEvent(new Event(Event.RESIZE));
      }
      
      public function set shopTab(param1:ButtonBar) : void
      {
         _shopTab = param1;
         this._shopTab.selectedIndex = 0;
         this._shopTab.addEventListener(IndexEvent.INDEX_CHANGE,this.onSortTypeChange);
      }
      
      public function set checkbox_enabled_items(param1:Button) : void
      {
         _checkbox_enabled_items = param1;
         this._checkbox_enabled_items.addEventListener(ButtonEvent.CLICK,this.onBtnClick);
      }
      
      public function get checkbox_enabled_items() : Button
      {
         return this._checkbox_enabled_items;
      }
      
      public function set checkbox_shop_items(param1:Button) : void
      {
         _checkbox_shop_items = param1;
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
      
      private function onNewFilters(param1:Event) : void
      {
         var _loc2_:int;
         var _loc3_:uint;
         var _loc4_:uint;
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
            _loc2_ = this.filterList.dataProvider.length;
            _loc3_ = 64;
            _loc4_ = _loc2_ * _loc3_;
            this.filterList.width = _loc4_;
            this.filterList.x = int((573 - (_loc4_ + _loc3_)) / 2 + _loc3_);
            trace("filterspos:",_loc2_,this.filterList.x);
            this.selectAll.x = this.filterList.x - _loc3_;
            this.checkAllButton();
         }
      }
      
      private function checkAllButton(param1:Event = null) : void
      {
         var _loc2_:uint = this.filterList.getSelectedIndexes().length;
         trace("checkAllButton",_loc2_);
         this.selectAll.selected = _loc2_ == 0 || _loc2_ == this.filterList.dataProvider.length;
      }
      
      private function get maxOffset() : int
      {
         var _loc1_:int = (this.filterList.y - 16);
         return _loc1_;
      }
      
      public function get enabled() : Boolean
      {
         return true;
      }
      
      public function getFilterOptions() : Object
      {
         return {
            weapons:2,
            sortFirst:(this.sortTab.selectedIndex == 0 ? this.sortTypes[0] : this.sortTypes[1]),
            sortSecond:(this.sortTab.selectedIndex == 0 ? this.sortTypes[1] : this.sortTypes[0]),
            onlyEnabled:false,
            allTraders:this.checkbox_all_traders.selected,
            shopItems:this._shopTab.selectedIndex
         };
      }
      
      public function showWeaponsSort(param1:Boolean) : void
      {
      }
      
      public function setSortTab(param1:ButtonBar) : void
      {
         if(!param1)
         {
            return;
         }
         sortTab = param1;
         this.sortTab.selectedIndex = 0;
         this.sortTab.addEventListener(IndexEvent.INDEX_CHANGE,this.onSortTypeChange);
      }
      
      public function setShowBtn(param1:Button) : void
      {
         if(!param1)
         {
            return;
         }
      }
      
      public function setCheckBox(param1:Button) : void
      {
         checkbox_all_traders = param1;
         this.checkbox_all_traders.addEventListener(ButtonEvent.CLICK,this.onBtnClick);
         LocalizationData.addButton(this.checkbox_all_traders,"all_factions");
      }
      
      public function selectCategoty(param1:int) : void
      {
         var _loc2_:uint;
         while(_loc2_ < this.filterList.dataProvider.length)
         {
            if(this.filterList.dataProvider[_loc2_].sucat == param1)
            {
               this.filterList.selectedIndex = _loc2_;
               return;
            }
            _loc2_++;
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
      
      public function onBtnSelectAllClick(param1:Event) : void
      {
         trace("onBtnSelectAllClick");
         this.filterList.deselectAll();
         this.selectAll.selected = true;
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE));
      }
      
      public function reset(param1:Boolean = true) : void
      {
         this.filterList.deselectAll();
         if(param1)
         {
            this.checkbox_all_traders.selected = true;
         }
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE));
      }
      
      protected function onSortTypeChange(param1:Event) : void
      {
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE));
      }
      
      protected function onBtnClick(param1:Event) : void
      {
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE));
      }
      
      protected function onLocal(param1:String) : void
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
      
      protected function onBtnResetClick(param1:Event) : void
      {
         this.reset();
      }
      
      protected function onBtnShowClick(param1:Event) : void
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

