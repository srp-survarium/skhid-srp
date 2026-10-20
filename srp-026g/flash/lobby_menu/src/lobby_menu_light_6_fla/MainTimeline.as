package lobby_menu_light_6_fla
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
   
   public dynamic class MainTimeline extends MovieClip
   {
      
      public var itemIcon:MovieClip;
      
      public var notificationContainer:SrvNotificationList;
      
      public var labelInfo:LabelMaintence;
      
      public var test:MessageUnlock;
      
      public var progressIndicator:LoadingIndicator;
      
      public var checkbox_all_traders:CheckBoxSkinned;
      
      public function MainTimeline()
      {
         super();
         this.__setProp_labelInfo_Scene1_top_panel_clan_0();
         this.__setProp_checkbox_all_traders_Scene1_notifications_0();
      }
      
      internal function __setProp_labelInfo_Scene1_top_panel_clan_0() : *
      {
         try
         {
            this.labelInfo["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.labelInfo.autoSize = "left";
         this.labelInfo.enabled = true;
         this.labelInfo.enableInitCallback = false;
         this.labelInfo.text = "";
         this.labelInfo.visible = true;
         try
         {
            this.labelInfo["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_checkbox_all_traders_Scene1_notifications_0() : *
      {
         try
         {
            this.checkbox_all_traders["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.checkbox_all_traders.autoSize = "none";
         this.checkbox_all_traders.data = "";
         this.checkbox_all_traders.enabled = true;
         this.checkbox_all_traders.enableInitCallback = false;
         this.checkbox_all_traders.focusable = true;
         this.checkbox_all_traders.label = "Все группировки";
         this.checkbox_all_traders.selected = false;
         this.checkbox_all_traders.visible = true;
         try
         {
            this.checkbox_all_traders["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

