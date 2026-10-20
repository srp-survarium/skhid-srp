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
   import survarium.controls.OptionFilterControl;
   
   public dynamic class SrvFilterOPtionControl extends OptionFilterControl
   {
      
      public function SrvFilterOPtionControl()
      {
         super();
         this.__setProp_filterList_SrvFilterOptionControl_();
      }
      
      internal function __setProp_filterList_SrvFilterOptionControl_() : *
      {
         try
         {
            filterList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         filterList.columnWidth = 74;
         filterList.direction = "horizontal";
         filterList.enabled = true;
         filterList.enableInitCallback = false;
         filterList.externalColumnCount = 0;
         filterList.focusable = true;
         filterList.itemRendererName = "FilterItemCatRendererIL";
         filterList.itemRendererInstanceName = "";
         filterList.margin = 0;
         filterList.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         filterList.rowHeight = 28;
         filterList.scrollBar = "";
         filterList.visible = true;
         filterList.wrapping = "normal";
         try
         {
            filterList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

