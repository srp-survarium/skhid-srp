package
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   import survarium.controls.InventoryList;
   
   public dynamic class SrvInventoryReskin extends InventoryList
   {
      
      public function SrvInventoryReskin()
      {
         super();
         this.__setProp_sellersTabPage_SrvInventoryReskin_();
         this.__setProp_itemsTileList_SrvInventoryReskin_content_0();
         this.__setProp_scrollBar_SrvInventoryReskin_content_0();
         this.__setProp_inventoryTabPage_SrvInventoryReskin_content_0();
      }
      
      internal function __setProp_sellersTabPage_SrvInventoryReskin_() : *
      {
         try
         {
            sellersTabPage["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         sellersTabPage.autoSize = "none";
         sellersTabPage.buttonWidth = 110;
         sellersTabPage.direction = "horizontal";
         sellersTabPage.enabled = true;
         sellersTabPage.enableInitCallback = false;
         sellersTabPage.focusable = true;
         sellersTabPage.itemRendererName = "SrvSellerTabIL";
         sellersTabPage.spacing = 0;
         sellersTabPage.visible = true;
         try
         {
            sellersTabPage["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_itemsTileList_SrvInventoryReskin_content_0() : *
      {
         try
         {
            itemsTileList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         itemsTileList.columnWidth = 183;
         itemsTileList.direction = "vertical";
         itemsTileList.draggable = false;
         itemsTileList.enabled = true;
         itemsTileList.enableInitCallback = false;
         itemsTileList.externalColumnCount = 0;
         itemsTileList.focusable = true;
         itemsTileList.itemRendererName = "";
         itemsTileList.itemRendererInstanceName = "";
         itemsTileList.margin = 0;
         itemsTileList.noIndexOnLostFocus = false;
         itemsTileList.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         itemsTileList.rowHeight = 132;
         itemsTileList.scrollBar = "";
         itemsTileList.scrollYPadding = -10;
         itemsTileList.visible = true;
         itemsTileList.wrapping = "normal";
         try
         {
            itemsTileList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_scrollBar_SrvInventoryReskin_content_0() : *
      {
         try
         {
            scrollBar["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         scrollBar.enabled = true;
         scrollBar.enableInitCallback = false;
         scrollBar.minThumbSize = 10;
         scrollBar.offsetBottom = 0;
         scrollBar.offsetTop = 0;
         scrollBar.scrollTarget = "itemsTileList";
         scrollBar.trackMode = "scrollPage";
         scrollBar.visible = true;
         try
         {
            scrollBar["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_inventoryTabPage_SrvInventoryReskin_content_0() : *
      {
         try
         {
            inventoryTabPage["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         inventoryTabPage.autoSize = "none";
         inventoryTabPage.buttonWidth = 96;
         inventoryTabPage.direction = "horizontal";
         inventoryTabPage.enabled = true;
         inventoryTabPage.enableInitCallback = false;
         inventoryTabPage.focusable = true;
         inventoryTabPage.itemRendererName = "CategoryTabButtonIL";
         inventoryTabPage.spacing = 0;
         inventoryTabPage.visible = true;
         try
         {
            inventoryTabPage["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

