package panel_fla
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
   
   public dynamic class BandlesList_28 extends MovieClip
   {
      
      public var background:MovieClip;
      
      public var list:DefaultScrollingListPLayerListIL;
      
      public var textField:TextField;
      
      public function BandlesList_28()
      {
         super();
         this.__setProp_list_BandlesList_();
      }
      
      internal function __setProp_list_BandlesList_() : *
      {
         try
         {
            this.list["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.list.enabled = true;
         this.list.enableInitCallback = false;
         this.list.focusable = true;
         this.list.itemRendererName = "";
         this.list.itemRendererInstanceName = "";
         this.list.margin = 0;
         this.list.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         this.list.rowHeight = 132;
         this.list.scrollBar = "";
         this.list.visible = true;
         this.list.wrapping = "normal";
         try
         {
            this.list["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

