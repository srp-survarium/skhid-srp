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
   import survarium.messages.MessageRepair;
   
   public dynamic class SrvMessageRepairSkinned extends MessageRepair
   {
      
      public function SrvMessageRepairSkinned()
      {
         super();
         this.__setProp_modifiersList_SrvMessageRepairSkinned_();
      }
      
      internal function __setProp_modifiersList_SrvMessageRepairSkinned_() : *
      {
         try
         {
            modifiersList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         modifiersList.columnWidth = 45;
         modifiersList.direction = "vertical";
         modifiersList.enabled = true;
         modifiersList.enableInitCallback = false;
         modifiersList.externalColumnCount = 0;
         modifiersList.focusable = true;
         modifiersList.itemRendererName = "ModificatorItemRednerer";
         modifiersList.itemRendererInstanceName = "";
         modifiersList.margin = 0;
         modifiersList.noIndexOnLostFocus = false;
         modifiersList.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         modifiersList.rowHeight = 40;
         modifiersList.scrollBar = "";
         modifiersList.visible = false;
         modifiersList.wrapping = "normal";
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

