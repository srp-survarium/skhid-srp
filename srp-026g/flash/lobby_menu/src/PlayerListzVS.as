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
   import survarium.controls.PlayerList;
   
   public dynamic class PlayerListzVS extends PlayerList
   {
      
      public function PlayerListzVS()
      {
         super();
         this.__setProp_team2list_PlayerListzVS_team2_0();
         this.__setProp_team1list_PlayerListzVS_team1_0();
      }
      
      internal function __setProp_team2list_PlayerListzVS_team2_0() : *
      {
         try
         {
            team2list["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         team2list.enabled = true;
         team2list.enableInitCallback = false;
         team2list.focusable = true;
         team2list.itemRendererName = "VSPLayerListRenderer";
         team2list.itemRendererInstanceName = "";
         team2list.margin = 0;
         team2list.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         team2list.rowHeight = 28;
         team2list.scrollBar = "";
         team2list.visible = true;
         team2list.wrapping = "normal";
         try
         {
            team2list["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_team1list_PlayerListzVS_team1_0() : *
      {
         try
         {
            team1list["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         team1list.enabled = true;
         team1list.enableInitCallback = false;
         team1list.focusable = true;
         team1list.itemRendererName = "VSPLayerListRenderer";
         team1list.itemRendererInstanceName = "";
         team1list.margin = 0;
         team1list.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         team1list.rowHeight = 28;
         team1list.scrollBar = "";
         team1list.visible = true;
         team1list.wrapping = "normal";
         try
         {
            team1list["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

