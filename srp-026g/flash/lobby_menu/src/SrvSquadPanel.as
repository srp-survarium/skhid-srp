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
   import survarium.friends.SquadList;
   
   public dynamic class SrvSquadPanel extends SquadList
   {
      
      public function SrvSquadPanel()
      {
         super();
         this.__setProp_list_SrvSquadPanel_();
         this.__setProp_border_SrvSquadPanel_();
      }
      
      internal function __setProp_list_SrvSquadPanel_() : *
      {
         try
         {
            list["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         list.enabled = true;
         list.enableInitCallback = false;
         list.focusable = true;
         list.itemRendererName = "SrvSquadSideRenderer";
         list.itemRendererInstanceName = "";
         list.margin = 0;
         list.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         list.rowHeight = 83;
         list.scrollBar = "";
         list.visible = true;
         list.wrapping = "normal";
         try
         {
            list["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_border_SrvSquadPanel_() : *
      {
         try
         {
            border["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         border.autoRepeat = false;
         border.autoSize = "none";
         border.data = "";
         border.enabled = true;
         border.enableInitCallback = false;
         border.focusable = true;
         border.label = "";
         border.selected = false;
         border.tooltipRendererName = "";
         border.toggle = true;
         border.visible = true;
         try
         {
            border["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

