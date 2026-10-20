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
   import survarium.player.ProfileSelector;
   
   public dynamic class SrvProfileSelectorHUD extends ProfileSelector
   {
      
      public function SrvProfileSelectorHUD()
      {
         super();
         this.__setProp_tabPage_SrvProfileSelectorHUD_();
      }
      
      internal function __setProp_tabPage_SrvProfileSelectorHUD_() : *
      {
         try
         {
            tabPage["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         tabPage.autoSize = "none";
         tabPage.buttonWidth = 187;
         tabPage.direction = "horizontal";
         tabPage.enabled = true;
         tabPage.enableInitCallback = false;
         tabPage.focusable = true;
         tabPage.itemRendererName = "SrvPlayerProfileTabHUD";
         tabPage.spacing = 0;
         tabPage.visible = true;
         try
         {
            tabPage["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

