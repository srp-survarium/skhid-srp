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
   import survarium.controls.QuestWindow;
   
   public dynamic class SrvQuestWindowSkinned extends QuestWindow
   {
      
      public function SrvQuestWindowSkinned()
      {
         super();
         this.__setProp_btnJoin_SrvQuestWindowSkinned_();
         this.__setProp_btnInfo_SrvQuestWindowSkinned_();
         this.__setProp_btnShop_SrvQuestWindowSkinned_();
      }
      
      internal function __setProp_btnJoin_SrvQuestWindowSkinned_() : *
      {
         try
         {
            btnJoin["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btnJoin.autoRepeat = false;
         btnJoin.autoSize = "none";
         btnJoin.data = "";
         btnJoin.enabled = true;
         btnJoin.enableInitCallback = false;
         btnJoin.focusable = true;
         btnJoin.label = "label";
         btnJoin.selected = false;
         btnJoin.tooltipRendererName = "";
         btnJoin.toggle = false;
         btnJoin.visible = true;
         try
         {
            btnJoin["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btnInfo_SrvQuestWindowSkinned_() : *
      {
         try
         {
            btnInfo["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btnInfo.autoRepeat = false;
         btnInfo.autoSize = "none";
         btnInfo.data = "";
         btnInfo.enabled = true;
         btnInfo.enableInitCallback = false;
         btnInfo.focusable = true;
         btnInfo.label = "label";
         btnInfo.selected = false;
         btnInfo.tooltipRendererName = "";
         btnInfo.toggle = false;
         btnInfo.visible = true;
         try
         {
            btnInfo["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btnShop_SrvQuestWindowSkinned_() : *
      {
         try
         {
            btnShop["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btnShop.autoRepeat = false;
         btnShop.autoSize = "none";
         btnShop.data = "";
         btnShop.enabled = true;
         btnShop.enableInitCallback = false;
         btnShop.focusable = true;
         btnShop.label = "label";
         btnShop.selected = false;
         btnShop.tooltipRendererName = "";
         btnShop.toggle = false;
         btnShop.visible = true;
         try
         {
            btnShop["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

