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
   import survarium.clan.ManageClanWindow;
   
   public dynamic class ManageClanWindow extends survarium.clan.ManageClanWindow
   {
      
      public function ManageClanWindow()
      {
         super();
         this.__setProp_dailyMessage_ManageClanWindow_();
         this.__setProp_mainTabBar_ManageClanWindow_mange_0();
         this.__setProp_progressIndicator_ManageClanWindow_players_0();
      }
      
      internal function __setProp_dailyMessage_ManageClanWindow_() : *
      {
         try
         {
            dailyMessage["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         dailyMessage.actAsButton = false;
         dailyMessage.defaultText = "";
         dailyMessage.displayAsPassword = false;
         dailyMessage.editable = true;
         dailyMessage.enabled = true;
         dailyMessage.enableInitCallback = false;
         dailyMessage.focusable = true;
         dailyMessage.maxChars = 250;
         dailyMessage.minThumbSize = 1;
         dailyMessage.scrollBar = "SrvScrollBarSkinned";
         dailyMessage.text = "";
         dailyMessage.thumbOffset = {
            "top":0,
            "bottom":0
         };
         dailyMessage.visible = true;
         try
         {
            dailyMessage["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_mainTabBar_ManageClanWindow_mange_0() : *
      {
         try
         {
            mainTabBar["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         mainTabBar.autoSize = "none";
         mainTabBar.buttonWidth = 282;
         mainTabBar.direction = "horizontal";
         mainTabBar.enabled = true;
         mainTabBar.enableInitCallback = false;
         mainTabBar.focusable = true;
         mainTabBar.itemRendererName = "SrvButtonTab";
         mainTabBar.spacing = 0;
         mainTabBar.visible = true;
         try
         {
            mainTabBar["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_progressIndicator_ManageClanWindow_players_0() : *
      {
         try
         {
            progressIndicator["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         progressIndicator.enabled = true;
         progressIndicator.enableInitCallback = false;
         progressIndicator.maximum = 100;
         progressIndicator.minimum = 0;
         progressIndicator.value = 0;
         progressIndicator.visible = true;
         try
         {
            progressIndicator["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

