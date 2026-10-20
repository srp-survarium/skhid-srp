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
   import survarium.controls.ItemInfoRenderer;
   
   public dynamic class InventoryItemHint extends ItemInfoRenderer
   {
      
      public function InventoryItemHint()
      {
         super();
         this.__setProp_defaultModifiersList_InventoryItemHint_();
         this.__setProp_criticalModifiersList_InventoryItemHint_();
         this.__setProp_modifiersList_InventoryItemHint_();
      }
      
      internal function __setProp_defaultModifiersList_InventoryItemHint_() : *
      {
         try
         {
            defaultModifiersList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         defaultModifiersList.autoSize = "none";
         defaultModifiersList.buttonWidth = 0;
         defaultModifiersList.direction = "vertical";
         defaultModifiersList.enabled = true;
         defaultModifiersList.enableInitCallback = false;
         defaultModifiersList.focusable = true;
         defaultModifiersList.itemRendererName = "SrvModifierLabeledRendererDefault";
         defaultModifiersList.spacing = 0;
         defaultModifiersList.visible = true;
         try
         {
            defaultModifiersList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_criticalModifiersList_InventoryItemHint_() : *
      {
         try
         {
            criticalModifiersList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         criticalModifiersList.autoSize = "none";
         criticalModifiersList.buttonWidth = 0;
         criticalModifiersList.direction = "vertical";
         criticalModifiersList.enabled = true;
         criticalModifiersList.enableInitCallback = false;
         criticalModifiersList.focusable = true;
         criticalModifiersList.itemRendererName = "SrvModifierLabeledRenderer";
         criticalModifiersList.spacing = 0;
         criticalModifiersList.visible = true;
         try
         {
            criticalModifiersList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_modifiersList_InventoryItemHint_() : *
      {
         try
         {
            modifiersList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         modifiersList.autoSize = "none";
         modifiersList.buttonWidth = 0;
         modifiersList.direction = "vertical";
         modifiersList.enabled = true;
         modifiersList.enableInitCallback = false;
         modifiersList.focusable = true;
         modifiersList.itemRendererName = "SrvModifierLabeledRenderer";
         modifiersList.spacing = 0;
         modifiersList.visible = true;
         try
         {
            modifiersList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

