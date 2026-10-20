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
   import survarium.controls.QuickSlots;
   
   public dynamic class SrvQuickSlots extends QuickSlots
   {
      
      public function SrvQuickSlots()
      {
         super();
         this.__setProp_slots_list_SrvQuickSlots_Layer1_0();
      }
      
      internal function __setProp_slots_list_SrvQuickSlots_Layer1_0() : *
      {
         try
         {
            slots_list["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         slots_list.columnWidth = 85;
         slots_list.direction = "horizontal";
         slots_list.enabled = true;
         slots_list.enableInitCallback = false;
         slots_list.externalColumnCount = 0;
         slots_list.focusable = true;
         slots_list.itemRendererName = "SrvQuickItemRenderer";
         slots_list.itemRendererInstanceName = "";
         slots_list.margin = 0;
         slots_list.inspectablePadding = {
            "top":4,
            "right":4,
            "bottom":4,
            "left":4
         };
         slots_list.rowHeight = 64;
         slots_list.scrollBar = "";
         slots_list.visible = true;
         slots_list.wrapping = "normal";
         try
         {
            slots_list["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

