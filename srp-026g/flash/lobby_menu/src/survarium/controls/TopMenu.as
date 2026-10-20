package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import survarium.data.LocalizationData;
   
   public class TopMenu extends UIComponent
   {
      
      public var tabPage:ButtonBar;
      
      public const CHAR_INDEX:uint = 0;
      
      public const SHOP_INDEX:uint = 2;
      
      public const INV_INDEX:uint = 1;
      
      public const CLAN_INDEX:uint = 3;
      
      public const QUESTS_INDEX:uint = 6;
      
      public const RANK_INDEX:uint = 5;
      
      public const MAP_INDEX:uint = 4;
      
      protected var sceneContainer:MovieClip;
      
      protected var currentUI:UIWindow;
      
      protected var locked:Boolean = false;
      
      protected var _byUser:Boolean = true;
      
      protected var _questButton:Button;
      
      public function TopMenu()
      {
         super();
         LocalizationData.addCallback(this.fill_windows,"character");
      }
      
      public function get inventoryTab() : MovieClip
      {
         return this.getRenderer(this.INV_INDEX);
      }
      
      public function get questsTab() : MovieClip
      {
         return this.getRenderer(this.QUESTS_INDEX);
      }
      
      public function get skillsTab() : MovieClip
      {
         return this.getRenderer(this.CHAR_INDEX);
      }
      
      public function set questButton(param1:Button) : void
      {
         _questButton = param1;
         this._questButton.toggle = true;
         this._questButton.addEventListener(ButtonEvent.CLICK,this.showQuests);
      }
      
      public function get questButton() : Button
      {
         return this._questButton;
      }
      
      private function showQuests(param1:ButtonEvent) : void
      {
         trace("TOP::showQuests");
         if(this.getActiveWindow() == this.QUESTS_INDEX)
         {
            this.selectActiveWindow(-1);
         }
         else
         {
            this.selectActiveWindow(this.QUESTS_INDEX);
         }
      }
      
      private function setByUser(param1:Boolean) : void
      {
         trace("setByUser",param1);
         _byUser = param1;
         stackTrace("_affectByUser");
      }
      
      public function selectActiveWindow(param1:int, param2:Boolean = true) : void
      {
         trace("selectActiveWindow::",param1,param2);
         if(param1 != -1 && this.tabPage.selectedIndex != param1)
         {
            this.setByUser(param2);
         }
         this.tabPage.selectedIndex = param1;
      }
      
      public function getActiveWindow() : int
      {
         return this.tabPage.selectedIndex;
      }
      
      public function inventoryLocked() : Boolean
      {
         return this.locked;
      }
      
      public function setActiveFaction(param1:int) : void
      {
      }
      
      public function getRenderer(param1:int) : MovieClip
      {
         return this.tabPage.getButtonAt(param1);
      }
      
      public function lockIndex(param1:int, param2:Boolean) : void
      {
         var _loc3_:Object = this.tabPage.dataProvider[param1];
         if(_loc3_)
         {
            _loc3_.enabled = !param2;
         }
         if(this.QUESTS_INDEX == param1)
         {
            this._questButton.enabled = _loc3_.enabled;
         }
         trace("lockIndex",param1);
         this.tabPage.dataProvider.invalidate();
      }
      
      public function lockInventory(param1:Boolean) : void
      {
         if(this.locked == param1)
         {
            return;
         }
         locked = param1;
         this.tabPage.dataProvider[this.CHAR_INDEX] = {
            name:LocalizationData.getName("character"),
            icon:1,
            enabled:!param1
         };
         this.tabPage.dataProvider[this.SHOP_INDEX] = {
            name:LocalizationData.getName("shop"),
            icon:3,
            enabled:!param1
         };
         this.tabPage.dataProvider[this.INV_INDEX] = {
            name:LocalizationData.getName("inventrory"),
            icon:2,
            enabled:!param1
         };
         if((this.tabPage.selectedIndex == this.SHOP_INDEX || this.tabPage.selectedIndex == this.INV_INDEX || this.tabPage.selectedIndex == this.CHAR_INDEX) && param1 == true)
         {
            this.tabPage.selectedIndex = -1;
         }
         this.tabPage.dataProvider.invalidate();
      }
      
      protected function fill_windows(param1:String) : void
      {
         this.tabPage.dataProvider = new DataProvider([{
            name:LocalizationData.getName("character"),
            icon:1
         },{
            name:LocalizationData.getName("inventrory"),
            icon:2
         },{
            name:LocalizationData.getName("shop"),
            icon:3
         },{
            name:LocalizationData.getName("clan"),
            icon:4
         },{
            name:LocalizationData.getName("map"),
            icon:6,
            enabled:false
         },{
            name:LocalizationData.getName("ranking"),
            icon:5
         },{
            name:LocalizationData.getName("missions"),
            icon:7,
            enabled:true
         }]);
         this.tabPage.addEventListener(IndexEvent.INDEX_CHANGE,this.changeActiveMenu);
         SrpRooms.topMenu(this);
      }
      
      protected function changeActiveMenu(param1:IndexEvent) : void
      {
         if(MainScene.activePop && MainScene.activePop.closePopup)
         {
            MainScene.activePop.closePopup();
         }
         if(this.currentUI && this.currentUI.visible == true)
         {
            this.currentUI.removeEventListener(Event.CLOSE,this.onClose);
            this.currentUI.closeWindow();
            MainScene.activeWindow = null;
         }
         this._questButton.selected = this.tabPage.selectedIndex == this.QUESTS_INDEX;
         if(this.tabPage.selectedIndex == this.CHAR_INDEX)
         {
            currentUI = this.sceneContainer["charWindow"];
         }
         else if(this.tabPage.selectedIndex == this.INV_INDEX)
         {
            currentUI = this.sceneContainer["inventory_list"];
            this.sceneContainer["inventory_list"].toggleShopMode(false);
         }
         else if(this.tabPage.selectedIndex == this.SHOP_INDEX)
         {
            currentUI = this.sceneContainer["inventory_list"];
            this.sceneContainer["inventory_list"].toggleShopMode(true);
         }
         else if(this.tabPage.selectedIndex == this.QUESTS_INDEX)
         {
            currentUI = this.sceneContainer["questWindow"];
            trace("QUESTS_INDEX",this.currentUI);
            this.sceneContainer["questWindow"].onActivate();
         }
         else if(this.tabPage.selectedIndex == this.RANK_INDEX)
         {
            currentUI = this.sceneContainer["ranking"];
            this.sceneContainer["ranking"].onActivate();
         }
         else if(this.tabPage.selectedIndex == this.CLAN_INDEX)
         {
            currentUI = this.sceneContainer["clanWindow"];
            this.sceneContainer["clanWindow"].onActivate();
         }
         else
         {
            currentUI = null;
            MainScene.activeWindow = null;
         }
         if(this.currentUI)
         {
            trace("changeActiveMenu",this._byUser);
            MainScene.activeWindow = this.currentUI;
            if(this.sceneContainer["victory_screen"] && this.sceneContainer["victory_screen"].parent)
            {
               this.sceneContainer.addChildAt(this.currentUI,Math.max(this.sceneContainer.getChildIndex(this.sceneContainer["victory_screen"]) - 1,0));
            }
            else
            {
               this.sceneContainer.addChild(this.currentUI);
            }
            this.currentUI.activate(true,this._byUser);
            this.currentUI.addEventListener(Event.CLOSE,this.onClose);
         }
         else
         {
            MainScene.activeWindow = null;
         }
         _byUser = true;
         SrpRooms.topChanged(this);
      }
      
      protected function onClose(param1:Event) : void
      {
         this.currentUI.removeEventListener(Event.CLOSE,this.onClose);
         this.tabPage.selectedIndex = -1;
      }
      
      override protected function configUI() : void
      {
         sceneContainer = parent.parent as MovieClip;
         this.sceneContainer["shop_list"].visible = false;
         this.sceneContainer["charWindow"].visible = false;
         this.sceneContainer["inventory_list"].visible = false;
         this.sceneContainer["ranking"].visible = false;
      }
   }
}

