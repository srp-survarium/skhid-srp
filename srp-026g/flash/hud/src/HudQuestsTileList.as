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
   
   public dynamic class HudQuestsTileList extends MovieClip
   {
      
      public var tileList:SrvTileList;
      
      public function HudQuestsTileList()
      {
         super();
         this.__setProp_tileList_HudQuestsTileList_();
      }
      
      internal function __setProp_tileList_HudQuestsTileList_() : *
      {
         try
         {
            this.tileList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.tileList.columnWidth = 62;
         this.tileList.direction = "horizontal";
         this.tileList.enabled = true;
         this.tileList.enableInitCallback = false;
         this.tileList.externalColumnCount = 0;
         this.tileList.focusable = true;
         this.tileList.itemRendererName = "SrvQustListRenderer";
         this.tileList.itemRendererInstanceName = "";
         this.tileList.margin = 0;
         this.tileList.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         this.tileList.rowHeight = 85;
         this.tileList.scrollBar = "";
         this.tileList.visible = true;
         this.tileList.wrapping = "normal";
         try
         {
            this.tileList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

