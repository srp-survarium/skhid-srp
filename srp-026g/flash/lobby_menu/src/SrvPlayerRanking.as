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
   import survarium.controls.PlayerRanking;
   
   public dynamic class SrvPlayerRanking extends PlayerRanking
   {
      
      public function SrvPlayerRanking()
      {
         super();
         this.__setProp_tabBar_SrvPlayerRanking_top_label_0();
         this.__setProp_list_SrvPlayerRanking_players_list_0();
      }
      
      internal function __setProp_tabBar_SrvPlayerRanking_top_label_0() : *
      {
         try
         {
            tabBar["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         tabBar.autoSize = "none";
         tabBar.buttonWidth = 180;
         tabBar.direction = "horizontal";
         tabBar.enabled = true;
         tabBar.enableInitCallback = false;
         tabBar.focusable = true;
         tabBar.itemRendererName = "SvButtonWide2";
         tabBar.spacing = 20;
         tabBar.visible = true;
         try
         {
            tabBar["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_list_SrvPlayerRanking_players_list_0() : *
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
         list.itemRendererName = "SrvRankingRenderer";
         list.itemRendererInstanceName = "";
         list.margin = 0;
         list.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         list.rowHeight = 23;
         list.scrollBar = "SrvScrollBar";
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
   }
}

