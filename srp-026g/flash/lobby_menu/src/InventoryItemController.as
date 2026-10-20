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
   import survarium.controls.ItemControlPanel;
   
   public dynamic class InventoryItemController extends ItemControlPanel
   {
      
      public function InventoryItemController()
      {
         super();
         this.__setProp_btn_item_fix_InventoryItemController_();
         this.__setProp_btn_item_upgrade_InventoryItemController_();
         this.__setProp_btn_item_reroll_InventoryItemController_();
         this.__setProp_btn_item_craft_InventoryItemController_();
         this.__setProp_btn_item_buy_InventoryItemController_();
         this.__setProp_btn_item_sell_InventoryItemController_();
         this.__setProp_btn_show_r_InventoryItemController_();
         this.__setProp_btnAccept_InventoryItemController_();
         this.__setProp_btnCancel_InventoryItemController_();
         this.__setProp_btnSellBig_InventoryItemController_();
      }
      
      internal function __setProp_btn_item_fix_InventoryItemController_() : *
      {
         try
         {
            btn_item_fix["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btn_item_fix.autoRepeat = false;
         btn_item_fix.autoSize = "none";
         btn_item_fix.data = "";
         btn_item_fix.enabled = true;
         btn_item_fix.enableInitCallback = false;
         btn_item_fix.focusable = true;
         btn_item_fix.label = "Чинить";
         btn_item_fix.selected = false;
         btn_item_fix.toggle = false;
         btn_item_fix.visible = true;
         try
         {
            btn_item_fix["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btn_item_upgrade_InventoryItemController_() : *
      {
         try
         {
            btn_item_upgrade["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btn_item_upgrade.autoRepeat = false;
         btn_item_upgrade.autoSize = "none";
         btn_item_upgrade.data = "";
         btn_item_upgrade.enabled = true;
         btn_item_upgrade.enableInitCallback = false;
         btn_item_upgrade.focusable = true;
         btn_item_upgrade.label = "Установить";
         btn_item_upgrade.selected = false;
         btn_item_upgrade.toggle = false;
         btn_item_upgrade.visible = true;
         try
         {
            btn_item_upgrade["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btn_item_reroll_InventoryItemController_() : *
      {
         try
         {
            btn_item_reroll["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btn_item_reroll.autoRepeat = false;
         btn_item_reroll.autoSize = "none";
         btn_item_reroll.data = "";
         btn_item_reroll.enabled = true;
         btn_item_reroll.enableInitCallback = false;
         btn_item_reroll.focusable = true;
         btn_item_reroll.label = "Установить";
         btn_item_reroll.selected = false;
         btn_item_reroll.toggle = false;
         btn_item_reroll.visible = true;
         try
         {
            btn_item_reroll["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btn_item_craft_InventoryItemController_() : *
      {
         try
         {
            btn_item_craft["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btn_item_craft.autoRepeat = false;
         btn_item_craft.autoSize = "none";
         btn_item_craft.data = "";
         btn_item_craft.enabled = true;
         btn_item_craft.enableInitCallback = false;
         btn_item_craft.focusable = true;
         btn_item_craft.label = "Установить";
         btn_item_craft.selected = false;
         btn_item_craft.toggle = false;
         btn_item_craft.visible = true;
         try
         {
            btn_item_craft["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btn_item_buy_InventoryItemController_() : *
      {
         try
         {
            btn_item_buy["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btn_item_buy.autoRepeat = false;
         btn_item_buy.autoSize = "none";
         btn_item_buy.data = "";
         btn_item_buy.enabled = true;
         btn_item_buy.enableInitCallback = false;
         btn_item_buy.focusable = true;
         btn_item_buy.label = "Установить";
         btn_item_buy.selected = false;
         btn_item_buy.toggle = false;
         btn_item_buy.visible = true;
         try
         {
            btn_item_buy["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btn_item_sell_InventoryItemController_() : *
      {
         try
         {
            btn_item_sell["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btn_item_sell.autoRepeat = false;
         btn_item_sell.autoSize = "none";
         btn_item_sell.data = "";
         btn_item_sell.enabled = true;
         btn_item_sell.enableInitCallback = false;
         btn_item_sell.focusable = true;
         btn_item_sell.label = "Установить";
         btn_item_sell.selected = false;
         btn_item_sell.toggle = false;
         btn_item_sell.visible = true;
         try
         {
            btn_item_sell["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btn_show_r_InventoryItemController_() : *
      {
         try
         {
            btn_show_r["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btn_show_r.autoRepeat = false;
         btn_show_r.autoSize = "none";
         btn_show_r.data = "";
         btn_show_r.enabled = true;
         btn_show_r.enableInitCallback = false;
         btn_show_r.focusable = true;
         btn_show_r.label = "";
         btn_show_r.selected = false;
         btn_show_r.toggle = false;
         btn_show_r.visible = false;
         try
         {
            btn_show_r["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btnAccept_InventoryItemController_() : *
      {
         try
         {
            btnAccept["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btnAccept.autoRepeat = false;
         btnAccept.autoSize = "none";
         btnAccept.data = "";
         btnAccept.enabled = true;
         btnAccept.enableInitCallback = false;
         btnAccept.focusable = true;
         btnAccept.label = "s_accept";
         btnAccept.selected = false;
         btnAccept.toggle = false;
         btnAccept.visible = true;
         try
         {
            btnAccept["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btnCancel_InventoryItemController_() : *
      {
         try
         {
            btnCancel["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btnCancel.autoRepeat = false;
         btnCancel.autoSize = "none";
         btnCancel.data = "";
         btnCancel.enabled = false;
         btnCancel.enableInitCallback = false;
         btnCancel.focusable = true;
         btnCancel.label = "s_cancel";
         btnCancel.selected = false;
         btnCancel.toggle = false;
         btnCancel.visible = false;
         try
         {
            btnCancel["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btnSellBig_InventoryItemController_() : *
      {
         try
         {
            btnSellBig["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btnSellBig.autoRepeat = false;
         btnSellBig.autoSize = "none";
         btnSellBig.data = "";
         btnSellBig.enabled = true;
         btnSellBig.enableInitCallback = false;
         btnSellBig.focusable = true;
         btnSellBig.label = "";
         btnSellBig.selected = false;
         btnSellBig.toggle = false;
         btnSellBig.visible = false;
         try
         {
            btnSellBig["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

