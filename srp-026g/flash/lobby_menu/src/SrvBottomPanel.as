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
   import survarium.controls.BottomPanel;
   
   public dynamic class SrvBottomPanel extends BottomPanel
   {
      
      public function SrvBottomPanel()
      {
         super();
         this.__setProp_tabsBar_bottom_panel_();
         this.__setProp_barInvites_bottom_panel_();
      }
      
      internal function __setProp_tabsBar_bottom_panel_() : *
      {
         try
         {
            tabsBar["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         tabsBar.autoSize = "none";
         tabsBar.buttonWidth = 146;
         tabsBar.direction = "horizontal";
         tabsBar.enabled = true;
         tabsBar.enableInitCallback = false;
         tabsBar.focusable = true;
         tabsBar.itemRendererName = "ButtonChatTabBottom";
         tabsBar.spacing = 0;
         tabsBar.visible = true;
         try
         {
            tabsBar["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_barInvites_bottom_panel_() : *
      {
         try
         {
            barInvites["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         barInvites.autoSize = "none";
         barInvites.buttonWidth = 84;
         barInvites.direction = "horizontal";
         barInvites.enabled = true;
         barInvites.enableInitCallback = false;
         barInvites.focusable = true;
         barInvites.itemRendererName = "SvButtonInvite";
         barInvites.spacing = 0;
         barInvites.visible = true;
         try
         {
            barInvites["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

