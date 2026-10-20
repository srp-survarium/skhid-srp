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
   import survarium.controls.StatusPanel;
   
   public dynamic class SrvTopMenuContainer extends StatusPanel
   {
      
      public function SrvTopMenuContainer()
      {
         super();
         this.__setProp_labelInfo_TopMenuContainer_Layer21_0();
         this.__setProp_labelInfoAdmin_TopMenuContainer_Layer21_0();
      }
      
      internal function __setProp_labelInfo_TopMenuContainer_Layer21_0() : *
      {
         try
         {
            labelInfo["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         labelInfo.autoSize = "left";
         labelInfo.enabled = true;
         labelInfo.enableInitCallback = false;
         labelInfo.text = "";
         labelInfo.visible = true;
         try
         {
            labelInfo["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_labelInfoAdmin_TopMenuContainer_Layer21_0() : *
      {
         try
         {
            labelInfoAdmin["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         labelInfoAdmin.autoSize = "left";
         labelInfoAdmin.enabled = true;
         labelInfoAdmin.enableInitCallback = false;
         labelInfoAdmin.text = "";
         labelInfoAdmin.visible = true;
         try
         {
            labelInfoAdmin["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

