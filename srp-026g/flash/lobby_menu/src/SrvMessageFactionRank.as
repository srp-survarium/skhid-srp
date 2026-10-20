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
   import survarium.messages.MessageFactionRank;
   
   public dynamic class SrvMessageFactionRank extends MessageFactionRank
   {
      
      public function SrvMessageFactionRank()
      {
         super();
         this.__setProp_btnAccept_mrssageFactionRank_();
         this.__setProp_btnCancel_mrssageFactionRank_();
         this.__setProp_itemsTileList_mrssageFactionRank_();
      }
      
      internal function __setProp_btnAccept_mrssageFactionRank_() : *
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
         btnAccept.label = "red_label";
         btnAccept.selected = false;
         btnAccept.tooltipRendererName = "";
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
      
      internal function __setProp_btnCancel_mrssageFactionRank_() : *
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
         btnCancel.enabled = true;
         btnCancel.enableInitCallback = false;
         btnCancel.focusable = true;
         btnCancel.label = "label";
         btnCancel.selected = false;
         btnCancel.tooltipRendererName = "";
         btnCancel.toggle = false;
         btnCancel.visible = true;
         try
         {
            btnCancel["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_itemsTileList_mrssageFactionRank_() : *
      {
         try
         {
            itemsTileList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         itemsTileList.columnWidth = 171;
         itemsTileList.direction = "horizontal";
         itemsTileList.enabled = true;
         itemsTileList.enableInitCallback = false;
         itemsTileList.externalColumnCount = 0;
         itemsTileList.focusable = true;
         itemsTileList.itemRendererName = "SrvListItemRenderer";
         itemsTileList.itemRendererInstanceName = "";
         itemsTileList.margin = 0;
         itemsTileList.noIndexOnLostFocus = false;
         itemsTileList.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         itemsTileList.rowHeight = 130;
         itemsTileList.scrollBar = "scrollBar";
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
   }
}

